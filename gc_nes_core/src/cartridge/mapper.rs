//! The mapper module contains implementation code for the various
//! types of mapping circuits that were present in NES cartridges.
//!
//! At present only iNES mappers 000 through 004 are supported.

use super::*;

/// Returns a boxed mapper based on the mapper_id argument
pub(super) fn get_mapper(mapper_id: u16, submapper_id: u8) -> Result<Box<dyn Mapper>, Box<dyn Error>> {
    debug!("Getting mapper with id {}, submapper {}", mapper_id, submapper_id);
    match mapper_id {
        0 => Ok(Box::new(Mapper000 {})),
        1 => Ok(Box::new(Mapper001 {
            load_register: 0x10,
            control_register: 0x1c,
            character_bank_0_register: 0,
            character_bank_1_register: 0,
            program_bank_register: 0,
        })),
        2 => Ok(Box::new(Mapper002 { bank_select: 0x00 })),
        3 => Ok(Box::new(Mapper003 { bank_select: 0x00 })),
        4 => Ok(Box::new(Mapper004 {
            bank_control: 0,
            bank_select: [0x00; 8],
            mirroring: Mirroring::Horizontal,
            program_ram_write_protect: false,
            program_ram_enabled: false,
            scanline_counter: 0,
            scanline_counter_reload: 0,
            scanline_counter_reload_flag: false,
            interrupt_request_enabled: false,
            pending_interrupt_request: false,
        })),
        _ => bail!("Mapper ID {:03} unsupported!", mapper_id),
    }
}

/// The circuit in the cartridge that is reponsible for mapping the addresses provided by the cpu to the onboard memory.
/// ROM only for now.
pub(super) trait Mapper {
    /// Read from the cartridge's program ROM/RAM through the cartridge's mapper
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8 {
        match address {
            0x0000..=0x5fff => {
                warn!("Mapper read from {:04X}", address);
                return 0x00;
            }
            0x6000..=0x7fff => {
                if program_ram.is_empty() {
                    0x00
                } else {
                    program_ram[usize::from(address - 0x6000) % program_ram.len()]
                }
            }
            0x8000..=0xffff => {
                if program_rom.is_empty() {
                    0x00
                } else {
                    program_rom[usize::from(address - 0x8000) % program_rom.len()]
                }
            }
        }
    }

    /// Read from the cartridge's character ROM/RAM through the cartridge's mapper
    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return character_ram[usize::from(address)];
    }

    /// Write to the cartridge's program RAM through the cartridge's mapper
    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            _ => warn!("Mapper::program_write called with invalid address 0x{:4X}", address),
        }
    }

    /// Write to the cartridge's character RAM through the cartridge's mapper
    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        character_ram[usize::from(address)] = data;
    }

    /// Get the mirroring mode from the cartridge
    fn get_mirroring(&mut self, mirroring: Mirroring) -> Mirroring {
        return mirroring;
    }

    /// Check if the cartridge is triggering an interrupt
    fn get_pending_interrupt_request(&mut self) -> bool {
        return false;
    }

    /// Called at the end of each scanline. Used by iNES Mapper 004 to
    /// trigger interrupt requests at specific times during screen rendering
    fn end_of_scanline(&mut self) {}
}

/// Mapper struct for the NROM Mapper, which is given the iNES id of 000
pub(super) struct Mapper000 {}

impl Mapper for Mapper000 {}

/// Mapper struct for the SxROM Mappers, which are given the iNES id of 001
pub(super) struct Mapper001 {
    load_register: u8,
    control_register: u8,
    character_bank_0_register: u8,
    character_bank_1_register: u8,
    program_bank_register: u8,
}

