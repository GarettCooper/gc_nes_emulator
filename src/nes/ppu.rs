use crate::cartridge::{Cartridge, Mirroring};

/// The total number of scanlines in a frame.
const MAX_SCANLINES: u16 = 261;
/// The total number of cycles in a scanline.
const MAX_CYCLES: u16 = 340;
/// Mask for the coarse x bits in the vram addresses.
const COARSE_X_MASK: u16 = 0b00000000_00011111;
/// Mask for the coarse y bits in the vram addresses.
const COARSE_Y_MASK: u16 = 0b00000011_11100000;
/// The offset of the coarse y bits in the vram address.
const COARSE_Y_OFFSET: u16 = 5;
/// Mask for the coarse y bits in the vram addresses.
const FINE_Y_MASK: u16 = 0b01110000_00000000;
/// The offset of the coarse y bits in the vram address.
const FINE_Y_OFFSET: u16 = 12;

#[cfg_attr(test, derive(Clone))]
pub(super) struct NesPpu {
    ctrl_flags: PpuCtrl,
    mask_flags: PpuMask,
    status_flags: PpuStatus,
    oam_address: u8,
    /// The register that temporarily holds a vram address before copying it over to the current register
    temporary_vram_address: u16,
    /// The register holding the current vram address pointing to the internal memory of the PPU
    current_vram_address: u16,
    /// 3 bit address used for scrolling across individual pixels in tiles
    fine_x_scroll: u8,
    /// Latch for multiple writes to the address and scroll
    write_latch: bool,
    /// Buffer for storing data between reads.
    data_buffer: u8,
    /// The pattern ram stores values used for mapping the sprite bitmaps to colours that the NES
    /// can display.
    palette_ram: Box<[u8; 0x20]>,
    /// The name table, which is used for storing the pattern id that should be displayed on the screen
    /// in a particular location. The two kilobytes of memory are both their own pattern table, which are
    /// used for scrolling.
    name_table: Box<[u8; 0x800]>,
    /// Stores 4 bits of information about up to 64 sprites
    object_attribute_memory: Box<[u8; u8::max_value() as usize + 1]>,
    /// The current state of the screen
    screen_buffer: Box<[u32; super::NES_SCREEN_DIMENSIONS]>,
    /// The scanline (-1 to 261) of the screen that is currently being drawn
    scanline: u16,
    /// The cycle (0 to 340) of the current scanline
    cycle: u16,
}

impl NesPpu {
    /// Create a new instance of a NesPpu
    pub fn new() -> Self {
        NesPpu {
            ctrl_flags: Default::default(),
            mask_flags: Default::default(),
            status_flags: Default::default(),
            oam_address: 0x00,
            temporary_vram_address: 0x0000,
            current_vram_address: 0x0000,
            fine_x_scroll: 0,
            write_latch: false,
            data_buffer: 0x00,
            palette_ram: Box::new([0; 0x20]),
            name_table: Box::new([0; 0x800]),
            object_attribute_memory: Box::new([0; u8::max_value() as usize + 1]),
            screen_buffer: Box::new([0; super::NES_SCREEN_DIMENSIONS]),
            scanline: 261,
            cycle: 0,
        }
    }

    /// Runs a single PPU cycle, which draws a single pixel into the frame buffer
    pub fn cycle(&mut self, cartridge: &Cartridge) {

        let sprite_colour: u8 = 0x01;
        let background_colour: u8 = 0x02;
        let sprite_priority: bool = true;

        match self.scanline {
            MAX_SCANLINES | 0..=239 => {
                match self.cycle {
                    // Idle cycle
                    0 => {}, // TODO: Accurate PPU address bus value
                    // Cycles for visible pixels
                    1..=256 => {},
                    // Fetch the tile data for the sprites on the next scanline
                    257..=320 => {
                        self.oam_address = 0;
                    },
                    // Fetch the first two tiles for the next scanline
                    321..=336 => {},
                    // Final four cycles just make dummy reads
                    c @ 337..=340 if c % 2 == 0 => { cartridge.character_read(0x00); }, // TODO: Read from the correct location
                    // Idle cycles to simulate two cycle read time
                    337..=340 => {},
                    _ => panic!("Invalid Cycle") // TODO: Consider unreachable!()
                }
            },
            240 => {}, // Nothing happens on the first scanline off the screen
            241 => {
                if self.cycle == 1 { // The vertical blank flag is set on the second cycle of scanline 241
                    self.status_flags.set(PpuStatus::VERTICAL_BLANK, true);
                }
            },
            242..=260 => {}, // Nothing continues to happen so that CPU can manipulate PPU freely
            _ => panic!("Invalid Scanline: {}", self.scanline) // TODO: Consider unreachable!()
        }


        unimplemented!();
    }

