# References

picotool-rs is a clean-room implementation (see
[ADR 0003](adr/0003-clean-room-reimplementation-and-license.md)). It is written against the
public specifications below. picotool's source is **not** used; picotool serves only as a
behavioral oracle for golden test vectors.

## Specifications

- **RP2040 datasheet** — PICOBOOT interface and USB bootloader.
  <https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf>
  (TODO: pin the exact section and document revision when first implementing.)
- **UF2 file format** — <https://github.com/microsoft/uf2> (`UF2.md`, `utils/uf2families.json`).
  (TODO: pin the commit used.)

## Known constants

- BOOTSEL device: VID `0x2E8A`, PID `0x0003`.
- UF2 family id (RP2040): `0xE48BFF56`.
- PICOBOOT command magic: `0x431FD10B`; UF2 block magic: `0x0A324655`.

## Behavioral oracle

- **picotool** — <https://github.com/raspberrypi/picotool>
  Used to capture golden USB traffic (usbmon / Wireshark) for unit-test vectors.
  (TODO: record the exact picotool version when vectors are captured.)
