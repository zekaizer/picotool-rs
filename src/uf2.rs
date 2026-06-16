//! UF2 container parsing.
//!
//! Parses a UF2 file into coalesced flash [`Segment`]s ready to write. Two safety guards
//! are always on (see CLAUDE.md "Safety defaults"):
//!
//! - **family-id guard** — only blocks tagged with the RP2040 family id are accepted; a
//!   main-flash block carrying a different family id is skipped (combined images), and a
//!   main-flash block with no family id at all is rejected (the target cannot be confirmed).
//! - **flash-address guard** — every accepted block must lie within the flash XIP window.
//!
//! The 512-byte block layout follows the UF2 specification (see `docs/references.md`).

use crate::constants::{
    FLASH_START, RP2040_FAMILY_ID, UF2_BLOCK_SIZE, UF2_FLAG_FAMILY_ID_PRESENT,
    UF2_FLAG_NOT_MAIN_FLASH, UF2_MAGIC_END, UF2_MAGIC_START0, UF2_MAGIC_START1, UF2_MAX_PAYLOAD,
};
use thiserror::Error;

/// A contiguous run of bytes to write to flash, starting at [`addr`](Self::addr).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Absolute flash (XIP) start address.
    pub addr: u32,
    /// Bytes to write, in order.
    pub data: Vec<u8>,
}

impl Segment {
    /// First address past the end of this segment.
    pub fn end(&self) -> u32 {
        self.addr + self.data.len() as u32
    }
}

/// Errors produced while parsing a UF2 file.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Uf2Error {
    /// The input is empty.
    #[error("UF2 file is empty")]
    Empty,

    /// The file length is not a whole number of 512-byte blocks.
    #[error("UF2 file length {len} is not a multiple of {UF2_BLOCK_SIZE}")]
    Truncated {
        /// Length of the input in bytes.
        len: usize,
    },

    /// A block's start or end magic word did not match.
    #[error("UF2 block {block} has bad magic")]
    BadMagic {
        /// Zero-based block index.
        block: usize,
    },

    /// A block's declared payload size exceeds the maximum a block can carry.
    #[error("UF2 block {block} payload size {size} exceeds {UF2_MAX_PAYLOAD}")]
    PayloadTooLarge {
        /// Zero-based block index.
        block: usize,
        /// The declared payload size.
        size: u32,
    },

    /// A main-flash block carries no family id, so the RP2040 target cannot be confirmed.
    #[error("UF2 block {block} has no family id; cannot confirm RP2040 target")]
    MissingFamilyId {
        /// Zero-based block index.
        block: usize,
    },

    /// A block's target range falls outside the flash window (`[FLASH_START, flash_end)`).
    #[error(
        "UF2 block {block} target {addr:#010x}+{size} is outside flash [{FLASH_START:#010x}, {flash_end:#010x})"
    )]
    AddressOutOfRange {
        /// Zero-based block index.
        block: usize,
        /// The block's target address.
        addr: u32,
        /// The block's payload size.
        size: u32,
        /// The exclusive upper bound the block was checked against.
        flash_end: u32,
    },

    /// Two accepted blocks overlap in their target addresses.
    #[error("UF2 blocks overlap at {addr:#010x}")]
    Overlap {
        /// The overlapping address.
        addr: u32,
    },

    /// No RP2040 main-flash blocks were found in the file.
    #[error("UF2 file contains no RP2040 flash data")]
    NoFlashData,
}

/// Read a little-endian `u32` from `block` at byte offset `off`.
fn rd_u32(block: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]])
}

