# picotool-rs — Project Instructions

picotool-rs flashes a UF2 onto an RP2040 over USB PICOBOOT, from a desktop host or a
non-rooted Android tablet (Termux). Start at [README.md](README.md) and
[CONTEXT.md](CONTEXT.md); the rationale behind the constraints below lives in
[docs/adr](docs/adr/).

## Ground rules

- **Documentation uses progressive disclosure.** Keep each document's entry point lean
  and high-level; push detail into linked sub-documents surfaced on demand. This file
  included — state constraints as one-liners and link out for the "why".
- Use the canonical terms from [CONTEXT.md](CONTEXT.md) (BOOTSEL device, PICOBOOT, UF2,
  flash, load) in code and docs.

## Engineering constraints

Decided (rationale in the linked ADR). Do not silently deviate; to change one, supersede
the ADR rather than editing it.

- **Clean-room only.** Never copy or translate picotool (or any other) source. Implement
  from the public specs; use picotool only as a behavioral oracle.
  ([ADR 0003](docs/adr/0003-clean-room-reimplementation-and-license.md))
- **RP2040 only.** Centralize device-identifying constants (VID/PID, UF2 family id, flash
  geometry) in one module; do not scatter them.
  ([ADR 0001](docs/adr/0001-scope-flasher-first-rp2040-only.md))
- **One shared core; only open differs.** picoboot / uf2 / device layers are
  platform-agnostic. Open dispatches at runtime on a supplied fd (`--fd` /
  `$TERMUX_USB_FD`) vs VID/PID enumeration; `from_fd` is `cfg`-gated to linux/android, so
  macOS compiles enumerate-only.
  ([ADR 0002](docs/adr/0002-usb-access-via-nusb-and-termux-fd.md))
- **nusb (pure Rust), blocking, no async runtime.** Do not add tokio or another executor.
  ([ADR 0002](docs/adr/0002-usb-access-via-nusb-and-termux-fd.md))
- **Safety defaults** (convention, no ADR): family-id and flash-address guards are always
  on; read-back verify is opt-in (`--verify`).

## Testing

- Pure layers (UF2 parse, PICOBOOT command encoding) are unit-tested against golden
  byte-vectors captured once from picotool via usbmon and committed as fixtures.
- USB I/O and end-to-end flashing are manual hardware checks (no CI hardware).

## License

MIT OR Apache-2.0.
