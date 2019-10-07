extern crate structopt;

use crate::structopt::StructOpt;
use gc_nes_emulator::cartridge::load_cartridge_from_file;
use gc_nes_emulator::nes::Nes;
use std::path::Path;

#[macro_use]
extern crate log;

fn main() {
    let arguments = Arguments::from_args();

    std::env::set_var("RUST_LOG", "trace"); // TODO: Replace this with an argument
    env_logger::init();

    info!("Starting {} by {}, version {}...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_AUTHORS"), env!("CARGO_PKG_VERSION"));
    let cartridge = load_cartridge_from_file(Path::new(&arguments.file)).expect("File read error"); // TODO: Present a message to the user instead of crashing
    let nes = Nes::new(cartridge);
}

#[derive(StructOpt, Debug)]
pub struct Arguments {
    #[structopt(short = "f", long = "file")]
    file: String,
}
