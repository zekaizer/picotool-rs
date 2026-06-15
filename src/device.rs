//! USB transport for the PICOBOOT interface (nusb, blocking).
//!
//! This is the one place that touches USB (ADR 0002). Opening is the only platform-varying
//! part: [`Device::open_enumerate`] finds the BOOTSEL device by VID/PID, while
//! [`Device::open_fd`] wraps a descriptor handed over by `termux-usb` and is only functional
//! on Linux/Android (`nusb::Device::from_fd` does not exist elsewhere); on other platforms it
//! returns [`DeviceError::FdUnsupported`], so macOS compiles to an enumerate-only binary.
//!
//! Everything after opening is shared. Each PICOBOOT command follows the same bulk framing:
//! a 32-byte command packet on bulk OUT, an optional data phase, then a zero-length
//! acknowledgement transfer in the direction opposite the data (IN for OUT/no-data commands,
//! OUT for IN commands).

use std::os::fd::RawFd;
use std::time::Duration;

#[cfg(any(target_os = "android", target_os = "linux"))]
use std::os::fd::{FromRawFd, OwnedFd};

use nusb::MaybeFuture;
use nusb::descriptors::TransferType;
use nusb::transfer::{
    Buffer, Bulk, ControlOut, ControlType, Direction, In, Out, Recipient, TransferError,
};
use thiserror::Error;

use crate::constants::{PICOBOOT_IF_RESET, PICOBOOT_INTERFACE_CLASS};
#[cfg(not(target_os = "android"))]
use crate::constants::{PRODUCT_ID, VENDOR_ID};
use crate::picoboot::{Command, Exclusivity};

/// Timeout for a command packet, data chunk, or acknowledgement.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
/// Timeout for a flash-erase acknowledgement; erasing a large region is synchronous and slow.
const ERASE_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors from the USB transport layer.
#[derive(Debug, Error)]
pub enum DeviceError {
    /// No BOOTSEL device was found during enumeration.
    #[error("no BOOTSEL device found (VID {vid:#06x} PID {pid:#06x}); is it in bootloader mode?")]
    NotFound {
        /// The VID searched for.
        vid: u16,
        /// The PID searched for.
        pid: u16,
    },

    /// The device exposes no PICOBOOT (vendor-class) interface.
    #[error("device has no PICOBOOT (vendor) interface")]
    NoPicobootInterface,

    /// The PICOBOOT interface lacks a bulk IN/OUT endpoint pair.
    #[error("PICOBOOT interface is missing a bulk IN/OUT endpoint pair")]
    NoBulkEndpoints,

    /// Opening from a file descriptor was requested on a platform that does not support it.
    #[error("opening a device from a file descriptor is only supported on Linux/Android")]
    FdUnsupported,

    /// VID/PID enumeration is unavailable (non-rooted Android); a descriptor must be supplied.
    #[error("device enumeration is unavailable on this platform; supply --fd or $TERMUX_USB_FD")]
    EnumerateUnsupported,

    /// The device's active configuration could not be read.
    #[error("cannot read active configuration: {0}")]
    Config(String),

    /// A USB control or setup operation failed.
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    /// A bulk or control transfer failed.
    #[error("USB transfer failed during {op}: {source}")]
    Transfer {
        /// Label of the operation that failed.
        op: &'static str,
        /// The underlying transfer error.
        #[source]
        source: TransferError,
    },

    /// A transfer moved fewer bytes than expected.
    #[error("short transfer during {op}: expected {expected} bytes, got {actual}")]
    ShortTransfer {
        /// Label of the operation.
        op: &'static str,
        /// Bytes expected.
        expected: usize,
        /// Bytes actually transferred.
        actual: usize,
    },
}

/// An opened PICOBOOT connection to a BOOTSEL device.
pub struct Device {
    interface: nusb::Interface,
    ep_out: nusb::Endpoint<Bulk, Out>,
    ep_in: nusb::Endpoint<Bulk, In>,
    if_num: u8,
    in_max_packet: usize,
    token: u32,
}

impl Device {
    /// Open the device, dispatching at runtime on whether a descriptor was supplied (ADR 0002).
    ///
    /// With `fd`, wrap that descriptor — the only route on non-rooted Android. Without it,
    /// enumerate by VID/PID, as on desktop hosts.
    pub fn open(fd: Option<RawFd>) -> Result<Device, DeviceError> {
        match fd {
            Some(fd) => Device::open_fd(fd),
            None => Device::open_enumerate(),
        }
    }

