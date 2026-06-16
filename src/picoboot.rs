//! PICOBOOT command encoding.
//!
//! Every PICOBOOT command is a fixed 32-byte packet sent on the bulk OUT endpoint. The
//! layout (RP2040 datasheet, USB bootloader / PICOBOOT; see `docs/references.md`):
//!
//! ```text
//! offset size field
//! 0      4    magic            0x431FD10B
//! 4      4    token            host-chosen, echoed back in the status response
//! 8      1    cmd_id           command id; bit 7 set means the data phase is IN
//! 9      1    cmd_size         number of bytes used in args (0..=16)
//! 10     2    reserved         zero
//! 12     4    transfer_length  bytes in the data phase
//! 16     16   args             command-specific
//! ```
//!
//! This module is pure: it turns a [`Command`] into the 32 wire bytes and reports the
//! command's [`DataPhase`]. The bulk framing around it (sending the packet, the data
//! transfer, and the zero-length acknowledgement) lives in [`crate::device`].

use crate::constants::PICOBOOT_MAGIC;

// PICOBOOT command ids. Bit 7 set marks a device-to-host (IN) data phase.
const CMD_EXCLUSIVE_ACCESS: u8 = 0x01;
const CMD_REBOOT: u8 = 0x02;
const CMD_FLASH_ERASE: u8 = 0x03;
const CMD_READ: u8 = 0x84;
const CMD_WRITE: u8 = 0x05;
const CMD_EXIT_XIP: u8 = 0x06;
const CMD_ENTER_CMD_XIP: u8 = 0x07;

/// Exclusivity level for [`Command::ExclusiveAccess`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Exclusivity {
    /// Share access with the mass-storage interface.
    NotExclusive = 0,
    /// Take exclusive access, locking out mass storage.
    Exclusive = 1,
    /// Take exclusive access and eject the mass-storage volume.
    ExclusiveAndEject = 2,
}

/// The data transfer that follows a command packet, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataPhase {
    /// No data phase; the command is acknowledged directly.
    None,
    /// `len` bytes flow host-to-device on the bulk OUT endpoint.
    HostToDevice(u32),
    /// `len` bytes flow device-to-host on the bulk IN endpoint.
    DeviceToHost(u32),
}

/// A PICOBOOT command, covering the subset the `load` operation needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Claim (or release) access to the device relative to mass storage.
    ExclusiveAccess(Exclusivity),
    /// Leave XIP mode so flash can be erased and written.
    ExitXip,
    /// Re-enter command-based XIP so flash can be read back.
    EnterCmdXip,
    /// Erase a flash region; `addr` and `size` must be sector-aligned.
    FlashErase {
        /// Flash start address.
        addr: u32,
        /// Number of bytes to erase.
        size: u32,
    },
    /// Write `size` bytes (carried in the following data phase) to memory at `addr`.
    Write {
        /// Destination address.
        addr: u32,
        /// Number of bytes in the data phase.
        size: u32,
    },
    /// Read `size` bytes from memory at `addr` (returned in the following data phase).
    Read {
        /// Source address.
        addr: u32,
        /// Number of bytes to read.
        size: u32,
    },
    /// Reboot the device. `pc == 0` requests a normal reboot to the flashed application.
    Reboot {
        /// Entry point, or 0 for a normal reboot.
        pc: u32,
        /// Initial stack pointer.
        sp: u32,
        /// Delay before rebooting, in milliseconds.
        delay_ms: u32,
    },
}

impl Command {
    /// The command id byte (bit 7 set for IN-direction commands).
    fn cmd_id(&self) -> u8 {
        match self {
            Command::ExclusiveAccess(_) => CMD_EXCLUSIVE_ACCESS,
            Command::ExitXip => CMD_EXIT_XIP,
            Command::EnterCmdXip => CMD_ENTER_CMD_XIP,
            Command::FlashErase { .. } => CMD_FLASH_ERASE,
            Command::Write { .. } => CMD_WRITE,
            Command::Read { .. } => CMD_READ,
            Command::Reboot { .. } => CMD_REBOOT,
        }
    }

    /// Number of bytes used in the args field.
    fn cmd_size(&self) -> u8 {
        match self {
            Command::ExclusiveAccess(_) => 1,
            Command::ExitXip | Command::EnterCmdXip => 0,
            Command::FlashErase { .. } | Command::Write { .. } | Command::Read { .. } => 8,
            Command::Reboot { .. } => 12,
        }
    }

    /// The data phase that follows this command's packet.
    pub fn data_phase(&self) -> DataPhase {
        match *self {
            Command::Write { size, .. } => DataPhase::HostToDevice(size),
            Command::Read { size, .. } => DataPhase::DeviceToHost(size),
            _ => DataPhase::None,
        }
    }

    /// Number of bytes in the data phase (0 if none).
    fn transfer_length(&self) -> u32 {
        match self.data_phase() {
            DataPhase::None => 0,
            DataPhase::HostToDevice(n) | DataPhase::DeviceToHost(n) => n,
        }
    }

    /// Fill the 16-byte args region for this command.
    fn write_args(&self, args: &mut [u8]) {
        match *self {
            Command::ExclusiveAccess(level) => args[0] = level as u8,
            Command::FlashErase { addr, size }
            | Command::Write { addr, size }
            | Command::Read { addr, size } => {
                args[0..4].copy_from_slice(&addr.to_le_bytes());
                args[4..8].copy_from_slice(&size.to_le_bytes());
            }
            Command::Reboot { pc, sp, delay_ms } => {
                args[0..4].copy_from_slice(&pc.to_le_bytes());
                args[4..8].copy_from_slice(&sp.to_le_bytes());
                args[8..12].copy_from_slice(&delay_ms.to_le_bytes());
            }
            Command::ExitXip | Command::EnterCmdXip => {}
        }
    }

