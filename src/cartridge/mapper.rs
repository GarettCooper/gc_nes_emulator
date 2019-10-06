use super::*;

/// Returns a boxed mapper based on the mapper_id argument
pub(super) fn get_mapper(mapper_id: u16, submapper_id: u8) -> Result<Box<dyn Mapper>, Box<dyn Error>> {
    debug!("Getting mapper with id {}, submapper {}", mapper_id, submapper_id);
    match mapper_id {
        0 => Ok(Box::new(Mapper000 {})),
        _ => bail!("Mapper ID not found!"),
    }
}

pub(super) trait Mapper {
    fn program_rom_read(&self, program_rom: &[u8], address: u16) -> u8;
    fn character_rom_read(&self, character_rom: &[u8], address: u16) -> u8;
}

pub(super) struct Mapper000 {}

impl Mapper for Mapper000 {
    fn program_rom_read(&self, program_rom: &[u8], address: u16) -> u8 {
        unimplemented!()
    }

    fn character_rom_read(&self, character_rom: &[u8], address: u16) -> u8 {
        unimplemented!()
    }
}