    /// Function for reading from the PPU. Any address passed to the function will be mapped to one of
    /// the eight valid ppu addresses ( address % 8), equivalent to only using the lowest three bits
    pub fn read(&mut self, cartridge: &mut Cartridge, address: u16) -> u8 {
        match address & 0x07 {
            // Mirroring first 3 bits
            0x0000 => panic!("Attempting to read from ppu control flag"), // TODO: Check this behaviour
            0x0001 => panic!("Attempting to read from ppu mask flag"),    // TODO: Check this behaviour
            0x0002 => {
                let value = self.status_flags.bits;
                // Reset Vertical Blank flag and the latch
                self.status_flags.set(PpuStatus::VERTICAL_BLANK, false);
                self.write_latch = false;
                return value;
            }
            0x0003 => panic!("Attempting to read from ppu OAM address"), // TODO: Check this behaviour
            0x0004 => self.oam_read(),
            0x0005 => panic!("Attempting to read from ppu scroll address"), // TODO: Check this behaviour
            0x0006 => panic!("Attempting to read from ppu vram address"),   // TODO: Check this behaviour
            0x0007 => {
                // Reading from the PPU is delayed by a cycle, so return data from the last address
                // that was read from.
                let temp = self.data_buffer;
                self.data_buffer = self.vram_read(self.current_vram_address, cartridge);
                // Increment the address in the x or y direction depending on a ctrl flag
                self.current_vram_address += if self.ctrl_flags.contains(PpuCtrl::VRAM_INCREMENT) {
                    0x20
                } else {
                    0x01
                };
                return temp
            },
            _ => panic!("Invalid PPU Read Address"), // TODO: Consider unreachable!()
        }
    }

    /// Function for writing to the PPU. Any address passed to the function will be mapped to one of
    /// the eight valid ppu addresses ( address % 8), equivalent to only using the lowest three bits
    pub fn write(&mut self, cartridge: &mut Cartridge, address: u16, data: u8) {
        match address & 0x07 {
            // Mirroring first 3 bits
            0x0000 => {
                self.ctrl_flags.bits = data;
                // Mask out the nametable selection bits
                self.temporary_vram_address &= 0b1110011_11111111;
                // Select the nametables based on the new values set to the ctrl register
                self.temporary_vram_address = (data as u16 & 0b11) << 10
            },
            0x0001 => self.mask_flags.bits = data,
            0x0002 => warn!("Ignored attempted write to the ppu status flag. Data: {:2X}", data), // TODO: Check this behaviour
            0x0003 => self.oam_address = data,
            0x0004 => self.oam_write(data),
            0x0005 => self.scroll_write(data),
            0x0006 => self.vram_address_write(data),
            0x0007 => {
                self.vram_write(self.current_vram_address, data, cartridge);
                // Increment the address in the x or y direction depending on a ctrl flag
                self.current_vram_address += if self.ctrl_flags.contains(PpuCtrl::VRAM_INCREMENT) {
                    0x20
                } else {
                    0x01
                }
            },
            _ => warn!("Invalid PPU Write Address"), // TODO: Consider unreachable!()
        }
    }

    fn oam_read(&mut self) -> u8 {
        self.object_attribute_memory[self.oam_address as usize]
    }

    /// Reads from the internal bus of the PPU
    fn vram_read(&mut self, address: u16, cartridge: &mut Cartridge) -> u8 {
        return match address {
            0x0000..=0x1fff => cartridge.character_read(address),
            0x2000..=0x3eff => self.name_table[self.apply_name_table_mirroring(cartridge, address)],
            0x3f00..=0x3fff => self.palette_ram[usize::from(address) & 0x1f],
            _ => panic!("Attempt to read from an invalid PPU bus address!")
        }
    }