    /// Encode the command into its 32 wire bytes, stamping `token` into the header.
    pub fn encode(&self, token: u32) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0..4].copy_from_slice(&PICOBOOT_MAGIC.to_le_bytes());
        buf[4..8].copy_from_slice(&token.to_le_bytes());
        buf[8] = self.cmd_id();
        buf[9] = self.cmd_size();
        // bytes 10..12 are reserved and left zero.
        buf[12..16].copy_from_slice(&self.transfer_length().to_le_bytes());
        self.write_args(&mut buf[16..32]);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Spec-derived golden vectors. These are computed from the documented 32-byte layout,
    // not captured from picotool; reconciling them against a usbmon capture is a follow-up.

    #[test]
    fn exit_xip_golden() {
        #[rustfmt::skip]
        let expected = [
            0x0B, 0xD1, 0x1F, 0x43, // magic
            0x00, 0x00, 0x00, 0x00, // token
            0x06, 0x00, 0x00, 0x00, // cmd_id=EXIT_XIP, cmd_size=0, reserved
            0x00, 0x00, 0x00, 0x00, // transfer_length=0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(Command::ExitXip.encode(0), expected);
    }

    #[test]
    fn enter_cmd_xip_golden() {
        let enc = Command::EnterCmdXip.encode(0);
        assert_eq!(enc[8], 0x07);
        assert_eq!(enc[9], 0x00);
        assert_eq!(&enc[12..16], &[0, 0, 0, 0]);
    }

    #[test]
    fn exclusive_access_golden() {
        #[rustfmt::skip]
        let expected = [
            0x0B, 0xD1, 0x1F, 0x43,
            0x00, 0x00, 0x00, 0x00,
            0x01, 0x01, 0x00, 0x00, // cmd_id=EXCLUSIVE_ACCESS, cmd_size=1
            0x00, 0x00, 0x00, 0x00, // transfer_length=0
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // args[0]=Exclusive
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(
            Command::ExclusiveAccess(Exclusivity::Exclusive).encode(0),
            expected
        );
        assert_eq!(Exclusivity::ExclusiveAndEject as u8, 2);
        assert_eq!(Exclusivity::NotExclusive as u8, 0);
    }

    #[test]
    fn flash_erase_golden() {
        #[rustfmt::skip]
        let expected = [
            0x0B, 0xD1, 0x1F, 0x43,
            0x78, 0x56, 0x34, 0x12, // token=0x12345678
            0x03, 0x08, 0x00, 0x00, // cmd_id=FLASH_ERASE, cmd_size=8
            0x00, 0x00, 0x00, 0x00, // transfer_length=0
            0x00, 0x00, 0x00, 0x10, // addr=0x10000000
            0x00, 0x10, 0x00, 0x00, // size=0x1000
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let cmd = Command::FlashErase {
            addr: 0x1000_0000,
            size: 0x1000,
        };
        assert_eq!(cmd.encode(0x1234_5678), expected);
        assert_eq!(cmd.data_phase(), DataPhase::None);
    }

    #[test]
    fn write_golden() {
        #[rustfmt::skip]
        let expected = [
            0x0B, 0xD1, 0x1F, 0x43,
            0x01, 0x00, 0x00, 0x00, // token=1
            0x05, 0x08, 0x00, 0x00, // cmd_id=WRITE, cmd_size=8
            0x00, 0x01, 0x00, 0x00, // transfer_length=256
            0x00, 0x00, 0x00, 0x10, // addr=0x10000000
            0x00, 0x01, 0x00, 0x00, // size=256
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let cmd = Command::Write {
            addr: 0x1000_0000,
            size: 256,
        };
        assert_eq!(cmd.encode(1), expected);
        assert_eq!(cmd.data_phase(), DataPhase::HostToDevice(256));
    }

    #[test]
    fn read_sets_in_bit_and_device_to_host_phase() {
        let cmd = Command::Read {
            addr: 0x1000_0000,
            size: 256,
        };
        let enc = cmd.encode(0);
        assert_eq!(enc[8], 0x84, "READ must set the IN direction bit");
        assert_eq!(enc[9], 0x08);
        assert_eq!(&enc[12..16], &256u32.to_le_bytes());
        assert_eq!(&enc[16..20], &0x1000_0000u32.to_le_bytes());
        assert_eq!(cmd.data_phase(), DataPhase::DeviceToHost(256));
    }

    #[test]
    fn reboot_golden() {
        #[rustfmt::skip]
        let expected = [
            0x0B, 0xD1, 0x1F, 0x43,
            0x00, 0x00, 0x00, 0x00,
            0x02, 0x0C, 0x00, 0x00, // cmd_id=REBOOT, cmd_size=12
            0x00, 0x00, 0x00, 0x00, // transfer_length=0
            0x00, 0x00, 0x00, 0x00, // pc=0
            0x00, 0x20, 0x04, 0x20, // sp=0x20042000
            0xF4, 0x01, 0x00, 0x00, // delay_ms=500
            0x00, 0x00, 0x00, 0x00,
        ];
        let cmd = Command::Reboot {
            pc: 0,
            sp: 0x2004_2000,
            delay_ms: 500,
        };
        assert_eq!(cmd.encode(0), expected);
    }

    #[test]
    fn magic_is_first_word_little_endian() {
        let enc = Command::ExitXip.encode(0);
        assert_eq!(
            u32::from_le_bytes([enc[0], enc[1], enc[2], enc[3]]),
            PICOBOOT_MAGIC
        );
    }
}