/// Parse a UF2 file into coalesced, address-ordered flash segments.
///
/// `flash_end` is the exclusive upper bound of the flash window every accepted block must
/// fall within; callers pass the runtime-detected flash size, or a conservative bound when
/// detection is unavailable.
///
/// Blocks flagged not-main-flash and blocks tagged with a non-RP2040 family id are
/// skipped. Returns an error if the file is malformed, a block is out of range or
/// untagged, accepted blocks overlap, or nothing flashable remains.
pub fn parse(bytes: &[u8], flash_end: u32) -> Result<Vec<Segment>, Uf2Error> {
    if bytes.is_empty() {
        return Err(Uf2Error::Empty);
    }
    if bytes.len() % UF2_BLOCK_SIZE != 0 {
        return Err(Uf2Error::Truncated { len: bytes.len() });
    }

    let mut segments: Vec<Segment> = Vec::new();

    for (block, chunk) in bytes.chunks_exact(UF2_BLOCK_SIZE).enumerate() {
        if rd_u32(chunk, 0) != UF2_MAGIC_START0
            || rd_u32(chunk, 4) != UF2_MAGIC_START1
            || rd_u32(chunk, UF2_BLOCK_SIZE - 4) != UF2_MAGIC_END
        {
            return Err(Uf2Error::BadMagic { block });
        }

        let flags = rd_u32(chunk, 8);
        if flags & UF2_FLAG_NOT_MAIN_FLASH != 0 {
            continue;
        }

        // family-id guard: require a family id on every main-flash block, and accept only
        // RP2040; other families belong to a different target in a combined image.
        if flags & UF2_FLAG_FAMILY_ID_PRESENT == 0 {
            return Err(Uf2Error::MissingFamilyId { block });
        }
        if rd_u32(chunk, 28) != RP2040_FAMILY_ID {
            continue;
        }

        let addr = rd_u32(chunk, 12);
        let size = rd_u32(chunk, 16);
        if size as usize > UF2_MAX_PAYLOAD {
            return Err(Uf2Error::PayloadTooLarge { block, size });
        }

        // flash-address guard: the whole payload must land inside the flash window.
        let end = u64::from(addr) + u64::from(size);
        if addr < FLASH_START || end > u64::from(flash_end) {
            return Err(Uf2Error::AddressOutOfRange {
                block,
                addr,
                size,
                flash_end,
            });
        }

        let payload = &chunk[32..32 + size as usize];
        append_block(&mut segments, addr, payload)?;
    }

    if segments.is_empty() {
        return Err(Uf2Error::NoFlashData);
    }
    Ok(segments)
}