    fn oam_write(&mut self, data: u8) {
        self.object_attribute_memory[self.oam_address as usize] = data;
        self.oam_address += 1; // Writing to the oam address increments it
    }

    pub(super) fn oam_dma_write(&mut self, address: u8, data: u8) {
        self.object_attribute_memory[self.oam_address.wrapping_add(address) as usize] = data;
    }

    /// Write to the scroll register (Which is also repurposed as the vram address).
    /// The first write sets x scroll and the second write sets y scroll.
    ///
    /// This function and [vram_address_write](#NesPpu::vram_address_write) are both backed by the same register
    /// but write to it in different ways. See the [NesDev wiki](https://wiki.nesdev.com/w/index.php/PPU_scrolling)
    /// to learn more.
    fn scroll_write(&mut self, data: u8) {
        let data = data as u16; // So that it doesn't need to be cast in every place
        if self.write_latch {
            // SECOND WRITE IS TO Y SCROLL
            // Top 3 bits of the vram address represent fine y scroll, and are set based on the
            // bottom three bits of the written byte. Bits 8 to 11 represent the coarse y scroll,
            // and are set based on the top 5 bits of the written byte.
            self.temporary_vram_address &= 0x0c1f;
            self.temporary_vram_address |= ((data & 0x07) << FINE_Y_OFFSET) | ((data & 0xf8) << 2);
            self.write_latch = false;
        } else {
            // FIRST WRITE IS TO X SCROLL
            // Bottom three bits are written to x scroll register
            self.fine_x_scroll = data as u8 & 0x07;
            // Top 5 bits are written to the bottom five bits of the temporary address
            // (Which represent coarse x scroll)
            self.temporary_vram_address &= 0xffe0; // Mask out bottom 5 bits
            self.temporary_vram_address |= data >> 3;
            self.write_latch = true;
        }
    }

    /// Write to the PPU's internal bus address. (Which is also repurposed as the scroll register).
    /// The first write sets the top six bits of the address and the second write sets the bottom
    /// eight bits.
    ///
    /// This function and [scroll_write](#NesPpu::scroll_write) are both backed by the same register
    /// but write to it in different ways. See the [NesDev wiki](https://wiki.nesdev.com/w/index.php/PPU_scrolling)
    /// to learn more.
    fn vram_address_write(&mut self, data: u8) {
        if self.write_latch {
            // Second write is to the bottom byte of the temp vram address
            self.temporary_vram_address = (0xff00 & self.temporary_vram_address) | u16::from(data);
            self.current_vram_address = self.temporary_vram_address;
            self.write_latch = false;
        } else {
            // First write to bits 13-8 of the temp vram address, the 14th bit is set to 0
            self.temporary_vram_address = (0x00ff & self.temporary_vram_address) | ((u16::from(data) & 0x3f) << 8);
            self.write_latch = true; // Set latch so next write is to lower byte
        }
    }

    /// Increment the coarse x scroll position, accounting for wrapping and name table swapping.
    fn coarse_x_increment(&mut self) {
        // If the coarse x address has reached its maximum value...
        if self.current_vram_address & COARSE_X_MASK == COARSE_X_MASK {
            // Flip it to zero. 0x0400, the 10th bit, is also flipped, which determines the
            // Horizontal nametable that is used.
            self.current_vram_address ^= 0x0400 | COARSE_X_MASK;
        } else {
            // Otherwise, just increment the coarse x
            self.current_vram_address += 0x1;
        }
    }

