# picotool-rs

Flash a UF2 onto an RP2040 over USB PICOBOOT — from a desktop host (macOS / Linux)
or from a non-rooted Android tablet via Termux, with one shared codebase.

> **Status: early.** Architecture decisions are recorded in [docs/adr](docs/adr/);
> implementation has not started. Scope is flasher-first and RP2040-only — see
> [ADR 0001](docs/adr/0001-scope-flasher-first-rp2040-only.md).

## Usage

Desktop host — enumerate the BOOTSEL device by VID/PID and flash:

```sh
picotool-rs load firmware.uf2        # add --verify for read-back
# Linux: needs a udev rule for VID 2e8a, or run with sudo
# macOS: no setup (PICOBOOT is a vendor interface)
```

Android tablet (Termux) — `termux-usb` hands over the device fd via `-E`:

```sh
termux-usb -l                                          # find the device path
UF2=firmware.uf2 termux-usb -r -E -e picotool-rs <device-path>
# -E exports the fd as $TERMUX_USB_FD; approve the on-screen permission prompt
```

## Documentation

- [CONTEXT.md](CONTEXT.md) — glossary (BOOTSEL device, PICOBOOT, UF2, flash, load)
- [docs/adr](docs/adr/) — architecture decisions and the reasoning behind them

## License

MIT OR Apache-2.0
