# Clean-room reimplementation, not a port; permissive license

## Status

Accepted (2026-06-16)

## Context

picotool is Raspberry Pi's tool under BSD-3-Clause. picotool-rs covers overlapping
behavior, so the tempting shortcut is to translate picotool's C++ sources — but that makes
picotool-rs a derivative work bound to BSD-3-Clause with attribution, an obligation that is
hard to shed later. The PICOBOOT protocol and UF2 format are publicly specified (RP2040
datasheet, UF2 spec).

## Decision

We will implement from the published specifications only. picotool is used solely as a
behavioral oracle: its real USB traffic is captured once (usbmon / Wireshark) and committed
as golden byte-vector fixtures, against which the pure layers (UF2 parsing and 32-byte
PICOBOOT command encoding) are unit-tested; USB I/O and end-to-end flashing remain manual
hardware checks. No picotool source is copied or translated. We license picotool-rs under
`MIT OR Apache-2.0` (Rust convention).

## Consequences

- No derivative-work obligation; the license is ours to choose.
- Golden vectors turn "diff against picotool" into a hardware-free regression asset that
  runs in CI.
- Clean-room is more work than translating, and correctness rests on our reading of the
  spec — mitigated by the captured oracle vectors.
- Golden-vector capture needs a Linux host (usbmon), which aligns with the Linux
  verification host.
- Follow-up (outside this ADR): add `LICENSE-MIT` + `LICENSE-APACHE` files and the
  `Cargo.toml` `license = "MIT OR Apache-2.0"` field when the crate is scaffolded.