    /// Increment the y scroll value, accounting for coarse/fine bits, wrapping, and nametable swapping
    fn y_increment(&mut self) {
        // If the fine y value isn't at its maximum...
        if (self.current_vram_address >> FINE_Y_OFFSET) != 0x7 {
            // Just increment it
            self.current_vram_address += 0x1 << FINE_Y_OFFSET;
        } else {
            // Otherwise, wrap the fine y value around to 0
            self.current_vram_address &= !FINE_Y_MASK;
            // And increment/wrap coarse y
            match (self.current_vram_address & COARSE_Y_MASK) >> COARSE_Y_OFFSET {
                // Wrap around and flip the nametable at 29, because the last two rows are used for
                // other data, the attribute memory.
                0x1d => {
                    // Flip the vertical nametable
                    self.current_vram_address ^= 0x0800;
                    // Wrap around
                    self.current_vram_address &= !COARSE_Y_MASK;
                },
                // But if it's at 31, wrap it around without changing vertical nametables.
                // This is to replicate specific NES behaviour
                0x1f => self.current_vram_address &= !COARSE_Y_MASK,
                //Otherwise, increment coarse y
                _ => self.current_vram_address += 0x1 << COARSE_Y_OFFSET
            }
        }
    }

    /// Writes onto the internal bus of the PPU
    fn vram_write(&mut self, address: u16, data: u8, cartridge: &mut Cartridge) {
        match address {
            0x0000..=0x1fff => cartridge.character_write(address, data),
            0x2000..=0x3eff => self.name_table[self.apply_name_table_mirroring(cartridge, address)] = data,
            0x3f00..=0x3fff => self.palette_ram[usize::from(address) & 0x1f] = data,
            _ => warn!("Attempt to write to an invalid PPU bus address!")
        }
    }

    /// Gets the screen buffer from the PPU
    pub(super) fn get_screen(&mut self) -> &[u32; super::NES_SCREEN_DIMENSIONS] {
        return &self.screen_buffer
    }

    /// Maps an address to a name table address by applying mirroring
    fn apply_name_table_mirroring(&mut self, cartridge: &mut Cartridge, address: u16) -> usize {
        return ((address & 0x3ff) | ((address >> (0xa | (cartridge.get_mirroring() == Mirroring::Horizontal) as u16) & 0x1) << 0xa)) as usize
    }
}

/// Function that determines whether the sprite or the background colour will be used for a pixel.
fn colour_priority(sprite_colour: u8, background_colour: u8, sprite_priority: bool) {
    match (sprite_colour, background_colour, sprite_priority) {
        (0x00, 0x00, _) => background_colour, //TODO: BG0x3f00?
        (0x00, 0x01..=0x03, _) => sprite_colour,
        (0x01..=0x03, 0x00, _) => background_colour,
        (0x01..=0x03, 0x01..=0x03, true) => sprite_colour,
        (0x01..=0x03, 0x01..=0x03, false) => background_colour,
        _ => panic!("Invalid colour values") // Consider unreachable!()
    };
}

bitflags! {
    #[derive(Default)]
    struct PpuCtrl: u8{ // Labels from https://wiki.nesdev.com/w/index.php/PPU_registers
        const NMI_ENABLE = 0b1000_0000;// Generate an NMI at the start of the vertical blanking interval (0: off; 1: on)
        const MASTER_SELECT = 0b0100_0000;// PPU master/slave select (0: read backdrop from EXT pins; 1: output color on EXT pins)
        const SPRITE_HEIGHT = 0b0010_0000;// Sprite size (0: 8x8 pixels; 1: 8x16 pixels)
        const BACKGROUND_SELECT = 0b0001_0000;//Background pattern table address (0: $0000; 1: $1000)
        const SPRITE_SELECT = 0b0000_1000;// Sprite pattern table address for 8x8 sprites (0: $0000; 1: $1000; ignored in 8x16 mode)
        const VRAM_INCREMENT = 0b0000_0100; // VRAM address increment per CPU read/write of PPUDATA (0: add 1, going across; 1: add 32, going down)
        const NAMETABLE_SELECT = 0b0000_0011; // Base nametable address (0 = $2000; 1 = $2400; 2 = $2800; 3 = $2C00)
    }
}

