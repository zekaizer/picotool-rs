//! picotool-rs core: platform-agnostic layers for flashing an RP2040 over PICOBOOT.
//!
//! The crate is split into reusable layers (see [ADR 0001]):
//! [`constants`] centralizes device identity and flash geometry, [`uf2`] parses the
//! firmware container, [`picoboot`] encodes the wire protocol, [`detect`] decodes the flash
//! JEDEC id into a size, [`device`] performs USB I/O, and [`load`] ties them together into
//! the `load` operation.
//!
//! [ADR 0001]: https://github.com/zekaizer/picotool-rs/blob/main/docs/adr/0001-scope-flasher-first-rp2040-only.md

pub mod constants;
pub mod detect;
pub mod device;
pub mod load;
pub mod picoboot;
pub mod uf2;
