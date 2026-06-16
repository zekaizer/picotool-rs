# Termux device open: self-re-exec through `termux-usb`

## Status

Accepted (2026-06-16)

Extends [ADR 0002](0002-usb-access-via-nusb-and-termux-fd.md), which established the
`from_fd` open path and the `--fd` / `$TERMUX_USB_FD` handover but left obtaining that
descriptor as a manual, out-of-band step.

## Context

ADR 0002 opens the BOOTSEL device on non-rooted Android from a descriptor handed over by
`termux-usb`. In practice the user had to bridge two gaps by hand: discover the device
path (`termux-usb -l`) and then run the flash *inside* a `termux-usb -E -e` invocation,
wrapping the real command in a throwaway shell script. The wrapper exists only to absorb
the device path that `termux-usb` appends as a trailing argument — which `clap` would
otherwise reject:

```sh
termux-usb -l                                       # find the device path
echo 'picotool-rs load firmware.uf2' > flash.sh     # wrapper to swallow the trailing path
termux-usb -r -E -e 'sh flash.sh' <device-path>
```

This per-flash scripting is the main friction on the tablet. We want the user to discover
the path once (`termux-usb -l` stays manual, by choice) and then run a single command:

```sh
picotool-rs load firmware.uf2 --device /dev/bus/usb/001/002
```

Two facts about `termux-usb` constrain the design (confirmed from the public CLI behaviour,
not its source — see clean-room note below):

- With `-E`, the descriptor arrives via `$TERMUX_USB_FD`, but the device path is **still**
  appended as the final argv of the executed command.
- `-e` accepts a full multi-argument command string, so the historical wrapper was needed
  for trailing-argument absorption, not for argument packing.

A second gap surfaces once `--device` becomes the common path: the `from_fd` route performs
no device-identity check. Enumeration matches VID/PID before opening, but the descriptor
route only locates a vendor-class interface with bulk endpoints. A user who picks the wrong
entry from `termux-usb -l` could hand over a descriptor to an unintended device and have it
erased.

## Decision

We will make `--device <path>` (Android only) flash through `termux-usb` on the user's
behalf, in the binary, leaving the shared core untouched.

- **Wrap, do not reimplement.** When `--device` is given, the process re-executes itself
  through `termux-usb -r -E -e <self> <path>`, replacing its own image with
  `std::os::unix::process::CommandExt::exec`. We do not reimplement the Termux:API socket
  protocol. The `--device` path is `cfg(target_os = "android")`-gated; on other platforms
  it returns a clear "Termux/Android only" error, mirroring `DeviceError::FdUnsupported`.
- **Pass real arguments out-of-band.** The outer process serializes its effective
  arguments (everything except `--device <path>`) into an environment variable and sets a
  re-exec guard variable, then execs `termux-usb` with only the executable path as the
  `-e` command. The inner (re-execed) process detects the guard, ignores its argv entirely
  (so the trailing device path `termux-usb` appends is harmless), reads `$TERMUX_USB_FD` as
  it already does, and runs the flash. The guard also prevents an infinite re-exec loop.
- **Guard device identity on open.** `Device::from_nusb` rejects any device whose VID/PID
  is not the RP2040 BOOTSEL identity (`2e8a:0003`), on both the enumerate and the
  descriptor path. The check is always on and opt-out only via `--any-device`, threaded as
  a single boolean through `Device::open`. The VID/PID constants stop being host-only and
  become available on Android too.

## Considered Options

- **Reimplement `termux-usb` natively in Rust** (create the Termux:API local sockets,
  broadcast the intent, receive the `ParcelFileDescriptor`) — rejected: it requires
  reverse-engineering a private, undocumented, version-volatile IPC protocol from GPLv3
  source, which collides with the clean-room rule (ADR 0003). The `termux-usb` CLI is the
  stable, documented surface; it is already a required Termux package.
- **Automatic re-exec** (wrap whenever on Android without a descriptor) — rejected in favour
  of an explicit `--device`: supplying the path *is* the intent to go through `termux-usb`,
  so no separate flag or hidden magic is needed, and the desktop and Termux paths stay
  visibly distinct.
- **Inline command string** (`-e '<self> load file.uf2 ...'`, inner strips the trailing
  path) — rejected in favour of out-of-band env passing: it routes the user's file path
  through `termux-usb`'s command-string parsing, reintroducing a quoting/whitespace failure
  class, and couples us to exactly how `termux-usb` appends and splits arguments.
- **Identity guard behind a `--force`-style default-off, or fd-path-only** — rejected:
  always-on with an `--any-device` opt-out keeps both open paths under one invariant and
  matches the project's "safety defaults always on" convention; a single specific opt-out
  flag is clearer than reusing an overloaded `--force`.

## Consequences

- The tablet flow collapses to one command after a manual `termux-usb -l`; the wrapper
  script and any direct handling of `$TERMUX_USB_FD` disappear from the user's view, while
  the inner process still consumes the descriptor exactly as ADR 0002 specified.
- The re-exec glue is confined to the binary and gated to Android; the core stays a pure
  fd-in layer, and the `--fd` path remains for exercising `from_fd` on a Linux host.
- The identity guard makes "wrong path from `termux-usb -l`" a refusal rather than an
  erase, at the cost of one new error variant and lifting the host-only restriction on the
  VID/PID constants. `--any-device` exists for the deliberate exception.
- We depend on two `termux-usb` behaviours that are confirmed from CLI usage but not pinned
  by a spec: that custom environment variables are inherited by the executed child, and the
  exact argv it appends under `-r -E -e`. The out-of-band env design tolerates surprises in
  the latter; the former is verified on-device before relying on it for a real flash.
