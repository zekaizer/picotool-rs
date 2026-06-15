# Scope: flasher-first, RP2040-only

## Status

Accepted (2026-06-16)

## Context

The validated need is to flash a UF2 onto an RP2040 from a single non-rooted Android
tablet (LTE-only, one USB port). The project name `picotool-rs` invites a full picotool
reimplementation, and RP2350 (Pico 2) exists as a newer target. Both expansions carry
real cost the validated need does not require: a broader command surface, and for RP2350
a second PICOBOOT dialect, multiple UF2 family ids, and secure-boot/partition concepts.

## Decision

We will build a focused subset of picotool, prioritizing the `load` operation
(UF2 → flash → reboot), structured internally as reusable layers (`picoboot` / `uf2` /
`device`) behind a subcommand CLI so it can grow. We will target RP2040 only and
centralize device-identifying constants (VID/PID, UF2 family id, flash geometry) in one
place so a second target can be added later without restructuring. RP2350 and the
remaining picotool commands are explicitly out of scope for now.

## Consequences

- Smallest path to the validated outcome, with a fast host development loop.
- The name/scope gap is deliberate and recorded; later growth is additive, not a rewrite.
- A reader expecting full picotool parity will not find it, and RP2350 is unserved until
  its constants and PICOBOOT dialect are added.
