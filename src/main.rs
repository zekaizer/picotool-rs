//! picotool-rs binary: a subcommand CLI over the core library.
//!
//! Open dispatches at runtime (ADR 0002): `--fd` or `$TERMUX_USB_FD` selects an already-open
//! descriptor (the Android route); otherwise the BOOTSEL device is found by enumeration.

use std::os::fd::RawFd;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use env_logger::Env;

use picotool_rs::{device::Device, load, uf2};

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

    /// Open this already-open USB file descriptor instead of enumerating (Linux/Android).
    ///
    /// Falls back to `$TERMUX_USB_FD` when not given.
    #[arg(long)]
    fd: Option<RawFd>,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    match cli.command {
        Command::Load(args) => run_load(args),
    }
}

fn run_load(args: LoadArgs) -> Result<()> {
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
