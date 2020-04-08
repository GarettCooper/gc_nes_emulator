#![allow(clippy::needless_return)] // I prefer clarity of return
#![allow(clippy::verbose_bit_mask)] // For consistency in bit mask usage

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate simple_error;
#[macro_use]
extern crate log;

pub mod cartridge;
pub mod nes;
pub mod input;