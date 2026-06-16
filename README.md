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

Android tablet (Termux) — pass the device path with `--device`; picotool-rs re-executes
itself through `termux-usb` to obtain the descriptor, so it is one command rather than a
wrapper script ([ADR 0004](docs/adr/0004-termux-device-self-reexec.md)):

```sh
termux-usb -l                                                  # find the device path
picotool-rs load firmware.uf2 --device <device-path>          # approve the on-screen prompt
```

The opened device's VID/PID is checked against the RP2040 BOOTSEL identity before flashing;
pass `--any-device` to override. Advanced: `--fd <n>` / `$TERMUX_USB_FD` accept a descriptor
directly if you are driving `termux-usb` yourself.

## Documentation

- [CONTEXT.md](CONTEXT.md) — glossary (BOOTSEL device, PICOBOOT, UF2, flash, load)
- [docs/adr](docs/adr/) — architecture decisions and the reasoning behind them

## License

MIT OR Apache-2.0
