extern crate emulator_6502;

use emulator_6502::{MOS6502, Interface6502};
use std::fs::File;
use crate::cartridge::NesCartridge;
use crate::nes::ppu::NesPpu;
use crate::nes::apu::NesApu;

mod ppu;
mod apu;

pub (crate) struct Nes {
    cpu: MOS6502, //The actual nes used a 2A03 which combined the cpu and apu functionality, but they are represented separately here
    ppu: NesPpu, //The picture processing unit of the Nes
    apu: NesApu,
    ram: Box<[u8; 0x0800]>, //The nes' two kilobytes of ram
    cartridge: Box<dyn NesCartridge> //The cartridge loaded into the NES
}

impl Nes {

    ///Creates a new NES instance
    fn new(cartridge: Box<NesCartridge>) -> Self{
        Nes {
            cpu: MOS6502::new(),
            ppu: NesPpu::new(),
            apu: NesApu::new(),
            ram: Box::new([0; 0x0800]),
            cartridge
        }
    }

}

impl Interface6502 for Nes {
    fn read(&mut self, address: u16) -> u8{
        match address {
            0x0000..=0x1fff => self.ram[address as usize % 0x0800], //Addresses 0x0800-0x1ff mirror the 2kb of ram
            0x2000..=0x3fff => unimplemented!(), //self.ppu.read(address)
            0x4000..=0x4015 => unimplemented!(), //self.apu.read(address)
            0x4016..=0x4017 => unimplemented!(), //self.input.read(address)
            0x4018..=0x401f => unimplemented!(), //Usually disabled on the nes TODO: Decide how to handle these
            0x4020..=0xffff => unimplemented!(), //self.cartridge.read(address)
        }
    }

    fn write(&mut self, address: u16, data: u8){
        match address {
            0x0000..=0x1fff => self.ram[address as usize % 0x0800] = data, //Addresses 0x0800-0x1ff mirror the 2kb of ram
            0x2000..=0x3fff => unimplemented!(), //self.ppu.write(address, data)
            0x4000..=0x4015 => unimplemented!(), //self.apu.write(address, data)
            0x4016..=0x4017 => unimplemented!(), //self.input.write(address, data)
            0x4018..=0x401f => unimplemented!(), //Usually disabled on the nes
            0x4020..=0xffff => unimplemented!(), //self.cartridge.write(address, data) //Cartridge is ROM, decide what to do for this
        }
    }
}