bitflags! {
    #[derive(Default)]
    struct PpuMask: u8{ // Labels from https://wiki.nesdev.com/w/index.php/PPU_registers
        const EMPHASIZE_BLUE = 0b1000_0000;
        const EMPHASIZE_GREEN = 0b0100_0000;
        const EMPHASIZE_RED = 0b0010_0000;
        const SPRITE_ENABLE = 0b0001_0000;// 1: Show sprites
        const BACKGROUND_ENABLE = 0b0000_1000;// 1: Show background
        const SPRITE_LEFT_ENABLE = 0b0000_0100;// 1: Show sprites in leftmost 8 pixels of screen, 0: Hide
        const BACKGROUN_LEFT_ENABLE = 0b0000_0010; // 1: Show background in leftmost 8 pixels of screen, 0: Hide
        const GREYSCALE = 0b0000_0001; // Greyscale (0: normal color, 1: produce a greyscale display)
    }
}

bitflags! {
    struct PpuStatus: u8{ // Labels from https://wiki.nesdev.com/w/index.php/PPU_registers
        const VERTICAL_BLANK = 0b1000_0000; // Vertical blank has started (0: not in vblank; 1: in vblank)
        const SPRITE_0_HIT = 0b0100_0000; // Sprite 0 Hit.  Set when a nonzero pixel of sprite 0 overlaps a nonzero background pixel
        const SPRITE_OVERFLOW = 0b0010_0000; // In theory is set when more than 8 sprites appear on a scanline
    }
}

impl Default for PpuStatus {
    fn default() -> Self {
        PpuStatus::VERTICAL_BLANK | PpuStatus::SPRITE_OVERFLOW
    }
}

#[allow(clippy::unreadable_literal)] // Allow standard 6 character colour hex codes
const NES_COLOUR_MAP: [u32; 0x40] = [
    0x464646,
    0x00065a,
    0x000678,
    0x020673,
    0x35034c,
    0x57000e,
    0x5a0000,
    0x410000,
    0x120200,
    0x001400,
    0x001e00,
    0x001e00,
    0x001521,
    0x000000,
    0x000000,
    0x000000,
    0x9d9d9d,
    0x004ab9,
    0x0530e1,
    0x5718da,
    0x9f07a7,
    0xcc0255,
    0xcf0b00,
    0xa42300,
    0x5c3f00,
    0x0b5800,
    0x006600,
    0x006713,
    0x005e6e,
    0x000000,
    0x000000,
    0x000000,
    0xfeffff,
    0x1f9eff,
    0x5376ff,
    0x9865ff,
    0xfc67ff,
    0xff6cb3,
    0xff7466,
    0xff8014,
    0xc49a00,
    0x71b300,
    0x28c421,
    0x00c874,
    0x00bfd0,
    0x2b2b2b,
    0x000000,
    0x000000,
    0xfeffff,
    0x9ed5ff,
    0xafc0ff,
    0xd0b8ff,
    0xfebfff,
    0xffc0e0,
    0xffc3bd,
    0xffca9c,
    0xe7d58b,
    0xc5df8e,
    0xa6e6a3,
    0x94e8c5,
    0x92e4eb,
    0xa7a7a7,
    0x000000,
    0x000000
];


#[cfg(test)]
mod test {
    use super::*;
    use crate::nes::NES_SCREEN_DIMENSIONS;
    use bitflags::_core::fmt::{Debug, Formatter, Error};
    use std::fmt;

    fn test_colour_priority_sprite() {
        assert_eq!(0x400, 0x1 << 10)
    }

