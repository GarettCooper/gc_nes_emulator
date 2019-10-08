use super::*;

/// Returns a boxed mapper based on the mapper_id argument
pub(super) fn get_mapper(mapper_id: u16, submapper_id: u8) -> Result<Box<dyn Mapper>, Box<dyn Error>> {
    debug!("Getting mapper with id {}, submapper {}", mapper_id, submapper_id);
    match mapper_id {
        0 => Ok(Box::new(Mapper000 {})),
        _ => bail!("Mapper ID not found!"),
    }
}

/// The circuit in the cartridge that is reponsible for mapping the addresses provided by the cpu to the onboard memory.
/// ROM only for now.
pub(super) trait Mapper {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8;
    fn character_read(&self, character_ram: &[u8], address: u16) -> u8;
    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8);
    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8);
}

pub(super) struct Mapper000 {}

impl Mapper for Mapper000 {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8 {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)],
            0x8000..=0xffff => program_rom[usize::from(address - 0x8000) % program_rom.len()],
            _ => panic!("Mapper000::program_read called with invalid address 0x{:4X}", address),
        }
    }

    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return character_ram[usize::from(address)];
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            _ => panic!("Mapper000::program_write called with invalid address 0x{:4X}", address),
        }
    }

    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        character_ram[usize::from(address)] = data;
    }
}
