//! gc_nes_core is, as the name would suggest, the core of my Ninendo Entertainment System emulator.
//! It provides an interface for dependent crates to load and run NES ROMs, provide input, and extract
//! rendered image data. Audio is currently unsupported.
//!
//! ### Using the Emulator
//! ```rust, ignore
//! use gc_nes_core::cartridge::Cartridge;
//! use gc_nes_core::nes::Nes;
//!
//!
//! // Load a .nes file as a cartridge
//! let cartridge = Cartridge::load_from_file("/some/nes/rom.nes".as_ref()).expect("File read error");
//! // Create the NES with the cartridge loaded
//! let mut nes = Nes::new(cartridge);
//! // Run the NES until the next frame completes
//! let frame_buffer:&[u32; 61440] = nes.frame();
//! // Or run it cycle by cycle for a finer approach
//! nes.cycle();
//! // Provide input state:
//! nes.update_controller_one(Some(0b0001_0100));
//! nes.update_controller_two(None); // Disconnected controller
//!
//! ```

#![allow(clippy::needless_return)] // I prefer clarity of return
#![allow(clippy::verbose_bit_mask)] // For consistency in bit mask usage

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate simple_error;
#[macro_use]
extern crate log;

pub mod cartridge;
pub mod input;
pub mod nes;
