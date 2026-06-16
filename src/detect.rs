//! Runtime flash-size detection.
//!
//! RP2040 PICOBOOT exposes no flash-size query, so the real size is recovered by running a
//! small stub on the device (PICOBOOT EXEC) that reads the flash JEDEC ID over XIP_SSI; see
//! [`crate::constants::FLASH_ID_STUB`] and [`crate::device::Device::detect_flash_size`].
//! This module holds the pure decoding step: turning the JEDEC id bytes into a flash size.
//!
//! A JEDEC RDID response is three bytes — manufacturer, memory type, capacity. For the SPI
//! NOR parts RP2040 boards use, the capacity byte `N` encodes a size of `1 << N` bytes (e.g.
//! Winbond W25Q16 reports `0x15` → 2 MiB). See `docs/references.md`.

use crate::constants::{MAX_DETECTED_FLASH_SIZE, MIN_DETECTED_FLASH_SIZE};

/// Decode a JEDEC RDID response into a flash size in bytes.
///
/// `id` is `[manufacturer, memory_type, capacity]`. The capacity byte `N` gives a size of
/// `1 << N`. Returns `None` when the decoded size falls outside the accepted range
/// ([`MIN_DETECTED_FLASH_SIZE`]..=[`MAX_DETECTED_FLASH_SIZE`]) — for instance the all-`0x00`
/// or all-`0xFF` response of absent or malfunctioning flash — so the caller can fall back to
/// the conservative window.
pub fn flash_size_from_jedec(id: [u8; 3]) -> Option<u32> {
    let capacity = id[2];
    // 1 << N, guarding against capacity values whose shift would overflow u32.
    let size = 1u32.checked_shl(u32::from(capacity))?;
    if (MIN_DETECTED_FLASH_SIZE..=MAX_DETECTED_FLASH_SIZE).contains(&size) {
        Some(size)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winbond_w25q16_is_2_mib() {
        // Manufacturer 0xEF (Winbond), type 0x40, capacity 0x15 -> 1 << 0x15 = 2 MiB.
        assert_eq!(
            flash_size_from_jedec([0xEF, 0x40, 0x15]),
            Some(2 * 1024 * 1024)
        );
    }

    #[test]
    fn typical_capacities_decode() {
        assert_eq!(flash_size_from_jedec([0xEF, 0x40, 0x14]), Some(1024 * 1024));
        assert_eq!(
            flash_size_from_jedec([0xEF, 0x40, 0x16]),
            Some(4 * 1024 * 1024)
        );
        assert_eq!(
            flash_size_from_jedec([0xEF, 0x40, 0x18]),
            Some(16 * 1024 * 1024)
        );
    }

    #[test]
    fn all_zero_read_is_rejected() {
        // Absent/asleep flash reads as 0x00 -> 1 << 0 = 1 byte, below the floor.
        assert_eq!(flash_size_from_jedec([0x00, 0x00, 0x00]), None);
    }

    #[test]
    fn all_ones_read_is_rejected() {
        // Capacity 0xFF -> 1 << 255 overflows the shift -> None.
        assert_eq!(flash_size_from_jedec([0xFF, 0xFF, 0xFF]), None);
    }

    #[test]
    fn above_xip_window_is_rejected() {
        // Capacity 0x19 -> 32 MiB, beyond the 16 MiB XIP window.
        assert_eq!(flash_size_from_jedec([0xEF, 0x40, 0x19]), None);
    }
}
