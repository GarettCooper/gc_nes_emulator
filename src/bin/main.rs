use crate::structopt::StructOpt;
use gc_nes_emulator::cartridge::Cartridge;
use gc_nes_emulator::nes::{Nes, NesInputDevice};
use minifb::{Window, Key, WindowOptions};
use std::path::Path;

#[macro_use]
extern crate log;

extern crate structopt;

fn main() {
    let arguments = Arguments::from_args();

    std::env::set_var("RUST_LOG", "trace"); // TODO: Replace this with an argument
    env_logger::init();

    // TODO: Setup window scaling
    let window = Window::new("Test", 256, 240, WindowOptions::default()).expect("Error opening window");

    info!("Starting {} by {}, version {}...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_AUTHORS"), env!("CARGO_PKG_VERSION"));
    let mut cartridge = Cartridge::load_from_file(Path::new(&arguments.file)).expect("File read error"); // TODO: Present a message to the user instead of crashing
    let mut controller = MiniFbNesController { shift_register: 0x00, window: &window };
    let nes = Nes::new(cartridge, &mut controller);
}

#[derive(StructOpt, Debug)]
pub struct Arguments {
    #[structopt(short = "f", long = "file")]
    file: String,
}

struct MiniFbNesController<'a> {
    /// Shift register that stores the button information
    shift_register: u8,
    /// minifb Window that the controller will read key inputs from
    window: &'a Window,
}

impl NesInputDevice for MiniFbNesController<'_> {
    fn latch(&mut self, latch: u8) {
        // Set the bits in the shift register to match the appropriate buttons
        // TODO: Make these re-bindable
        self.shift_register =
            (self.window.is_key_down(Key::Space) as u8) |               // A
                (self.window.is_key_down(Key::LeftShift) as u8) << 1 |  // B
                (self.window.is_key_down(Key::Enter) as u8) << 2 |      // Select
                (self.window.is_key_down(Key::Escape) as u8) << 3 |     // Start
                (self.window.is_key_down(Key::W) as u8) << 4 |          // Up
                (self.window.is_key_down(Key::S) as u8) << 5 |          // Down
                (self.window.is_key_down(Key::A) as u8) << 6 |          // Left
                (self.window.is_key_down(Key::D) as u8) << 7;           // Right
    }

    fn poll(&mut self, bus: u8) -> u8 {
        // Select only the last bit of the
        let result = self.shift_register & 0x01;
        // Get the next bit in the shift register
        self.shift_register >>= 1;
        // Set the new bit to 1, which is returned after 8 polls on official NES controllers
        self.shift_register |= 0x80;
        // Return the result bit with the top 5 bits as the previous byte on the bus
        return result | (bus & 0xf4);
    }
}