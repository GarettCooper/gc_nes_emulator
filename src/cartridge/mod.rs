mod mapper;

use mapper::Mapper;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

// Header constants
/// Byte array equivalent to the string "NES\u{001a}", used for testing if the file format is valid
const IDENTIFICATION_STRING: [u8; 4] = [0x4e, 0x45, 0x53, 0x1a];

const PROGRAM_ROM_BANK_SIZE: usize = 16 * 1024; // 16 KiB
const CHARACTER_ROM_BANK_SIZE: usize = 8 * 1024; // 8 KiB

/// Returns the number of bytes of program rom for NES 2.0 or iNes format as a usize
/// Broken into its own function for ease of unit testing
fn calculate_rom_size(least_significant_byte: u8, most_significant_byte: u8, bank_size: usize, nes20: bool) -> Result<usize, Box<dyn Error>> {
    if nes20 && most_significant_byte == 0x0f {
        // In the NES 2.0 format an exponent multiplier format can be used
        let (size, overflow) = 2usize.pow(u32::from(least_significant_byte >> 2)).overflowing_mul(usize::from(least_significant_byte & 0x03) * 2 + 1);
        if overflow {
            bail!(".nes file memory size exceeded the maximum addressable range of the platform: {} bytes", usize::max_value())
        }
        return Ok(size);
    } else {
        // For other cases program rom size is just the value of the lsb and msb combined times 16 KiB
        let mut banks = usize::from(least_significant_byte);
        if nes20 {
            banks |= usize::from(most_significant_byte) << 8
        }
        return Ok(banks * bank_size);
    }
}

/// Type representing a Cartridge that can be loaded by the emulator, created by the
pub struct Cartridge {
    mapper: Box<dyn Mapper>,
    trainer_data: Box<[u8; 512]>,
    mirroring: Mirroring,
    program_rom: Box<[u8]>,
    program_ram: Box<[u8]>,
    // All character memory is treated as ram as games that only have ROM will not attempt to write to it
    character_ram: Box<[u8]>,
}

impl Cartridge {
    /// Read from the cartridge's program ROM/RAM through the cartridge's mapper
    pub(crate) fn program_read(&self, address: u16) -> u8 {
        return self.mapper.program_read(&self.program_rom, &self.program_ram, address)
    }

    /// Read from the cartridge's character ROM/RAM through the cartridge's mapper
    pub(crate) fn character_read(&self, address: u16) -> u8 {
        return self.mapper.character_read(&self.character_ram, address)
    }

    /// Write to the cartridge's program RAM through the cartridge's mapper
    pub(crate) fn program_write(&mut self, address: u16, data: u8) {
        self.mapper.program_write(&mut self.program_ram, address, data)
    }

    /// Write to the cartridge's character RAM through the cartridge's mapper
    pub(crate) fn character_write(&mut self, address: u16, data: u8) {
        self.mapper.character_write(&mut self.character_ram, address, data)
    }

    /// Get the mirroring mode from the cartridge
    pub(crate) fn get_mirroring(&mut self) -> Mirroring {
        return self.mapper.get_mirroring(self.mirroring);
    }

    /// Loads a cartridge from a file
    pub fn load_from_file(file_path: &Path) -> Result<Cartridge, Box<dyn Error>> {
        info!("Opening file: {}", file_path.to_str().unwrap());
        return Cartridge::load_from_reader(&mut BufReader::new(File::open(file_path)?));
    }

