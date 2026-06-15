# picotool-rs

A Rust tool for flashing RP2040 devices over USB PICOBOOT. It runs both on desktop
hosts (macOS / Linux) and on a non-rooted Android tablet via Termux. Flasher-first:
the `load` operation (UF2 → flash → reboot) is the focus; broader picotool commands
may follow.

## Language

**BOOTSEL device**:
An RP2040 enumerated in its USB ROM bootloader mode (VID `0x2E8A`, PID `0x0003`),
exposing the PICOBOOT vendor interface. This is the thing picotool-rs opens and flashes.
_Avoid_: "the Pico" (that is the board), "RPI-RP2" (that is the mass-storage volume, not the device)

**PICOBOOT**:
The USB vendor-interface command protocol on the BOOTSEL device used to reset, erase
and write flash, and reboot. _Avoid_: "bootloader protocol"

**UF2**:
The 512-byte-block firmware container format picotool-rs parses to recover the flash
payload and its target addresses. RP2040 family id is `0xE48BFF56`.
_Avoid_: "binary", "image"

**flash** (verb):
To write firmware into the RP2040's external QSPI flash via PICOBOOT. picotool-rs's
primary action. _Avoid_: "burn", "upload", "program"

**load**:
The CLI subcommand that performs a flash (name aligned with picotool's `load`).
_Avoid_: "flash" or "write" as a command name
