# picotool-rs

Flash a UF2 onto an RP2040 over USB PICOBOOT — from a desktop host (macOS / Linux)
or from a non-rooted Android tablet via Termux, with one shared codebase.

> **Status: MVP.** The `load` command (UF2 → flash → reboot) is implemented and builds for
> desktop hosts and Android (Termux); end-to-end flashing on hardware is a manual check (no
> CI hardware). Scope is flasher-first and RP2040-only — see
> [ADR 0001](docs/adr/0001-scope-flasher-first-rp2040-only.md).

## Usage

Desktop host — enumerate the BOOTSEL device by VID/PID and flash:

```sh
picotool-rs load firmware.uf2        # add --verify for read-back
# Linux: needs a udev rule for VID 2e8a, or run with sudo
# macOS: no setup (PICOBOOT is a vendor interface)
```

Android tablet (Termux) — `termux-usb` runs the given command with the device path appended
and, with `-E`, exports the fd as `$TERMUX_USB_FD`. picotool-rs reads that fd, so a one-line
wrapper lets it ignore the trailing path:

```sh
termux-usb -l                                       # find the device path
echo 'picotool-rs load firmware.uf2' > flash.sh
termux-usb -r -E -e 'sh flash.sh' <device-path>     # approve the on-screen permission prompt
```

## Documentation

- [CONTEXT.md](CONTEXT.md) — glossary (BOOTSEL device, PICOBOOT, UF2, flash, load)
- [docs/adr](docs/adr/) — architecture decisions and the reasoning behind them

## License

MIT OR Apache-2.0
