# References

picotool-rs is a clean-room implementation (see
[ADR 0003](adr/0003-clean-room-reimplementation-and-license.md)). It is written against the
public specifications below. picotool's source is **not** used; picotool serves only as a
behavioral oracle for golden test vectors.

## Specifications

### RP2040 datasheet — USB PICOBOOT interface and bootloader

- **Document:** *RP2040 Datasheet*, Raspberry Pi Ltd.
  <https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf> (canonical URL; redirects
  to the versioned asset `RP-008371-DS-1-rp2040-datasheet.pdf`).
- **Revision pinned:** build-date **2025-02-20**, build-version **3184e62-clean** (Colophon,
  p. 2; release-history entry "20 February 2025"). The RP2040 datasheet no longer carries a
  single "release N" number, so this build-date / build-version pair is the version of record.
- **Sections used:**
  - **§2.8.4.2 "UF2 Format Details"** — the RP2040 UF2 contract: family id `0xE48BFF56`,
    256-byte payloads, and the `0x10000000–0x11000000` flash window.
  - **§2.8.5 "USB PICOBOOT Interface"** — the vendor interface picotool-rs drives: device /
    interface / endpoint identity (§2.8.5.1–.3 — VID `0x2E8A` PID `0x0003`,
    `bInterfaceClass 0xFF`, one bulk OUT/IN pair), the 32-byte command packet and command set
    (§2.8.5.4, Table 174 — encoded in `src/picoboot.rs`), and the control requests (§2.8.5.5).

> **Naming / errata note.** The datasheet names the interface-reset control request
> INTERFACE_RESET (`0x41`, §2.8.5.5.1, Table 184); this is what `constants::PICOBOOT_IF_RESET`
> encodes (the pico-sdk header calls it `PICOBOOT_RESET`). It is a **vendor** request
> (`bmRequestType 01000001b`); earlier datasheet builds mislabelled it as a class request
> (`00100001b`) — see
> [pico-feedback #99](https://github.com/raspberrypi/pico-feedback/issues/99). The pinned
> 2025-02-20 build carries the fix.

### UF2 file format

- **Repository:** <https://github.com/microsoft/uf2> (Microsoft).
- **Spec text:** `README.md` (block layout, magic words, flag bits) — the repo has no
  `UF2.md`; `uf2.h` is the C reference header.
- **Family-id registry:** `utils/uf2families.json` — RP2040 is `0xE48BFF56` (`short_name`
  "RP2040").
- **Revision pinned:** `master` at commit
  [`90e9741`](https://github.com/microsoft/uf2/commit/90e9741f217f5a40c98ba74d663e408041037578)
  (`90e9741f217f5a40c98ba74d663e408041037578`, 2026-02-08).

## Known constants

These mirror `src/constants.rs` (the single source of truth per
[ADR 0001](adr/0001-scope-flasher-first-rp2040-only.md)); each traces to a spec section above.

- BOOTSEL device: VID `0x2E8A`, PID `0x0003` (datasheet §2.8.5.1).
- UF2 family id (RP2040): `0xE48BFF56` (datasheet §2.8.4.2; `uf2families.json`).
- PICOBOOT command magic: `0x431FD10B` (datasheet §2.8.5.4, Table 174).
- UF2 block magic: `MAGIC_START0 0x0A324655`, `MAGIC_START1 0x9E5D5157`,
  `MAGIC_END 0x0AB16F30` (UF2 `README.md`).

## Behavioral oracle

- **picotool** — <https://github.com/raspberrypi/picotool>
  Used to capture golden USB traffic (usbmon / Wireshark) for unit-test vectors.
  (TODO: record the exact picotool version when vectors are captured.)
