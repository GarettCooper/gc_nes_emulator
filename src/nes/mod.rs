extern crate emulator_6502;

use crate::cartridge::Cartridge;
use crate::nes::apu::NesApu;
use crate::nes::ppu::NesPpu;
use emulator_6502::{Interface6502, MOS6502};

mod apu;
mod ppu;

pub struct Nes {
    // NES Components
    cpu: MOS6502, // The actual NES used a 2A03 which combined the cpu and apu functionality, but they are represented separately here
    bus: Bus,     // The bus of the NES, which holds ownership of the other components
    // Additional Tracking Information
    cycle_count: u64,
}

struct Bus {
    cartridge: Box<Cartridge>, // The cartridge loaded into the NES
    ppu: NesPpu,               // The picture processing unit of the NES
    apu: NesApu,               // The audio processing unit of the NES
    ram: Box<[u8; 0x0800]>,    // The NES' two kilobytes of ram
}

impl Nes {
    /// Creates a new NES instance
    pub fn new(cartridge: Cartridge) -> Self {
        Nes {
            cpu: MOS6502::new(),
            bus: Bus {
                cartridge: Box::new(cartridge),
                ppu: NesPpu::new(),
                apu: NesApu::new(),
                ram: Box::new([0; 0x0800]),
            },
            cycle_count: 0,
        }
    }

    pub fn cycle(&mut self) {
        if self.cycle_count % 3 == 0 {
            // CPU cycles every third ppu dot
            self.cpu.cycle(&mut self.bus);
        }
        self.bus.ppu.cycle(&self.bus.cartridge);
        self.cycle_count += 1;
    }

    /// Resets the state of the console
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.bus.reset();
    }
}

impl Bus {
    /// Resets the state of the console components on the bus
    fn reset(&mut self) {
        // TODO: Implement these
        // self.ppu.reset();
        // self.apu.reset();
    }
}

impl Interface6502 for Bus {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x1fff => self.ram[usize::from(address) & 0x07ff], // Addresses 0x0800-0x1fff mirror the 2KiB of ram
            0x2000..=0x3fff => self.ppu.read(&self.cartridge, address), // Mirroring will be done by the ppu
            0x4000..=0x4015 => unimplemented!(),                        // self.apu.read(address)
            0x4016..=0x4017 => unimplemented!(),                        // self.input.read(address)
            0x4018..=0x401f => unimplemented!(),                        // Usually disabled on the nes TODO: Decide how to handle these
            0x4020..=0xffff => self.cartridge.program_read(address),
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            0x0000..=0x1fff => self.ram[usize::from(address) & 0x07ff] = data,     // Addresses 0x0800-0x1fff mirror the 2KiB of ram
            0x2000..=0x3fff => self.ppu.write(&mut self.cartridge, address, data), // Mirroring will be done by the ppu
            0x4000..=0x4015 => unimplemented!(),                                   // self.apu.write(address, data)
            0x4016..=0x4017 => unimplemented!(),                                   // self.input.write(address, data)
            0x4018..=0x401f => unimplemented!(),                                   // Usually disabled on the nes
            0x4020..=0xffff => self.cartridge.program_write(address, data),
        }
    }
}
