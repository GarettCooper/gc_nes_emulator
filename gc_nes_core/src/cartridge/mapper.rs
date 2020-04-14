use super::*;

/// Returns a boxed mapper based on the mapper_id argument
pub(super) fn get_mapper(mapper_id: u16, submapper_id: u8) -> Result<Box<dyn Mapper>, Box<dyn Error>> {
    debug!("Getting mapper with id {}, submapper {}", mapper_id, submapper_id);
    match mapper_id {
        0 => Ok(Box::new(Mapper000 {})),
        2 => Ok(Box::new(Mapper002 { bank_select: 0x00 })),
        _ => bail!("Mapper ID {} not found!", mapper_id),
    }
}

/// The circuit in the cartridge that is reponsible for mapping the addresses provided by the cpu to the onboard memory.
/// ROM only for now.
pub(super) trait Mapper {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8;
    fn character_read(&self, character_ram: &[u8], address: u16) -> u8;
    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8);
    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8);
    fn get_mirroring(&mut self, mirroring: Mirroring) -> Mirroring;
}

pub(super) struct Mapper000 {}

impl Mapper for Mapper000 {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8 {
        match address {
            0x0000..=0x5fff => {
                warn!("Mapper000 read from {:04X}", address);
                return 0x00;
            }
            0x6000..=0x7fff => {
                if program_ram.len() == 0 {
                    0x00
                } else {
                    program_ram[usize::from(address - 0x6000) % program_ram.len()]
                }
            }
            0x8000..=0xffff => {
                if program_rom.len() == 0 {
                    0x00
                } else {
                    program_rom[usize::from(address - 0x8000) % program_rom.len()]
                }
            }
            _ => panic!("Mapper000::program_read called with invalid address 0x{:4X}", address),
        }
    }

    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return character_ram[usize::from(address)];
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            _ => warn!("Mapper000::program_write called with invalid address 0x{:4X}", address),
        }
    }

    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        character_ram[usize::from(address)] = data;
    }

    fn get_mirroring(&mut self, mirroring: Mirroring) -> Mirroring {
        return mirroring;
    }
}

pub(super) struct Mapper002 {
    bank_select: u8,
}

impl Mapper for Mapper002 {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8 {
        match address {
            0x0000..=0x5fff => {
                warn!("Mapper000 read from {:04X}", address);
                return 0x00;
            }
            0x6000..=0x7fff => {
                if program_ram.len() == 0 {
                    0x00
                } else {
                    program_ram[usize::from(address - 0x6000) % program_ram.len()]
                }
            }
            // Pick a bank based on the internal register
            0x8000..=0xbfff => program_rom[usize::from(address & 0x3fff) + (self.bank_select as usize * 0x4000)],
            // Always points to the last program rom bank
            0xc000..=0xffff => program_rom[usize::from(address & 0x3fff) + ((program_rom.len() / 0x4000 - 1) * 0x4000)],
            _ => unreachable!(),
        }
    }

    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return character_ram[usize::from(address)];
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            // Writes to the rom set the bank select register
            0x8000..=0xffff => self.bank_select = data & 0x0f,
            _ => warn!("Mapper001::program_write called with invalid address 0x{:4X}", address),
        }
    }

    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        character_ram[usize::from(address)] = data;
    }

    fn get_mirroring(&mut self, mirroring: Mirroring) -> Mirroring {
        return mirroring;
    }
}
