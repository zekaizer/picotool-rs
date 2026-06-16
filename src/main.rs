//! picotool-rs binary: a subcommand CLI over the core library.
//!
//! Open dispatches at runtime (ADR 0002): `--fd` or `$TERMUX_USB_FD` selects an already-open
//! descriptor (the Android route); otherwise the BOOTSEL device is found by enumeration.
//!
//! On Termux, `--device <path>` (ADR 0004) lets the user run one command instead of wrapping
//! the flash in `termux-usb -E -e`: picotool-rs re-executes itself through `termux-usb` to
//! obtain the descriptor. The outer process passes the real arguments to the inner one
//! out-of-band (an env var) and sets a guard so the inner process ignores the device path
//! `termux-usb` appends as a trailing argument and does not re-exec again.

use std::os::fd::RawFd;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use env_logger::Env;

use picotool_rs::{device::Device, load, uf2};

/// Set by the outer process before re-execing through `termux-usb`; marks the inner run so it
/// reads its arguments from [`INNER_ARGS_ENV`] instead of argv and never re-execs again.
const REEXEC_GUARD_ENV: &str = "PICOTOOL_REEXEC";
/// Carries the inner process's arguments (NUL-separated) across the `termux-usb` re-exec.
const INNER_ARGS_ENV: &str = "PICOTOOL_INNER_ARGS";

#[derive(Parser)]
#[command(
    name = "picotool-rs",
    version,
    about = "Flash an RP2040 UF2 over USB PICOBOOT"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Flash a UF2 to the BOOTSEL device and reboot.
    Load(LoadArgs),
}

#[derive(Args)]
struct LoadArgs {
    /// Path to the UF2 file to flash.
    file: PathBuf,

    /// Read back and verify flash after writing.
    #[arg(long)]
    verify: bool,

    /// Flash the opened device even if its VID/PID is not the RP2040 BOOTSEL identity.
    #[arg(long)]
    any_device: bool,

    /// Flash through `termux-usb` for this USB device path, e.g. /dev/bus/usb/001/002 (Termux).
    ///
    /// picotool-rs re-executes itself via `termux-usb` to obtain the descriptor, so this is a
    /// one-command alternative to wrapping the flash in `termux-usb -E -e` (ADR 0004). Find the
    /// path with `termux-usb -l`.
    #[arg(long, value_name = "PATH")]
    device: Option<String>,

    /// Open this already-open USB file descriptor instead of enumerating (Linux/Android).
    ///
    /// Falls back to `$TERMUX_USB_FD` when not given.
    #[arg(long)]
    fd: Option<RawFd>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Inner (re-execed) run: take arguments from the env, not argv, since `termux-usb` appends
    // the device path as a trailing argument we must ignore (ADR 0004).
    let cli = match inner_args() {
        Some(argv) => Cli::parse_from(argv),
        None => Cli::parse(),
    };
    match cli.command {
        Command::Load(args) => run_load(args),
    }
}

fn run_load(args: LoadArgs) -> Result<()> {
    // Outer run with --device: hand off to termux-usb, which re-execs us with the descriptor in
    // the env. Returns only on failure (or on a platform without termux-usb).
    if let Some(path) = args.device.as_deref() {
        return reexec_via_termux(path);
    }

    let bytes = std::fs::read(&args.file)
        .with_context(|| format!("reading UF2 file {}", args.file.display()))?;
    let segments =
        uf2::parse(&bytes).with_context(|| format!("parsing UF2 file {}", args.file.display()))?;

    let fd = resolve_fd(args.fd);
    match fd {
        Some(fd) => log::info!("opening BOOTSEL device via fd {fd}"),
        None => log::info!("opening BOOTSEL device by enumeration"),
    }
    let mut device = Device::open(fd, !args.any_device).context("opening BOOTSEL device")?;

    load::load(&mut device, &segments, args.verify).context("flashing UF2")?;

    println!("flashed {} ({} bytes)", args.file.display(), bytes.len());
    Ok(())
}