/// Append a block's payload to the segment list, coalescing with the previous segment
/// when the addresses are contiguous and rejecting overlaps.
fn append_block(segments: &mut Vec<Segment>, addr: u32, payload: &[u8]) -> Result<(), Uf2Error> {
    if let Some(last) = segments.last_mut() {
        if addr == last.end() {
            last.data.extend_from_slice(payload);
            return Ok(());
        }
        if addr < last.end() {
            return Err(Uf2Error::Overlap { addr });
        }
    }
    segments.push(Segment {
        addr,
        data: payload.to_vec(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::FLASH_END;

    /// Build one 512-byte UF2 block from spec-derived field values.
    fn block(flags: u32, addr: u32, family: u32, payload: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; UF2_BLOCK_SIZE];
        let put =
            |b: &mut [u8], off: usize, v: u32| b[off..off + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut b, 0, UF2_MAGIC_START0);
        put(&mut b, 4, UF2_MAGIC_START1);
        put(&mut b, 8, flags);
        put(&mut b, 12, addr);
        put(&mut b, 16, payload.len() as u32);
        put(&mut b, 24, 1); // num_blocks
        put(&mut b, 28, family);
        b[32..32 + payload.len()].copy_from_slice(payload);
        put(&mut b, UF2_BLOCK_SIZE - 4, UF2_MAGIC_END);
        b
    }

    const RP2040: u32 = RP2040_FAMILY_ID;
    const FAM: u32 = UF2_FLAG_FAMILY_ID_PRESENT;

    #[test]
    fn single_block_yields_one_segment() {
        let b = block(FAM, FLASH_START, RP2040, &[1, 2, 3, 4]);
        let segs = parse(&b, FLASH_END).unwrap();
        assert_eq!(
            segs,
            vec![Segment {
                addr: FLASH_START,
                data: vec![1, 2, 3, 4]
            }]
        );
    }

    #[test]
    fn contiguous_blocks_coalesce() {
        let mut f = block(FAM, FLASH_START, RP2040, &[0xAA; 256]);
        f.extend(block(FAM, FLASH_START + 256, RP2040, &[0xBB; 256]));
        let segs = parse(&f, FLASH_END).unwrap();
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].addr, FLASH_START);
        assert_eq!(segs[0].data.len(), 512);
        assert_eq!(segs[0].data[0], 0xAA);
        assert_eq!(segs[0].data[256], 0xBB);
    }

    #[test]
    fn gap_splits_into_two_segments() {
        let mut f = block(FAM, FLASH_START, RP2040, &[1; 256]);
        f.extend(block(FAM, FLASH_START + 0x1000, RP2040, &[2; 256]));
        let segs = parse(&f, FLASH_END).unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].addr, FLASH_START + 0x1000);
    }

    #[test]
    fn not_main_flash_block_is_skipped() {
        let mut f = block(FAM | UF2_FLAG_NOT_MAIN_FLASH, FLASH_START, RP2040, &[9; 4]);
        f.extend(block(FAM, FLASH_START, RP2040, &[1, 2, 3, 4]));
        let segs = parse(&f, FLASH_END).unwrap();
        assert_eq!(
            segs,
            vec![Segment {
                addr: FLASH_START,
                data: vec![1, 2, 3, 4]
            }]
        );
    }

    #[test]
    fn foreign_family_block_is_skipped() {
        let mut f = block(FAM, FLASH_START, 0x1122_3344, &[9; 4]);
        f.extend(block(FAM, FLASH_START + 0x2000, RP2040, &[5, 6]));
        let segs = parse(&f, FLASH_END).unwrap();
        assert_eq!(
            segs,
            vec![Segment {
                addr: FLASH_START + 0x2000,
                data: vec![5, 6]
            }]
        );
    }

    #[test]
    fn missing_family_id_is_rejected() {
        let b = block(0, FLASH_START, 0, &[1, 2, 3, 4]);
        assert_eq!(
            parse(&b, FLASH_END),
            Err(Uf2Error::MissingFamilyId { block: 0 })
        );
    }

    #[test]
    fn address_below_flash_is_rejected() {
        let b = block(FAM, FLASH_START - 4, RP2040, &[1, 2, 3, 4]);
        assert!(matches!(
            parse(&b, FLASH_END),
            Err(Uf2Error::AddressOutOfRange { .. })
        ));
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut b = block(FAM, FLASH_START, RP2040, &[1, 2, 3, 4]);
        b[0] ^= 0xFF;
        assert_eq!(parse(&b, FLASH_END), Err(Uf2Error::BadMagic { block: 0 }));
    }

    #[test]
    fn non_block_aligned_length_is_rejected() {
        let b = vec![0u8; 100];
        assert_eq!(parse(&b, FLASH_END), Err(Uf2Error::Truncated { len: 100 }));
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(parse(&[], FLASH_END), Err(Uf2Error::Empty));
    }

    #[test]
    fn only_foreign_blocks_yields_no_flash_data() {
        let b = block(FAM, FLASH_START, 0x1122_3344, &[9; 4]);
        assert_eq!(parse(&b, FLASH_END), Err(Uf2Error::NoFlashData));
    }

    #[test]
    fn overlapping_blocks_are_rejected() {
        let mut f = block(FAM, FLASH_START + 256, RP2040, &[1; 256]);
        f.extend(block(FAM, FLASH_START, RP2040, &[2; 256]));
        assert!(matches!(
            parse(&f, FLASH_END),
            Err(Uf2Error::Overlap { .. })
        ));
    }

    #[test]
    fn block_past_detected_flash_end_is_rejected() {
        // With a 2 MiB flash_end, a block whose payload spills past it is refused even
        // though it sits well within the conservative 16 MiB window.
        let two_mib = FLASH_START + 2 * 1024 * 1024;
        let b = block(FAM, two_mib - 4, RP2040, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(matches!(
            parse(&b, two_mib),
            Err(Uf2Error::AddressOutOfRange { flash_end, .. }) if flash_end == two_mib
        ));
    }

    #[test]
    fn block_ending_exactly_at_flash_end_is_accepted() {
        let two_mib = FLASH_START + 2 * 1024 * 1024;
        let b = block(FAM, two_mib - 4, RP2040, &[1, 2, 3, 4]);
        let segs = parse(&b, two_mib).unwrap();
        assert_eq!(segs[0].end(), two_mib);
    }
}
