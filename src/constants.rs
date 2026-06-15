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
