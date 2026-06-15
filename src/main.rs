fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("picotool-rs {} starting", env!("CARGO_PKG_VERSION"));

    // TODO: CLI and the `load` command (UF2 -> flash -> reboot).
    // See CONTEXT.md for terms and docs/adr for the decisions that shape this.
    anyhow::bail!("not yet implemented")
}
