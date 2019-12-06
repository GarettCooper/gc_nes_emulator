use crate::structopt::StructOpt;
use gc_nes_emulator::cartridge::Cartridge;
use gc_nes_emulator::nes::Nes;
use gc_nes_emulator::input::{NesInput, NesInputDevice};
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
    let mut controller = MiniFbNesController::new(&window);
    let mut nes = Nes::new(cartridge);
    nes.connect_controller_one(NesInput::Connected(&mut controller));
}

#[derive(StructOpt, Debug)]
pub struct Arguments {
    #[structopt(short = "f", long = "file")]
    file: String,
}

struct MiniFbNesController<'a> {
    /// Shift register that stores the button information
    shift_register: u8,
    /// Controller latch that reloads shift register when true
    reload_latch: bool,
    /// minifb Window that the controller will read key inputs from
    window: &'a Window,
}

impl<'a> MiniFbNesController<'a> {
    /// Creates a new instance of a MiniFbNesController
    fn new(minifb_window: &'a Window) -> Self {
        MiniFbNesController {
            shift_register: 0x00,
            reload_latch: false,
            window: minifb_window,
        }
    }

    /// Reloads the shift register based on the reload latch
    fn reload_shift_register(&mut self) {
        if self.reload_latch {
            // Set the bits in the shift register to match the appropriate buttons
            // TODO: Make these re-bindable
            self.shift_register =
                (self.window.is_key_down(Key::Space) as u8) |           // A
                    (self.window.is_key_down(Key::LeftShift) as u8) << 1 |  // B
                    (self.window.is_key_down(Key::Enter) as u8) << 2 |      // Select
                    (self.window.is_key_down(Key::Escape) as u8) << 3 |     // Start
                    (self.window.is_key_down(Key::W) as u8) << 4 |          // Up
                    (self.window.is_key_down(Key::S) as u8) << 5 |          // Down
                    (self.window.is_key_down(Key::A) as u8) << 6 |          // Left
                    (self.window.is_key_down(Key::D) as u8) << 7;           // Right
        }
    }
}

impl NesInputDevice for MiniFbNesController<'_> {
    fn latch(&mut self, latch: u8) {
        self.reload_latch = latch & 0x01 == 0x01;
    }

    fn poll(&mut self, bus: u8) -> u8 {
        self.reload_shift_register();
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