impl Mapper for Mapper001 {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8 {
        match address {
            0x0000..=0x5fff => {
                warn!("Mapper001 read from {:04X}", address);
                return 0x00;
            }
            0x6000..=0x7fff => {
                if self.program_bank_register & 0x10 > 0 && program_ram.is_empty() {
                    0x00
                } else {
                    program_ram[usize::from(address - 0x6000) % program_ram.len()]
                }
            }
            0x8000..=0xffff => match ((self.control_register & 0x0c) >> 2, address) {
                (0, _) => program_rom[usize::from(address & 0x7fff)],
                (1, _) => program_rom[usize::from(address & 0x7fff) + ((self.program_bank_register as usize & 0x0e) * 0x4000)],
                (2, 0x8000..=0xbfff) => program_rom[usize::from(address & 0x3fff)],
                (2, 0xc000..=0xffff) => program_rom[usize::from(address & 0x3fff) + ((self.program_bank_register as usize & 0x0f) * 0x4000)],
                (3, 0x8000..=0xbfff) => program_rom[usize::from(address & 0x3fff) + ((self.program_bank_register as usize & 0x0f) * 0x4000)],
                (3, 0xc000..=0xffff) => {
                    program_rom[(usize::from(address & 0x3fff) + ((program_rom.len() / 0x4000 - 1) * 0x4000)) % program_rom.len()]
                }
                _ => unreachable!(),
            },
        }
    }

    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return match (self.control_register & 0x10, address) {
            (0x00, 0x0000..=0x1fff) => character_ram[(address as usize) + ((self.character_bank_0_register as usize & 0x1e) * 0x1000)],
            (0x10, 0x0000..=0x0fff) => character_ram[(address & 0x0fff) as usize + (self.character_bank_0_register as usize * 0x1000)],
            (0x10, 0x1000..=0x1fff) => character_ram[(address & 0x0fff) as usize + (self.character_bank_1_register as usize * 0x1000)],
            _ => unreachable!(),
        };
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            0x8000..=0xffff => {
                if data & 0x80 == 0 {
                    // Boolean to determine if the load register should be copied into the target register
                    // after this bit is written.
                    let copy = self.load_register & 1 > 0;
                    self.load_register = (self.load_register >> 1) | ((data & 1) << 4);
                    if copy {
                        // Set one of the mapper registers based on the target address
                        match (address & 0x6000) + 0x8000 {
                            0x8000 => self.control_register = self.load_register,
                            0xa000 => self.character_bank_0_register = self.load_register,
                            0xc000 => self.character_bank_1_register = self.load_register,
                            0xe000 => self.program_bank_register = self.load_register,
                            _ => unreachable!(),
                        }
                        self.load_register = 0x10
                    }
                } else {
                    // Reset the load register when the 7th bit isn't set
                    self.load_register = 0x10
                }
            }
            _ => warn!("Mapper000::program_write called with invalid address 0x{:4X}", address),
        }
    }

    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        match (self.control_register & 0x10, address) {
            (0x00, 0x0000..=0x1fff) => character_ram[(address as usize) + ((self.character_bank_0_register as usize & 0x1e) * 0x1000)] = data,
            (0x01, 0x0000..=0x0fff) => character_ram[(address & 0x0fff) as usize + (self.character_bank_0_register as usize * 0x1000)] = data,
            (0x01, 0x1000..=0x1fff) => character_ram[(address & 0x0fff) as usize + (self.character_bank_1_register as usize * 0x1000)] = data,
            _ => unreachable!(),
        }
    }

    fn get_mirroring(&mut self, _mirroring: Mirroring) -> Mirroring {
        return match self.control_register & 0b11 {
            0b00 => Mirroring::OneScreenLower,
            0b01 => Mirroring::OneScreenUpper,
            0b10 => Mirroring::Vertical,
            0b11 => Mirroring::Horizontal,
            _ => unreachable!(),
        };
    }
}

/// Mapper struct for the UxROM Mappers, which are given the iNES id of 002
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
                if program_ram.is_empty() {
                    0x00
                } else {
                    program_ram[usize::from(address - 0x6000) % program_ram.len()]
                }
            }
            // Pick a bank based on the internal register
            0x8000..=0xbfff => program_rom[usize::from(address & 0x3fff) + (self.bank_select as usize * 0x4000)],
            // Always points to the last program rom bank
            0xc000..=0xffff => program_rom[usize::from(address & 0x3fff) + ((program_rom.len() / 0x4000 - 1) * 0x4000)],
        }
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            // Writes to the rom set the bank select register
            0x8000..=0xffff => self.bank_select = data & 0x0f,
            _ => warn!("Mapper001::program_write called with invalid address 0x{:4X}", address),
        }
    }
}

