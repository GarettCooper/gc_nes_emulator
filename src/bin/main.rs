use crate::structopt::StructOpt;
use gc_nes_emulator::cartridge::Cartridge;
use gc_nes_emulator::nes::Nes;
use minifb::{Window, Key, WindowOptions, Scale};
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};

#[macro_use]
extern crate log;

extern crate structopt;

const FRAME_DURATION: Duration = Duration::from_millis(16);

fn main() {
    let arguments = Arguments::from_args();
    env_logger::init();

    let scale = match arguments.scale {
        1 => Scale::X1,
        2 => Scale::X2,
        4 => Scale::X4,
        8 => Scale::X8,
        16 => Scale::X16,
        32 => Scale::X32,
        _ => Scale::X2
    };

    let mut window = Window::new(format!("gc_nes_emulator v{}", env!("CARGO_PKG_VERSION")).as_ref(), 256, 240, WindowOptions { scale, ..Default::default() }).expect("Error opening window");

    info!("Starting {} by {}, version {}...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_AUTHORS"), env!("CARGO_PKG_VERSION"));
    let cartridge = Cartridge::load_from_file(&arguments.file).expect("File read error"); // TODO: Present a message to the user instead of crashing
    let mut nes = Nes::new(cartridge);
    let buffer = nes.frame();
    window.update_with_buffer(buffer).expect("Error updating frame buffer");

    while window.is_open() {
        let timer = Instant::now();
        nes.update_controller_one(Some(get_controller_one_state(&window)));
        window.update_with_buffer(nes.frame()).expect("Error updating frame buffer");
        // This isn't exactly the most portable way of timing the frames but it will do for now
        if let Some(duration) = FRAME_DURATION.checked_sub(timer.elapsed()) {
            std::thread::sleep(duration)
        }
    }
}

#[derive(StructOpt, Debug)]
pub struct Arguments {
    #[structopt(short = "f", long = "file", parse(from_os_str))]
    file: PathBuf,
    #[structopt(short = "s", long = "scale", default_value = "2")]
    scale: u8,
}

/// Get the state of controller one as a input state byte
fn get_controller_one_state(window: &Window) -> u8 {
    // Get the appropriate controller state byte from the keys
    // TODO: Make these re-bindable
    return (window.is_key_down(Key::Space) as u8) |           // A
        (window.is_key_down(Key::LeftShift) as u8) << 1 |  // B
        (window.is_key_down(Key::Enter) as u8) << 2 |      // Select
        (window.is_key_down(Key::Escape) as u8) << 3 |     // Start
        (window.is_key_down(Key::W) as u8) << 4 |          // Up
        (window.is_key_down(Key::S) as u8) << 5 |          // Down
        (window.is_key_down(Key::A) as u8) << 6 |          // Left
        (window.is_key_down(Key::D) as u8) << 7;           // Right
}