    #[test]
    fn test_vram_address_first_write() {
        let mut ppu_base = NesPpu {
            temporary_vram_address: 0b0100110_11010111,
            current_vram_address: 0b0111000_10110100,
            write_latch: false,
            ..Default::default()
        };

        let data = 0b11110011;

        let ppu_expected = NesPpu {
            temporary_vram_address: 0b0110011_11010111,
            write_latch: true,
            ..ppu_base.clone()
        };

        ppu_base.vram_address_write(data);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_vram_address_second_write() {
        let mut ppu_base = NesPpu {
            temporary_vram_address: 0b1100011_11010111,
            current_vram_address: 0b0111000_10110100,
            write_latch: true,
            ..Default::default()
        };

        let data = 0b00101000;

        let ppu_expected = NesPpu {
            temporary_vram_address: 0b1100011_00101000,
            current_vram_address: 0b1100011_00101000,
            write_latch: false,
            ..ppu_base.clone()
        };

        ppu_base.vram_address_write(data);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_scroll_first_write() {
        let mut ppu_base = NesPpu {
            temporary_vram_address: 0b1100011_11010111,
            current_vram_address: 0b0111000_10110100,
            fine_x_scroll: 0b101,
            write_latch: false,
            ..Default::default()
        };

        let data = 0b00101010;

        let ppu_expected = NesPpu {
            temporary_vram_address: 0b1100011_11000101,
            current_vram_address: 0b0111000_10110100,
            fine_x_scroll: 0b010,
            write_latch: true,
            ..ppu_base.clone()
        };

        ppu_base.scroll_write(data);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_scroll_second_write() {
        let mut ppu_base = NesPpu {
            temporary_vram_address: 0b1100011_11010111,
            current_vram_address: 0b0111000_10110100,
            fine_x_scroll: 0b101,
            write_latch: true,
            ..Default::default()
        };

        let data = 0b00101010;

        let ppu_expected = NesPpu {
            temporary_vram_address: 0b0100000_10110111,
            current_vram_address: 0b0111000_10110100,
            fine_x_scroll: 0b101,
            write_latch: false,
            ..ppu_base.clone()
        };

        ppu_base.scroll_write(data);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_coarse_x_increment_7() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b0111000_10100111,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0111000_10101000,
            ..ppu_base.clone()
        };

        ppu_base.coarse_x_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_coarse_x_increment_31() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b0111000_10111111,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0111100_10100000,
            ..ppu_base.clone()
        };

        ppu_base.coarse_x_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_y_increment_fine_4() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b1001000_10111111,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b1011000_10111111,
            ..ppu_base.clone()
        };

        ppu_base.y_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_y_increment_fine_7() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b1111000_10111111,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0001000_11011111,
            ..ppu_base.clone()
        };

        ppu_base.y_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_y_increment_fine_7_coarse_29() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b1110011_10111111,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0001000_00011111,
            ..ppu_base.clone()
        };

        ppu_base.y_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_y_increment_fine_7_coarse_31() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b1110011_11111111,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0000000_00011111,
            ..ppu_base.clone()
        };

        ppu_base.y_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    impl Default for NesPpu {
        fn default() -> Self {
            NesPpu {
                ctrl_flags: Default::default(),
                mask_flags: Default::default(),
                status_flags: Default::default(),
                oam_address: 0,
                temporary_vram_address: 0,
                current_vram_address: 0,
                fine_x_scroll: 0,
                write_latch: false,
                data_buffer: 0,
                palette_ram: Box::new([0; 32]),
                name_table: Box::new([0; 2048]),
                object_attribute_memory: Box::new([0; 256]),
                screen_buffer: Box::new([0; NES_SCREEN_DIMENSIONS]),
                scanline: 0,
                cycle: 0,
            }
        }
    }

    impl Debug for NesPpu {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.debug_struct("NesPPU")
                .field("ctrl_flags", &self.ctrl_flags)
                .field("mask_flags", &self.mask_flags)
                .field("status_flags", &self.status_flags)
                .field("temporary_vram_address", &self.temporary_vram_address)
                .field("current_vram_address", &self.current_vram_address)
                .field("fine_x_scroll", &self.fine_x_scroll)
                .field("ppu_write_latch", &self.write_latch)
                .field("ppu_data_buffer", &self.data_buffer)
                .field("scanline", &self.scanline)
                .field("cycle", &self.cycle)
                .finish()
            //TODO: Add additional fields
        }
    }

    impl PartialEq for NesPpu {
        fn eq(&self, other: &Self) -> bool {
            self.ctrl_flags == other.ctrl_flags &&
                self.mask_flags == other.mask_flags &&
                self.status_flags == other.status_flags &&
                self.temporary_vram_address == other.temporary_vram_address &&
                self.current_vram_address == other.current_vram_address &&
                self.fine_x_scroll == other.fine_x_scroll &&
                self.write_latch == other.write_latch &&
                self.data_buffer == other.data_buffer &&
                self.scanline == other.scanline &&
                self.cycle == other.cycle
            //TODO: Add additional fields
        }

        fn ne(&self, other: &Self) -> bool {
            !self.eq(other)
        }
    }
}