/// Mapper struct for the CNROM Mapper, which is given the iNES id of 003
pub(super) struct Mapper003 {
    bank_select: u8,
}

impl Mapper for Mapper003 {
    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return character_ram[usize::from(address & 0x1fff) | (self.bank_select as usize * 0x2000)];
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            0x8000..=0xffff => {
                // The real CNROM has two security bits, but I'm ignoring those
                self.bank_select = data & 0x03;
            }
            _ => warn!("Mapper003::program_write called with invalid address 0x{:4X}", address),
        }
    }

    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        character_ram[usize::from(address & 0x1fff) | (self.bank_select as usize * 0x2000)] = data;
    }
}

/// Mapper struct for the CxROM Mapper, which is given the iNES id of 003
pub(super) struct Mapper004 {
    bank_control: u8,
    bank_select: [u8; 8],
    mirroring: Mirroring,
    program_ram_write_protect: bool,
    program_ram_enabled: bool,
    scanline_counter: u8,
    scanline_counter_reload: u8,
    scanline_counter_reload_flag: bool,
    interrupt_request_enabled: bool,
    pending_interrupt_request: bool,
}

impl Mapper for Mapper004 {
    fn program_read(&self, program_rom: &[u8], program_ram: &[u8], address: u16) -> u8 {
        match address {
            0x0000..=0x5fff => {
                warn!("Mapper read from {:04X}", address);
                return 0x00;
            }
            0x6000..=0x7fff => {
                if program_ram.is_empty() {
                    0x00
                } else {
                    program_ram[usize::from(address - 0x6000) % program_ram.len()]
                }
            }
            0x8000..=0xffff => match (address, self.bank_control & 0x40) {
                // Point to either the second last bank or the bank selected by the 6th bank selector
                (0x8000..=0x9fff, 0x00) => program_rom[usize::from(address & 0x1fff) + usize::from(self.bank_select[6]) * 0x2000],
                (0x8000..=0x9fff, 0x40) => program_rom[usize::from(address & 0x1fff) + ((program_rom.len() / 0x2000 - 2) * 0x2000)],
                // Always points to the bank selected by the 7th bank selector
                (0xa000..=0xbfff, _) => program_rom[usize::from(address & 0x1fff) + usize::from(self.bank_select[7]) * 0x2000],
                // Point to either the second last bank or the bank selected by the 6th bank selector
                (0xc000..=0xdfff, 0x00) => program_rom[usize::from(address & 0x1fff) + ((program_rom.len() / 0x2000 - 2) * 0x2000)],
                (0xc000..=0xdfff, 0x40) => program_rom[usize::from(address & 0x1fff) + usize::from(self.bank_select[6]) * 0x2000],
                // Always points to the last bank
                (0xe000..=0xffff, _) => program_rom[usize::from(address & 0x1fff) + ((program_rom.len() / 0x2000 - 1) * 0x2000)],
                _ => unreachable!(),
            },
        }
    }