    /// Find and open the BOOTSEL device by enumerating USB and matching VID/PID.
    ///
    /// Enumeration is unavailable on non-rooted Android (no access to `/dev/bus/usb`); the
    /// Android build returns [`DeviceError::EnumerateUnsupported`] and must use [`open_fd`].
    ///
    /// [`open_fd`]: Device::open_fd
    pub fn open_enumerate() -> Result<Device, DeviceError> {
        #[cfg(not(target_os = "android"))]
        {
            let info = nusb::list_devices()
                .wait()?
                .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
                .ok_or(DeviceError::NotFound {
                    vid: VENDOR_ID,
                    pid: PRODUCT_ID,
                })?;
            let device = info.open().wait()?;
            Device::from_nusb(device)
        }
        #[cfg(target_os = "android")]
        {
            Err(DeviceError::EnumerateUnsupported)
        }
    }

    /// Open the BOOTSEL device from an already-open usbfs file descriptor.
    ///
    /// Ownership of `fd` transfers to the returned device. Supported on Linux/Android only.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    pub fn open_fd(fd: RawFd) -> Result<Device, DeviceError> {
        // SAFETY: the caller (termux-usb -E via $TERMUX_USB_FD, or --fd) hands over an open
        // usbfs descriptor whose ownership we take for the device's lifetime.
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let device = nusb::Device::from_fd(owned).wait()?;
        Device::from_nusb(device)
    }

    /// Stub on platforms without `from_fd`; always returns [`DeviceError::FdUnsupported`].
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    pub fn open_fd(_fd: RawFd) -> Result<Device, DeviceError> {
        Err(DeviceError::FdUnsupported)
    }

    /// Claim the PICOBOOT interface, open its bulk endpoints, and reset it to a clean state.
    fn from_nusb(device: nusb::Device) -> Result<Device, DeviceError> {
        let (if_num, out_addr, in_addr) = find_picoboot(&device)?;
        let interface = device.claim_interface(if_num).wait()?;
        let ep_out = interface.endpoint::<Bulk, Out>(out_addr)?;
        let ep_in = interface.endpoint::<Bulk, In>(in_addr)?;
        let in_max_packet = ep_in.max_packet_size();

        let mut dev = Device {
            interface,
            ep_out,
            ep_in,
            if_num,
            in_max_packet,
            token: 0,
        };
        dev.reset_interface()?;
        Ok(dev)
    }

    /// Reset the PICOBOOT interface via the vendor control request.
    pub fn reset_interface(&mut self) -> Result<(), DeviceError> {
        self.interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Interface,
                    request: PICOBOOT_IF_RESET,
                    value: 0,
                    index: u16::from(self.if_num),
                    data: &[],
                },
                COMMAND_TIMEOUT,
            )
            .wait()
            .map_err(|source| DeviceError::Transfer {
                op: "interface reset",
                source,
            })
    }

    /// Take exclusive access to the device, locking out the mass-storage interface.
    pub fn exclusive_access(&mut self, level: Exclusivity) -> Result<(), DeviceError> {
        self.exec_no_data(
            Command::ExclusiveAccess(level),
            "exclusive access",
            COMMAND_TIMEOUT,
        )
    }

    /// Leave XIP mode so flash can be erased and written.
    pub fn exit_xip(&mut self) -> Result<(), DeviceError> {
        self.exec_no_data(Command::ExitXip, "exit xip", COMMAND_TIMEOUT)
    }

    /// Re-enter command XIP so flash can be read back.
    pub fn enter_cmd_xip(&mut self) -> Result<(), DeviceError> {
        self.exec_no_data(Command::EnterCmdXip, "enter cmd xip", COMMAND_TIMEOUT)
    }

    /// Erase a flash region. `addr` and `size` must be sector-aligned.
    pub fn flash_erase(&mut self, addr: u32, size: u32) -> Result<(), DeviceError> {
        self.exec_no_data(
            Command::FlashErase { addr, size },
            "flash erase",
            ERASE_TIMEOUT,
        )
    }

    /// Write `data` to flash at `addr`. The region must already be erased.
    pub fn flash_write(&mut self, addr: u32, data: &[u8]) -> Result<(), DeviceError> {
        let token = self.next_token();
        let cmd = Command::Write {
            addr,
            size: data.len() as u32,
        };
        self.write_out(
            Buffer::from(cmd.encode(token)),
            "write command",
            COMMAND_TIMEOUT,
        )?;
        self.write_out(Buffer::from(data), "write data", COMMAND_TIMEOUT)?;
        self.read_ack("write ack", COMMAND_TIMEOUT)
    }

    /// Read `len` bytes from flash at `addr`.
    pub fn flash_read(&mut self, addr: u32, len: usize) -> Result<Vec<u8>, DeviceError> {
        let token = self.next_token();
        let cmd = Command::Read {
            addr,
            size: len as u32,
        };
        self.write_out(
            Buffer::from(cmd.encode(token)),
            "read command",
            COMMAND_TIMEOUT,
        )?;
        let data = self.read_in(len, "read data", COMMAND_TIMEOUT)?;
        self.write_ack("read ack")?;
        Ok(data)
    }

    /// Reboot the device. `pc == 0` requests a normal reboot to the flashed application.
    pub fn reboot(&mut self, pc: u32, sp: u32, delay_ms: u32) -> Result<(), DeviceError> {
        let token = self.next_token();
        let cmd = Command::Reboot { pc, sp, delay_ms };
        self.write_out(Buffer::from(cmd.encode(token)), "reboot", COMMAND_TIMEOUT)?;
        // The device acknowledges, then reboots after the delay; a missing ack (it may
        // disconnect first) is not a failure once the reboot command is accepted.
        if let Err(e) = self.read_ack("reboot ack", COMMAND_TIMEOUT) {
            log::debug!("reboot ack not received (device likely already rebooting): {e}");
        }
        Ok(())
    }

    /// Run a command that has no data phase: send the packet, read the zero-length ack.
    fn exec_no_data(
        &mut self,
        cmd: Command,
        op: &'static str,
        ack_timeout: Duration,
    ) -> Result<(), DeviceError> {
        let token = self.next_token();
        self.write_out(Buffer::from(cmd.encode(token)), op, COMMAND_TIMEOUT)?;
        self.read_ack(op, ack_timeout)
    }

    /// Send a bulk OUT transfer and require all bytes to be sent.
    fn write_out(
        &mut self,
        buf: Buffer,
        op: &'static str,
        timeout: Duration,
    ) -> Result<(), DeviceError> {
        let expected = buf.len();
        let completion = self.ep_out.transfer_blocking(buf, timeout);
        completion
            .status
            .map_err(|source| DeviceError::Transfer { op, source })?;
        if completion.actual_len != expected {
            return Err(DeviceError::ShortTransfer {
                op,
                expected,
                actual: completion.actual_len,
            });
        }
        Ok(())
    }

    /// Read a bulk IN data transfer of at least `len` bytes, returning the first `len`.
    fn read_in(
        &mut self,
        len: usize,
        op: &'static str,
        timeout: Duration,
    ) -> Result<Vec<u8>, DeviceError> {
        // nusb requires the requested length to be a nonzero multiple of the max packet size.
        let requested = len.div_ceil(self.in_max_packet).max(1) * self.in_max_packet;
        let completion = self
            .ep_in
            .transfer_blocking(Buffer::new(requested), timeout);
        completion
            .status
            .map_err(|source| DeviceError::Transfer { op, source })?;
        if completion.actual_len < len {
            return Err(DeviceError::ShortTransfer {
                op,
                expected: len,
                actual: completion.actual_len,
            });
        }
        let mut data = completion.buffer.into_vec();
        data.truncate(len);
        Ok(data)
    }

    /// Read the zero-length acknowledgement on bulk IN (OUT/no-data commands).
    fn read_ack(&mut self, op: &'static str, timeout: Duration) -> Result<(), DeviceError> {
        let completion = self
            .ep_in
            .transfer_blocking(Buffer::new(self.in_max_packet), timeout);
        completion
            .status
            .map_err(|source| DeviceError::Transfer { op, source })?;
        Ok(())
    }

    /// Write the zero-length acknowledgement on bulk OUT (IN commands).
    fn write_ack(&mut self, op: &'static str) -> Result<(), DeviceError> {
        let completion = self
            .ep_out
            .transfer_blocking(Buffer::new(0), COMMAND_TIMEOUT);
        completion
            .status
            .map_err(|source| DeviceError::Transfer { op, source })?;
        Ok(())
    }

    /// Next host token, used to correlate a command with its status.
    fn next_token(&mut self) -> u32 {
        self.token = self.token.wrapping_add(1);
        self.token
    }
}

/// Locate the PICOBOOT interface number and its bulk OUT/IN endpoint addresses.
fn find_picoboot(device: &nusb::Device) -> Result<(u8, u8, u8), DeviceError> {
    let config = device
        .active_configuration()
        .map_err(|e| DeviceError::Config(e.to_string()))?;

    for interface in config.interfaces() {
        let alt = interface.first_alt_setting();
        if alt.class() != PICOBOOT_INTERFACE_CLASS {
            continue;
        }
        let mut out_addr = None;
        let mut in_addr = None;
        for ep in alt.endpoints() {
            if ep.transfer_type() != TransferType::Bulk {
                continue;
            }
            match ep.direction() {
                Direction::Out => out_addr = Some(ep.address()),
                Direction::In => in_addr = Some(ep.address()),
            }
        }
        return match (out_addr, in_addr) {
            (Some(out), Some(inp)) => Ok((interface.interface_number(), out, inp)),
            _ => Err(DeviceError::NoBulkEndpoints),
        };
    }
    Err(DeviceError::NoPicobootInterface)
}