/// Resolve the USB descriptor: the `--fd` flag wins, else `$TERMUX_USB_FD`, else none.
fn resolve_fd(flag: Option<RawFd>) -> Option<RawFd> {
    flag.or_else(|| {
        std::env::var("TERMUX_USB_FD")
            .ok()
            .and_then(|s| s.trim().parse().ok())
    })
}

/// The inner run's argument vector (program name + args), or `None` for a normal outer run.
///
/// Present only when [`REEXEC_GUARD_ENV`] is set: the real arguments arrive via
/// [`INNER_ARGS_ENV`] (NUL-separated) rather than argv, because `termux-usb` appends the device
/// path as a trailing argument that `clap` would otherwise reject.
fn inner_args() -> Option<Vec<String>> {
    std::env::var_os(REEXEC_GUARD_ENV)?;
    let prog = std::env::args()
        .next()
        .unwrap_or_else(|| "picotool-rs".to_string());
    let mut argv = vec![prog];
    let packed = std::env::var(INNER_ARGS_ENV).unwrap_or_default();
    if !packed.is_empty() {
        argv.extend(packed.split('\0').map(str::to_string));
    }
    Some(argv)
}

/// Re-exec through `termux-usb` to obtain the descriptor for `path`, replacing this process.
///
/// The outer arguments minus `--device <path>` are passed to the inner run out-of-band via
/// [`INNER_ARGS_ENV`], and [`REEXEC_GUARD_ENV`] marks it so it ignores the device path
/// `termux-usb` appends and reads the descriptor from `$TERMUX_USB_FD` (ADR 0004).
#[cfg(target_os = "android")]
fn reexec_via_termux(path: &str) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let exe = std::env::current_exe().context("locating own executable for re-exec")?;
    let inner = strip_device_arg(std::env::args().skip(1));
    log::info!("re-executing through termux-usb for device {path}");

    // `exec` only returns if it fails; on success the image is replaced and never comes back.
    let err = std::process::Command::new("termux-usb")
        .args(["-r", "-E", "-e"])
        .arg(&exe)
        .arg(path)
        .env(REEXEC_GUARD_ENV, "1")
        .env(INNER_ARGS_ENV, inner.join("\0"))
        .exec();
    Err(err).context("executing termux-usb (is the termux-api package installed?)")
}

/// Stub on platforms without `termux-usb`; `--device` is a Termux/Android-only route.
#[cfg(not(target_os = "android"))]
fn reexec_via_termux(_path: &str) -> Result<()> {
    anyhow::bail!("--device is only supported on Termux/Android; use --fd or enumeration")
}

/// Drop `--device <path>` (and the `--device=<path>` form) from an argument iterator.
#[cfg(any(target_os = "android", test))]
fn strip_device_arg(args: impl Iterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip_value = false;
    for arg in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        if arg == "--device" {
            skip_value = true;
        } else if !arg.starts_with("--device=") {
            out.push(arg);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_device_arg;

    fn strip(args: &[&str]) -> Vec<String> {
        strip_device_arg(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn removes_device_flag_and_its_value() {
        assert_eq!(
            strip(&[
                "load",
                "fw.uf2",
                "--device",
                "/dev/bus/usb/001/002",
                "--verify"
            ]),
            vec!["load", "fw.uf2", "--verify"]
        );
    }

    #[test]
    fn removes_device_equals_form() {
        assert_eq!(
            strip(&["load", "fw.uf2", "--device=/dev/bus/usb/001/002"]),
            vec!["load", "fw.uf2"]
        );
    }

    #[test]
    fn keeps_other_args_when_no_device() {
        assert_eq!(
            strip(&["load", "fw.uf2", "--verify", "--any-device"]),
            vec!["load", "fw.uf2", "--verify", "--any-device"]
        );
    }
}