    fn character_read(&self, character_ram: &[u8], address: u16) -> u8 {
        return match (address, self.bank_control & 0x80) {
            (0x0000..=0x07ff, 0x00) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[0]) * 0x0400], // TODO: Check if 0x03ff is the right increment for the 2kb banks
            (0x0800..=0x0fff, 0x00) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[1]) * 0x0400],
            (0x1000..=0x13ff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[2]) * 0x0400],
            (0x1400..=0x17ff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[3]) * 0x0400],
            (0x1800..=0x1bff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[4]) * 0x0400],
            (0x1c00..=0x1fff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[5]) * 0x0400],
            // Bank Control 0x80 = data
            (0x0000..=0x03ff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[2]) * 0x0400],
            (0x0400..=0x07ff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[3]) * 0x0400],
            (0x0800..=0x0bff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[4]) * 0x0400],
            (0x0c00..=0x0fff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[5]) * 0x0400],
            (0x1000..=0x17ff, 0x80) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[0]) * 0x0400],
            (0x1800..=0x1fff, 0x80) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[1]) * 0x0400],
            _ => panic!("Mapper004::character_read called with invalid address: 0x{:04X}", address),
        };
    }

    fn program_write(&mut self, program_ram: &mut [u8], address: u16, data: u8) {
        match address {
            0x6000..=0x7fff => program_ram[usize::from(address - 0x6000)] = data,
            0x8000..=0xffff => match (address, address & 0x01) {
                (0x8000..=0x9fff, 0) => self.bank_control = data,
                (0x8000..=0x9fff, 1) => self.bank_select[self.bank_control as usize & 0x07] = data,
                (0xa000..=0xbfff, 0) => {
                    if data & 0x01 > 0 {
                        self.mirroring = Mirroring::Horizontal
                    } else {
                        self.mirroring = Mirroring::Vertical
                    }
                }
                (0xa000..=0xbfff, 1) => {
                    self.program_ram_write_protect = data & 0x40 > 0;
                    self.program_ram_enabled = data & 0x80 > 0;
                }
                (0xc000..=0xdfff, 0) => self.scanline_counter_reload = data,
                (0xc000..=0xdfff, 1) => {
                    self.scanline_counter = self.scanline_counter_reload;
                    self.scanline_counter_reload_flag = true;
                }
                (0xe000..=0xffff, 0) => {
                    self.interrupt_request_enabled = false;
                    self.pending_interrupt_request = false;
                }
                (0xe000..=0xffff, 1) => self.interrupt_request_enabled = true,
                _ => unreachable!(),
            },
            _ => warn!("Mapper004::program_write called with invalid address 0x{:4X}", address),
        }
    }

    fn character_write(&mut self, character_ram: &mut [u8], address: u16, data: u8) {
        match (address, self.bank_control & 0x80) {
            (0x0000..=0x07ff, 0x00) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[0]) * 0x0400] = data, // TODO: Check if 0x0400 is the right increment for the 2kb banks
            (0x0800..=0x0fff, 0x00) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[1]) * 0x0400] = data,
            (0x1000..=0x13ff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[2]) * 0x0400] = data,
            (0x1400..=0x17ff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[3]) * 0x0400] = data,
            (0x1800..=0x1bff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[4]) * 0x0400] = data,
            (0x1c00..=0x1fff, 0x00) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[5]) * 0x0400] = data,
            // Bank Control 0x80 = data
            (0x0000..=0x03ff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[2]) * 0x0400] = data,
            (0x0400..=0x07ff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[3]) * 0x0400] = data,
            (0x0800..=0x0bff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[4]) * 0x0400] = data,
            (0x0c00..=0x0fff, 0x80) => character_ram[usize::from(address & 0x03ff) + usize::from(self.bank_select[5]) * 0x0400] = data,
            (0x1000..=0x17ff, 0x80) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[0]) * 0x0400] = data,
            (0x1800..=0x1fff, 0x80) => character_ram[usize::from(address & 0x07ff) + usize::from(self.bank_select[1]) * 0x0400] = data,
            _ => warn!("Mapper004::character_write called with invalid address: 0x{:04X}", address),
        }
    }

    fn get_mirroring(&mut self, _mirroring: Mirroring) -> Mirroring {
        return self.mirroring;
    }

    fn get_pending_interrupt_request(&mut self) -> bool {
        let value = self.pending_interrupt_request;
        self.pending_interrupt_request = false;
        return value;
    }

    fn end_of_scanline(&mut self) {
        if self.scanline_counter == 0 && self.interrupt_request_enabled {
            self.pending_interrupt_request = true;
        }
        if self.scanline_counter == 0 || self.scanline_counter_reload_flag {
            self.scanline_counter = self.scanline_counter_reload;
            self.scanline_counter_reload_flag = false;
        } else {
            self.scanline_counter -= 1
        }
    }
}
