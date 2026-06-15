//! The `load` operation: drive a parsed UF2 onto flash and reboot.
//!
//! Sequence over the PICOBOOT transport: take exclusive access, exit XIP, erase the sectors
//! covering the image, write each segment, optionally read back and verify, then reboot to
//! the freshly flashed application.
//!
//! The sector-erase planning ([`plan_erase`]) is pure and unit-tested; the USB I/O sequence
//! itself is a manual hardware check (ADR 0003 testing split).

use crate::constants::{FLASH_SECTOR_SIZE, SRAM_END};
use crate::device::{Device, DeviceError};
use crate::picoboot::Exclusivity;
use crate::uf2::Segment;
use thiserror::Error;

/// Bytes written (and read back) per PICOBOOT data transfer.
const WRITE_CHUNK: usize = FLASH_SECTOR_SIZE as usize;

/// Delay the device waits before rebooting, giving the host time to release the interface.
const REBOOT_DELAY_MS: u32 = 500;

/// Errors from the `load` operation.
#[derive(Debug, Error)]
pub enum LoadError {
    /// A transport-level operation failed.
    #[error(transparent)]
    Device(#[from] DeviceError),

    /// Read-back verification found bytes that differ from what was written.
    #[error("verify mismatch at {addr:#010x}")]
    VerifyMismatch {
        /// Address of the first chunk that did not match.
        addr: u32,
    },
}

/// Flash `segments` to the device and reboot. When `verify` is set, every written chunk is
/// read back and compared before rebooting.
pub fn load(device: &mut Device, segments: &[Segment], verify: bool) -> Result<(), LoadError> {
    let total: usize = segments.iter().map(|s| s.data.len()).sum();
    log::info!(
        "loading {total} byte(s) across {} segment(s)",
        segments.len()
    );

    device.exclusive_access(Exclusivity::Exclusive)?;
    device.exit_xip()?;

    for (start, size) in plan_erase(segments) {
        log::info!("erasing {start:#010x} ({size} bytes)");
        device.flash_erase(start, size)?;
    }

    for seg in segments {
        log::info!("writing {:#010x} ({} bytes)", seg.addr, seg.data.len());
        for (index, chunk) in seg.data.chunks(WRITE_CHUNK).enumerate() {
            let addr = seg.addr + (index * WRITE_CHUNK) as u32;
            device.flash_write(addr, chunk)?;
        }
    }

    if verify {
        device.enter_cmd_xip()?;
        for seg in segments {
            log::info!("verifying {:#010x} ({} bytes)", seg.addr, seg.data.len());
            for (index, chunk) in seg.data.chunks(WRITE_CHUNK).enumerate() {
                let addr = seg.addr + (index * WRITE_CHUNK) as u32;
                let read = device.flash_read(addr, chunk.len())?;
                if read != chunk {
                    return Err(LoadError::VerifyMismatch { addr });
                }
            }
        }
        log::info!("verify ok");
    }

    log::info!("rebooting");
    device.reboot(0, SRAM_END, REBOOT_DELAY_MS)?;
    Ok(())
}

/// Compute the sector-aligned, merged erase ranges covering all segments, as `(start, size)`.
///
/// Each segment's span is rounded out to whole sectors; overlapping or touching ranges are
/// merged so every sector is erased exactly once even when segments share a sector.
fn plan_erase(segments: &[Segment]) -> Vec<(u32, u32)> {
    let sector = FLASH_SECTOR_SIZE;
    let mut ranges: Vec<(u32, u32)> = segments
        .iter()
        .map(|s| (align_down(s.addr, sector), align_up(s.end(), sector)))
        .collect();
    ranges.sort_by_key(|r| r.0);

    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged.into_iter().map(|(s, e)| (s, e - s)).collect()
}

/// Round `x` down to a multiple of `align` (a power of two).
fn align_down(x: u32, align: u32) -> u32 {
    x & !(align - 1)
}

/// Round `x` up to a multiple of `align` (a power of two).
fn align_up(x: u32, align: u32) -> u32 {
    x.wrapping_add(align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::FLASH_START;

    fn seg(addr: u32, len: usize) -> Segment {
        Segment {
            addr,
            data: vec![0u8; len],
        }
    }

    #[test]
    fn sub_sector_segment_erases_one_sector() {
        assert_eq!(
            plan_erase(&[seg(FLASH_START, 100)]),
            vec![(FLASH_START, 4096)]
        );
    }

    #[test]
    fn segment_spanning_sector_boundary_erases_two() {
        assert_eq!(
            plan_erase(&[seg(FLASH_START, 5000)]),
            vec![(FLASH_START, 8192)]
        );
    }

    #[test]
    fn unaligned_start_rounds_down() {
        assert_eq!(
            plan_erase(&[seg(FLASH_START + 0x800, 100)]),
            vec![(FLASH_START, 4096)]
        );
    }

    #[test]
    fn adjacent_sectors_merge() {
        let segs = [seg(FLASH_START, 100), seg(FLASH_START + 4096, 100)];
        assert_eq!(plan_erase(&segs), vec![(FLASH_START, 8192)]);
    }

    #[test]
    fn segments_sharing_a_sector_erase_it_once() {
        let segs = [seg(FLASH_START, 100), seg(FLASH_START + 200, 100)];
        assert_eq!(plan_erase(&segs), vec![(FLASH_START, 4096)]);
    }

    #[test]
    fn distant_segments_stay_separate() {
        let segs = [seg(FLASH_START, 100), seg(FLASH_START + 0x10000, 100)];
        assert_eq!(
            plan_erase(&segs),
            vec![(FLASH_START, 4096), (FLASH_START + 0x10000, 4096)]
        );
    }

    #[test]
    fn sector_aligned_end_adds_no_extra_sector() {
        assert_eq!(
            plan_erase(&[seg(FLASH_START, 4096)]),
            vec![(FLASH_START, 4096)]
        );
    }
}