    /// Loads a cartridge from a reader and returns
    pub fn load_from_reader<T: Read>(buf_reader: &mut T) -> Result<Cartridge, Box<dyn Error>> {
        //let mut buf_reader = game_file;
        let mut header: [u8; 16] = [0; 16];
        buf_reader.read_exact(&mut header)?;

        // Test file format
        if header[..IDENTIFICATION_STRING.len()] == IDENTIFICATION_STRING {
            let header_flags_6 = HeaderFlags6::from_bits_truncate(header[6]);
            let header_flags_7 = HeaderFlags7::from_bits_truncate(header[7]);

            let nes2: bool = header_flags_7.contains(HeaderFlags7::NES_2_IDENTIFIER); // Check if file is NES 2.0
            if nes2 {
                debug!("File is in NES 2.0 format");
            } else {
                debug!("File is in iNes format");
            }

            // Get a mapper based on the four mapper identification fragments in the 6th, 7th, and 8th bytes of the header, along with a submapper
            let mapper = mapper::get_mapper(
                u16::from(header[8] & 0x0f) << 8 | u16::from(header[7] & HeaderFlags7::MAPPER_HI.bits) | u16::from(header[6] & HeaderFlags6::MAPPER_LO.bits) >> 4,
                (header[8] & 0xf0) >> 4,
            )?;

            let mirroring = if header_flags_6.contains(HeaderFlags6::VERTICAL_MIRRORING) {
                Mirroring::Vertical
            } else {
                Mirroring::Horizontal
            };

            let program_rom_size = calculate_rom_size(header[4], header[9] & 0x0f, PROGRAM_ROM_BANK_SIZE, nes2)?;
            debug!("Allocating {} bytes for program ROM", program_rom_size);

            let character_rom_size = calculate_rom_size(header[5], header[9] & 0xf0, CHARACTER_ROM_BANK_SIZE, nes2)?;
            debug!("Allocating {} bytes for character ROM", character_rom_size);

            let mut cartridge = Cartridge {
                mapper,
                mirroring,
                trainer_data: Box::new([0; 512]),
                program_rom: vec![0; program_rom_size].into_boxed_slice(),
                program_ram: Box::new([]), // TODO: Empty initialization until I implement this
                character_ram: vec![0; character_rom_size].into_boxed_slice(),
            };

            if HeaderFlags6::from_bits_truncate(header[6]).contains(HeaderFlags6::TRAINER_PRESENT) {
                debug!("Trainer is present");
                buf_reader.read_exact(cartridge.trainer_data.as_mut())?;
            }

            buf_reader.read_exact(cartridge.program_rom.as_mut())?;
            buf_reader.read_exact(cartridge.character_ram.as_mut())?;

            info!("File loaded successfully");
            return Ok(cartridge);
        } else {
            bail!("File format is invalid!");
        }
    }
}

bitflags! {
    #[derive(Default)]
    struct HeaderFlags6: u8 {
        const VERTICAL_MIRRORING = 0b0000_0001;
        const PERSISTENT_MEMORY = 0b0000_0010;
        const TRAINER_PRESENT = 0b0000_0100;
        const FOUR_SCREEN_MODE = 0b0000_1000;
        const MAPPER_LO = 0b1111_0000;
    }
}

bitflags! {
    #[derive(Default)]
    struct HeaderFlags7: u8 {
        const CONSOLE_TYPE = 0b0000_0011;
        const NES_2_IDENTIFIER = 0b0000_1100;
        const MAPPER_HI = 0b1111_0000;
    }
}

#[derive(Copy, Clone, PartialEq)]
pub(crate) enum Mirroring {
    Horizontal,
    Vertical,
}

#[cfg(test)]
mod test {
    use super::*;

    //Macro for reducing the amount of boilerplate
    macro_rules! calculate_rom_size_tests {
        ($($name:ident: $expected:expr, $value:expr,)*) => {
            mod calculate_rom_size_tests{
                use super::*;
                $(
                #[test]
                fn $name() {
                    assert_eq!($expected, $value);
                }
                )*

                // calculate_rom_size_tests that don't use the format go here
                #[test]
                fn nes2_exp_maximum() {
                   // Pick appropriate size for to match system limitations
                   if usize::max_value() == u32::max_value() as usize {
                        calculate_rom_size(0xa0, 0x0f, PROGRAM_ROM_BANK_SIZE, true).expect_err("Did not produce an error for a value that exceeds the maximum addressable range of 32 bit systems");
                   } else {
                        calculate_rom_size(0xff, 0x0f, PROGRAM_ROM_BANK_SIZE, true).expect_err("Did not produce an error for a value that exceeds the maximum addressable range of 64 bit systems");
                   }
                }
            }
        }
    }

    calculate_rom_size_tests! {
        ines_minimum: 16384, calculate_rom_size(0x01, 0x0f, PROGRAM_ROM_BANK_SIZE, false).unwrap(),
        ines_middle: 65536, calculate_rom_size(0x04, 0x0f, PROGRAM_ROM_BANK_SIZE, false).unwrap(),
        ines_maximum: 0x3fc000, calculate_rom_size(0xff, 0x00, PROGRAM_ROM_BANK_SIZE, false).unwrap(),
        nes2_base_maximum: 62898176, calculate_rom_size(0xff, 0x0e, PROGRAM_ROM_BANK_SIZE, true).unwrap(),
        nes2_exp_minimum: 1, calculate_rom_size(0x00, 0x0f, PROGRAM_ROM_BANK_SIZE, true).unwrap(),
        nes2_exp_middle: 196608, calculate_rom_size(0x41, 0x0f, PROGRAM_ROM_BANK_SIZE, true).unwrap(),
    }
}
