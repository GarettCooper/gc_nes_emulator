use std::fs::File;

///Loads a cartridge and returns
pub (crate) fn load_cartridge(game_file: & File) -> Box<dyn NesCartridge>{
    unimplemented!()
}

pub (crate) trait NesCartridge{

}