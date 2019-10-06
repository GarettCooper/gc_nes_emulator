#![allow(clippy::needless_return)] // I prefer clarity of return

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate simple_error;
#[macro_use]
extern crate structopt;
#[macro_use]
extern crate log;

mod cartridge;
mod nes;

use crate::structopt::StructOpt;
use std::path::Path;

fn main() {
    let arguments = Arguments::from_args();

    std::env::set_var("RUST_LOG", "trace"); // TODO: Replace this with an argument
    env_logger::init();

    info!("Starting {} by {}, version {}...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_AUTHORS"), env!("CARGO_PKG_VERSION"));
    let result = cartridge::load_cartridge_from_file(Path::new(&arguments.file)).expect("msg: &str");
}

#[derive(StructOpt, Debug)]
struct Arguments {
    #[structopt(short = "f", long = "file")]
    file: String,
}
