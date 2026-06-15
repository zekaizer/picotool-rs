# USB device access: nusb + termux-usb file descriptor

## Status

Accepted (2026-06-16)

## Context

We must reach the RP2040's PICOBOOT USB interface from two environments with one shared
codebase: desktop hosts (macOS / Linux) and a non-rooted Android tablet via Termux. On
non-rooted Android, `/dev/bus/usb` is inaccessible (SELinux), so direct enumeration and
mass-storage paths are out; the only sanctioned route is a file descriptor handed over by
`termux-usb` after an on-device permission prompt. The obvious USB library, libusb/rusb,
needs a C toolchain that complicates on-device Termux builds.

## Decision

We will use nusb (pure Rust) for all USB access. The core (PICOBOOT + UF2 +
erase/write/reboot) is shared; only device-open differs. Open is dispatched at runtime:
when a descriptor is supplied — the `--fd` flag, or the `$TERMUX_USB_FD` env var set by
`termux-usb -E` — we wrap it with `nusb::Device::from_fd`; otherwise we enumerate by
VID/PID and open. Because `from_fd` exists only on Linux and Android, that branch is
`cfg`-gated to those targets, so macOS compiles to an enumerate-only binary. We will use
nusb's blocking adapter and add no async runtime.

## Considered Options

- **libusb / rusb** — rejected: the C dependency complicates on-device Termux builds;
  nusb being pure Rust builds uniformly on host and tablet.
- **Mass-storage drag-and-drop, block device `/dev/sdX`, ADB-over-USB, wireless ADB** —
  rejected respectively: unreliable RPI-RP2 FAT mount on Android, no `/dev/bus/usb` on
  non-root, USB data-role conflict (tablet can be ADB device or USB host, not both), and
  blocked corporate network.
- **Compile-time `cfg(target_os)` open switch** — rejected: collapses three platforms into
  two and prevents exercising the Android open path (`from_fd`) on a Linux host.

## Consequences

- One core, two open lines: the Android-only surface shrinks to the termux permission
  glue, and `from_fd` stays testable on a Linux host.
- No async runtime keeps the dependency tree minimal and the Termux build/download light
  over LTE.
- `termux-usb` prompts for permission on every run (single-shot model); accepted because
  the smoother alternative (Rust-as-APK) requires NDK cross-compilation off-device.
- Requires recent, matching-signature Termux + Termux:API apps; `-E` needs a recent
  Termux:API.
