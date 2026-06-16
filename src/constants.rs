//! RP2040 device-identifying constants and flash geometry.
//!
//! Per [ADR 0001], every value that identifies the target (USB VID/PID, UF2 family id,
//! flash geometry) and every protocol magic lives here, so adding a second target later
//! is a matter of extending this module rather than hunting constants across the tree.
//!
//! Sources: RP2040 datasheet (USB bootloader / PICOBOOT) and the UF2 specification.
//! See `docs/references.md`.
//!
//! [ADR 0001]: ../../docs/adr/0001-scope-flasher-first-rp2040-only.md

// ---- USB identity (BOOTSEL device) ----

/// USB vendor id of the RP2040 in BOOTSEL (USB bootloader) mode.
pub const VENDOR_ID: u16 = 0x2E8A;

/// USB product id of the RP2040 PICOBOOT interface.
pub const PRODUCT_ID: u16 = 0x0003;

/// USB interface class of the PICOBOOT vendor interface.
///
/// The BOOTSEL device also exposes a mass-storage interface (class `0x08`); the PICOBOOT
/// interface is the vendor-specific one, matched on this class.
pub const PICOBOOT_INTERFACE_CLASS: u8 = 0xFF;

// ---- UF2 container ----

/// UF2 block magic (`MAGIC_START0`), first word of every 512-byte block.
pub const UF2_MAGIC_START0: u32 = 0x0A32_4655;
/// UF2 block magic (`MAGIC_START1`), second word of every 512-byte block.
pub const UF2_MAGIC_START1: u32 = 0x9E5D_5157;
/// UF2 block magic (`MAGIC_END`), final word of every 512-byte block.
pub const UF2_MAGIC_END: u32 = 0x0AB1_6F30;

/// Total size of a UF2 block in bytes.
pub const UF2_BLOCK_SIZE: usize = 512;
/// Maximum payload bytes carried by a single UF2 block.
pub const UF2_MAX_PAYLOAD: usize = 476;

/// UF2 flag: the `file_size` field instead carries a family id.
pub const UF2_FLAG_FAMILY_ID_PRESENT: u32 = 0x0000_2000;
/// UF2 flag: this block is not destined for the main flash and must be skipped.
pub const UF2_FLAG_NOT_MAIN_FLASH: u32 = 0x0000_0001;

/// UF2 family id identifying an RP2040 image.
pub const RP2040_FAMILY_ID: u32 = 0xE48B_FF56;

// ---- Flash geometry (XIP-mapped external QSPI) ----

/// Base address of the flash XIP window.
pub const FLASH_START: u32 = 0x1000_0000;

/// Conservative upper bound of the flash XIP window (16 MiB).
///
/// The RP2040 has no on-chip flash; actual size depends on the board's external QSPI part.
/// The precise size needs a runtime device query (out of MVP scope); this window is the
/// hardware-addressable maximum and is used only as a guard rail to refuse writes that
/// clearly fall outside flash.
pub const FLASH_END: u32 = FLASH_START + 16 * 1024 * 1024;

/// Flash erase sector size; erase address and length must be multiples of this.
pub const FLASH_SECTOR_SIZE: u32 = 4096;

/// Flash program page size.
pub const FLASH_PAGE_SIZE: u32 = 256;

/// Top of SRAM (stack pointer used for a normal reboot-to-application).
pub const SRAM_END: u32 = 0x2004_2000;

// ---- PICOBOOT protocol ----

/// Magic word at the head of every 32-byte PICOBOOT command.
pub const PICOBOOT_MAGIC: u32 = 0x431F_D10B;

/// Vendor control request (host-to-interface) that resets the PICOBOOT interface,
/// clearing any half-finished command and stalled endpoint state.
pub const PICOBOOT_IF_RESET: u8 = 0x41;

// ---- Flash-size detection (PICOBOOT EXEC stub) ----

/// SRAM address the flash-id stub is written to and executed from.
///
/// The RP2040 USB bootrom does not document which SRAM it touches while servicing PICOBOOT,
/// so no address is provably free. Main-SRAM base is the conservative choice: the bootrom
/// loads and runs RAM-only UF2 images from the lowest main-SRAM address (datasheet §2.8,
/// erratum RP2040-E9), so it must leave this region available for downloaded code. The
/// choice is verified on real hardware (#3).
pub const STUB_ADDR: u32 = 0x2000_0000;

/// SRAM address the stub writes the three JEDEC id bytes to (manufacturer, type, capacity),
/// placed past the stub body so code and result never overlap.
pub const RESULT_ADDR: u32 = 0x2000_0080;

/// Clean-room thumb (ARMv6-M) machine code that reads the flash JEDEC ID (RDID, `0x9F`) over
/// XIP_SSI and stores the three id bytes at [`RESULT_ADDR`]. Run via PICOBOOT EXEC: it takes
/// no arguments and returns nothing, communicating only through RAM (datasheet §2.8.5.4.8).
///
/// It drives the Synopsys SSI directly (XIP_SSI base `0x1800_0000`, datasheet §4.10.12):
/// disable, set CTRLR0 to 8-bit standard-SPI transmit-and-receive, enable, select slave 0,
/// push the opcode plus three dummy bytes, spin until `RXFLR == 4`, then read the four
/// frames (the first is the opcode echo, discarded). The assembly source and this byte
/// vector's disassembly are reproduced in [ADR 0005].
///
/// [ADR 0005]: ../../docs/adr/0005-runtime-flash-size-detection.md
#[rustfmt::skip]
pub const FLASH_ID_STUB: [u8; 62] = [
    0x10, 0xb5, 0x18, 0x20, 0x00, 0x06, 0x00, 0x21, 0x81, 0x60, 0x07, 0x22,
    0x12, 0x04, 0x02, 0x60, 0x41, 0x60, 0x01, 0x22, 0x82, 0x60, 0x02, 0x61,
    0x9f, 0x22, 0x02, 0x66, 0x01, 0x66, 0x01, 0x66, 0x01, 0x66, 0x42, 0x6a,
    0x04, 0x2a, 0xfc, 0xd3, 0x20, 0x24, 0x24, 0x06, 0x80, 0x34, 0x02, 0x6e,
    0x02, 0x6e, 0x22, 0x70, 0x02, 0x6e, 0x62, 0x70, 0x02, 0x6e, 0xa2, 0x70,
    0x10, 0xbd,
];

/// Smallest flash size accepted from runtime detection; a JEDEC capacity decoding to less is
/// treated as a bogus read, and detection falls back to the conservative window.
pub const MIN_DETECTED_FLASH_SIZE: u32 = 64 * 1024;

/// Largest flash size accepted from runtime detection: the XIP-addressable maximum,
/// `FLASH_END - FLASH_START`.
pub const MAX_DETECTED_FLASH_SIZE: u32 = FLASH_END - FLASH_START;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_addr_does_not_overlap_stub() {
        assert!(RESULT_ADDR >= STUB_ADDR + FLASH_ID_STUB.len() as u32);
    }

    #[test]
    fn detection_bounds_are_sane() {
        assert!(MIN_DETECTED_FLASH_SIZE < MAX_DETECTED_FLASH_SIZE);
        assert_eq!(MAX_DETECTED_FLASH_SIZE, FLASH_END - FLASH_START);
    }
}
