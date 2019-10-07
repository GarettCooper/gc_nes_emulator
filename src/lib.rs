#![allow(clippy::needless_return)] // I prefer clarity of return

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate simple_error;
//extern crate gc_nes_emulator;
extern crate structopt;
#[macro_use]
extern crate log;

use crate::structopt::StructOpt;

pub mod cartridge;
pub mod nes;
