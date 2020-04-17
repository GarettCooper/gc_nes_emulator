//! The ppu module holds all of the code related to the functioning of the
//! NES' Picture Processing Unit, which is responsible for drawing pixels
//! to the screen.

use super::emulator_6502::MOS6502;
use crate::cartridge::{Cartridge, Mirroring};
use bit_reverse::BitwiseReverse;

/// The total number of scanlines in a frame.
const MAX_SCANLINES: u16 = 261;
/// The total number of cycles in a scanline.
const MAX_CYCLES: u16 = 340;
/// The total number of cycles in a scanline minus one. This is necessary
/// because math can't be done in pattern matching expressions.
const MAX_CYCLES_MINUS_ONE: u16 = MAX_CYCLES - 1;
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
/// Structure used to hold the registers and the state of the NES Picture Processing Unit
pub(super) struct NesPpu {
    /// Register containing flags used for controlling the function of the PPU
    ctrl_flags: PpuCtrl,
    /// Register containing flags used for controlling the rendering
    mask_flags: PpuMask,
    /// Flags that can only be read, used to inform the CPU of the PPU's status
    status_flags: PpuStatus,
    /// Address pointing to the current location in the Object Attribute Memory
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
    read_buffer: u8,
    /// The pattern ram stores values used for mapping the sprite bitmaps to colours that the NES
    /// can display.
    palette_ram: Box<[u8; 0x20]>,
    /// The name table, which is used for storing the pattern id that should be displayed on the screen
    /// in a particular location. The two kilobytes of memory are both their own pattern table, which are
    /// used for scrolling.
    name_table: Box<[u8; 0x800]>,
    /// Object attribute memory stores 4 bytes of information about up to 64 sprites
    object_attribute_memory: Box<[u8; u8::max_value() as usize + 1]>,
    /// Secondary object attribute memory stores sprite information for up to 8 sprites on the
    /// scanline that is currently being rendered.
    secondary_object_attribute_memory: [u8; 0x20],
    /// The current state of the screen
    screen_buffer: Box<[u32; super::NES_SCREEN_DIMENSIONS]>,
    /// The scanline (0 to 261) of the screen that is currently being drawn
    scanline: u16,
    /// The cycle (0 to 340) of the current scanline
    cycle: u16,
    /// Counts the number of frames that have been rendered so far.
    pub(super) frame_count: u64,
    /// Latch that stores the byte of low bits from the pattern table before they are moved into the
    /// shift register.
    pattern_latch_lo: u8,
    /// Latch that stores the byte of high bits from the pattern table before they are moved into the
    /// shift register.
    pattern_latch_hi: u8,
    /// Shift register that stores the low bits from the pattern table for the next tile to be rendered
    pattern_shifter_lo: u16,
    /// Shift register that stores the high bits from the pattern table for the next tile to be rendered
    pattern_shifter_hi: u16,
    /// Latch that stores the next attribute table byte before it is moved into the shift register
    attribute_latch: u8,
    /// Shift register that stores the high bits of the attribute table information for the two tiles being rendered
    attribute_shifter_lo: u16,
    /// Shift register that stores the low bits of the attribute table information for the two tiles being rendered
    attribute_shifter_hi: u16,
    /// Buffer that stores the pattern table id read from the nametable
    nametable_id: u8,
    /// The sprite evaluation index stores which sprite in the OAM is being evaluated.
    sprite_evaluation_index: u8,
    /// The secondary sprite evaluation index stores which index in the secondary OAM the next
    /// sprite will be written to.
    secondary_sprite_evaluation_index: u8,
    /// The sprite evaluation wrapped boolean indicates whether or not the all 64 sprites have
    /// been evaluated.
    sprite_evaluation_wrapped: bool,
    /// The sprite shifters low array contains the low plane of the sprite bitmaps for up to eight
    /// sprites on a scanline.
    sprite_shifters_lo: [u8; 8],
    /// The sprite shifters low array contains the high plane of the sprite bitmaps for up to eight
    /// sprites on a scanline.
    sprite_shifters_hi: [u8; 8],
    /// The sprite attributes array contains the attribute bytes for up to eight sprites on a scanline.
    sprite_attributes: [SpriteAttribute; 8],
    /// The sprite x offset array contains the distance between the leftmost pixel of a sprite and
    /// the pixel for the current cycle.
    sprite_x_offsets: [i16; 8],
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
            read_buffer: 0x00,
            palette_ram: Box::new([0; 0x20]),
            name_table: Box::new([0; 0x800]),
            object_attribute_memory: Box::new([0xff; u8::max_value() as usize + 1]),
            secondary_object_attribute_memory: [0; 0x20],
            screen_buffer: Box::new([0; super::NES_SCREEN_DIMENSIONS]),
            scanline: 261,
            cycle: 0,
            frame_count: 0,
            pattern_latch_lo: 0,
            pattern_latch_hi: 0,
            pattern_shifter_lo: 0,
            pattern_shifter_hi: 0,
            attribute_latch: 0,
            attribute_shifter_lo: 0,
            attribute_shifter_hi: 0,
            nametable_id: 0,
            sprite_evaluation_index: 0,
            secondary_sprite_evaluation_index: 0,
            sprite_evaluation_wrapped: false,
            sprite_shifters_lo: [0; 8],
            sprite_shifters_hi: [0; 8],
            sprite_attributes: [SpriteAttribute::from_bits(0).unwrap(); 8],
            sprite_x_offsets: [0; 8],
        }
    }

    /// Runs a single PPU cycle, which draws a single pixel into the frame buffer
    pub fn cycle(&mut self, cartridge: &mut Cartridge, cpu: &mut MOS6502) {
        match self.scanline {
            MAX_SCANLINES | 0..=239 => {
                match self.cycle {
                    // Idle cycle
                    0 => {} // TODO: Accurate PPU address bus value
                    // Cycles for visible pixels
                    1..=256 | 321..=336 => {
                        // Move the shifters that store pixel information
                        if self.mask_flags.intersects(PpuMask::BACKGROUND_ENABLE) {
                            self.attribute_shifter_lo <<= 1;
                            self.attribute_shifter_hi <<= 1;
                            self.pattern_shifter_lo <<= 1;
                            self.pattern_shifter_hi <<= 1;
                        }

                        self.select_next_background_tile(cartridge);

                        self.perform_sprite_evaluation();

                        // Draw pixel to the screen during visible pixels
                        if self.cycle <= 256 && self.scanline != MAX_SCANLINES {
                            self.draw_pixel(cartridge)
                        }

                        // Special Cases!
                        if self.scanline == MAX_SCANLINES && self.cycle == 1 {
                            // Clear the status flags at the start of the pre-render scanline
                            self.status_flags.bits = 0;
                        } else if self.cycle == 256 {
                            // Increment the y address at the end of each visible scanline
                            self.y_increment()
                        }
                    }
                    257..=320 => {
                        // Perform the rest of sprite evaluation, loading the sprite data into shift
                        // registers for rendering.
                        // The real PPU does this over eight cycles, but for the time being
                        // I'm  going to do it all in one for simplicity.
                        if self.cycle % 8 == 1 {
                            // Reset on the first cycle
                            if self.cycle == 257 {
                                self.secondary_sprite_evaluation_index = 0;
                            }
                            self.load_foregroud_shifters(cartridge);
                            self.secondary_sprite_evaluation_index += 4;
                        }

                        // Special Cases!
                        match (
                            self.cycle,
                            self.scanline,
                            self.mask_flags.intersects(PpuMask::BACKGROUND_ENABLE | PpuMask::SPRITE_ENABLE),
                        ) {
                            // Load the x information from the temporary vram address into the active vram address
                            (257, _, true) => {
                                self.current_vram_address =
                                    (self.current_vram_address & !(0x400 | COARSE_X_MASK)) | (self.temporary_vram_address & (0x400 | COARSE_X_MASK))
                            }
                            (260, 0..=240, true) if self.ctrl_flags.intersects(PpuCtrl::SPRITE_SELECT) => cartridge.end_of_scanline(),
                            (324, 0..=240, true) if self.ctrl_flags.intersects(PpuCtrl::BACKGROUND_SELECT) => cartridge.end_of_scanline(),
                            // Load the y information from the temporary vram address into the active vram address repeatedly
                            (280..=304, MAX_SCANLINES, true) => {
                                self.current_vram_address = (self.current_vram_address & !(FINE_Y_MASK | 0x800 | COARSE_Y_MASK))
                                    | (self.temporary_vram_address & (FINE_Y_MASK | 0x800 | COARSE_Y_MASK))
                            }
                            _ => {}
                        }
                    } // Final four cycles just make dummy reads
                    c @ 337..=340 if c & 0x1 == 0 => {
                        cartridge.character_read(0x00);
                    } // TODO: Read from the correct location
                    // Idle cycles to simulate two cycle read time
                    337..=340 => {}
                    _ => panic!("Invalid Cycle: {}", self.cycle), // TODO: Consider unreachable!()
                }
            }
            240 => {} // Nothing happens on the first scanline off the screen
            241 => {
                if self.cycle == 1 {
                    // The vertical blank flag is set on the second cycle of scanline 241
                    self.status_flags.set(PpuStatus::VERTICAL_BLANK, true);
                    if self.ctrl_flags.intersects(PpuCtrl::NMI_ENABLE) {
                        // Trigger a non maskable interrupt on the CPU
                        cpu.non_maskable_interrupt_request();
                    }
                }
            }
            242..=260 => {}                                     // Nothing continues to happen so that CPU can manipulate PPU freely
            _ => panic!("Invalid Scanline: {}", self.scanline), // TODO: Consider unreachable!()
        }

        // Increase the cycle count and rollover the scanline if necessary
        match (self.cycle, self.scanline, self.frame_count & 0x1) {
            // On odd frames, skip the last cycle of the pre-render scanline
            (MAX_CYCLES, MAX_SCANLINES, 0) | (MAX_CYCLES_MINUS_ONE, MAX_SCANLINES, 1) => {
                self.cycle = 0;
                self.scanline = 0;
                self.frame_count += 1;
            }
            (MAX_CYCLES, _, _) => {
                self.cycle = 0;
                self.scanline += 1;
            }
            _ => self.cycle += 1,
        }
    }

    /// Loads a background tile into the latches over the course of eight cycles,
    /// then loads the latches into the background shifters.
    fn select_next_background_tile(&mut self, cartridge: &mut Cartridge) {
        match self.cycle % 8 {
            1 => {
                // Load the shifters from the latches
                self.reload_shifters();
                // Read the byte of the next pattern from the nametable
                self.nametable_id = self.vram_read(0x2000 | (self.current_vram_address & 0x0fff), cartridge);
            }
            // Read the byte from the attribute table containing palette information
            3 => self.attribute_latch = self.read_attribute_table_byte(cartridge),
            // Read the lo bits for the next 8 pixels from the pattern table.
            // To do this, the bit set in the control flag which picks the first or
            // second pattern table is combined with the pattern id in the nametable
            // and the fine y scroll in the address.
            5 => {
                self.pattern_latch_lo = self.vram_read(
                    ((self.ctrl_flags.intersects(PpuCtrl::BACKGROUND_SELECT) as u16) << 12)
                        | ((self.nametable_id as u16) << 4)
                        | (self.current_vram_address >> FINE_Y_OFFSET),
                    cartridge,
                )
            }
            // Same as above, but offset by eight pixels
            7 => {
                self.pattern_latch_hi = self.vram_read(
                    (((self.ctrl_flags.intersects(PpuCtrl::BACKGROUND_SELECT) as u16) << 12)
                        | ((self.nametable_id as u16) << 4)
                        | (self.current_vram_address >> FINE_Y_OFFSET))
                        + 8,
                    cartridge,
                )
            }
            // Increment the coarse x value every eight cycles
            0 => self.coarse_x_increment(),
            // Do nothing otherwise
            _ => {}
        }
    }

    /// Loads the secondary object attribute memory by evaluating the sprites in object attribute memory
    /// and determining which ones will appear on the next scanline. This occurs on all cycles that draw
    /// to the screen buffer.
    fn perform_sprite_evaluation(&mut self) {
        match self.cycle {
            // First 64 cycles clear the secondary oam memory
            1..=64 => {
                if self.cycle == 1 {
                    // Reset the index on the first cycle
                    self.secondary_sprite_evaluation_index = 0;
                } else if self.cycle % 2 == 0 {
                    // The actual PPU reads and writes in alternating cycles, but
                    // this is much simpler.
                    self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize] = self.oam_read();
                    self.secondary_sprite_evaluation_index += 1;
                }
            }
            // Remaining cycles in the visible scanline fill it back up again
            65..=256 => {
                // The real PPU reads from OAM take place on odd cycles and writes to secondary OAM on even cycles.
                // This would require adding a lot more intermediate state fields to the NesPpu struct so it has been
                // simplified into a single cycle here.
                if self.cycle % 2 == 0 {
                    if self.cycle == 66 {
                        // Reset the state variables on the first cycle
                        self.sprite_evaluation_index = 0;
                        self.secondary_sprite_evaluation_index = 0;
                        self.sprite_evaluation_wrapped = false;
                    }

                    let sprite_y = self.object_attribute_memory[self.sprite_evaluation_index as usize] as u16;
                    let sprite_height = if self.ctrl_flags.intersects(PpuCtrl::SPRITE_HEIGHT) { 16 } else { 8 };

                    // Prevent duplication of sprites in secondary OAM by ensuring the
                    // evaluation doesn't continue after all the sprites in OAM have
                    // been evaluated.
                    if !self.sprite_evaluation_wrapped {
                        if (self.secondary_sprite_evaluation_index as usize) < self.secondary_object_attribute_memory.len() {
                            // Copy the first 8 sprites found on the scanline into the secondary oam
                            if self.scanline >= sprite_y && self.scanline - sprite_y < sprite_height {
                                // If the sprite overlaps with the scanline, copy its object attribute
                                // data into the secondary memory for evaluation on the next scanline
                                self.secondary_object_attribute_memory
                                    [self.secondary_sprite_evaluation_index as usize..self.secondary_sprite_evaluation_index as usize + 4]
                                    .clone_from_slice(
                                        &self.object_attribute_memory
                                            [self.sprite_evaluation_index as usize..self.sprite_evaluation_index as usize + 4],
                                    );

                                // This doesn't happen in the real PPU, but I am using
                                // the unused flags in the attribute byte to keep track
                                // of which sprite is sprite zero.
                                if self.sprite_evaluation_index == 0 {
                                    self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize + 2] |=
                                        SpriteAttribute::SPRITE_ZERO.bits
                                } else {
                                    self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize + 2] &=
                                        !SpriteAttribute::SPRITE_ZERO.bits
                                }

                                self.secondary_sprite_evaluation_index += 4;
                            }
                        } else if !self.status_flags.intersects(PpuStatus::SPRITE_OVERFLOW) {
                            // Once 8 sprites have been found, we need to check if an overflow has occurred.
                            if self.scanline >= sprite_y && self.scanline - sprite_y < sprite_height {
                                // If there is another sprite on the scanline, set the overflow flag
                                self.status_flags.set(PpuStatus::SPRITE_OVERFLOW, true)
                            }
                            // There's a bug that offsets the checked address when determining
                            // if an overflow occurred, causing false negatives and positives
                            let (temp_sprite_eval, temp_bool) = self.sprite_evaluation_index.overflowing_add(1);
                            self.sprite_evaluation_index = temp_sprite_eval;
                            self.sprite_evaluation_wrapped = self.sprite_evaluation_wrapped || temp_bool;
                        }
                    }
                    let (temp_sprite_eval, temp_bool) = self.sprite_evaluation_index.overflowing_add(4);
                    self.sprite_evaluation_index = temp_sprite_eval;
                    self.sprite_evaluation_wrapped = self.sprite_evaluation_wrapped || temp_bool;
                }
            }
            _ => {}
        }
    }

    /// Loads one sprite into the different sprite shifter arrays for the next scanline based on the current position of the secondary sprite evaluation index.
    /// Normally these reads would occur over an eight cycle period, but I'm doing them all in a single cycle for convenience.
    fn load_foregroud_shifters(&mut self, cartridge: &mut Cartridge) {
        let sprite_index = self.secondary_sprite_evaluation_index as usize / 4;
        let sprite_y = self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize];
        // Skip the garbage data after all the actual sprites have been loaded
        if sprite_y != 0xff {
            let sprite_pattern_id = self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize + 1] as u16; // Cast here instead of later

            self.sprite_attributes[sprite_index] =
                SpriteAttribute::from_bits_truncate(self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize + 2]);

            // Small workaround, add one to the x offset to account for the difference between cycles and x coordinates
            self.sprite_x_offsets[sprite_index] =
                self.secondary_object_attribute_memory[self.secondary_sprite_evaluation_index as usize + 3] as i16 + 1;
            let mut sprite_pattern_row = self.scanline - sprite_y as u16;
            // If the vertical mirroring bit is set in the attribute byte
            if self.sprite_attributes[sprite_index].intersects(SpriteAttribute::VERTICAL_MIRROR) {
                // In case of a 16 pixel tall sprite, make sure only the
                // least significant 3 bits are subtracted.
                sprite_pattern_row = 0x07 - (sprite_pattern_row & 0x07);
            }

            let sprite_address: u16 = if !self.ctrl_flags.intersects(PpuCtrl::SPRITE_HEIGHT) {
                (((self.ctrl_flags & PpuCtrl::SPRITE_SELECT).bits as u16) << 8) | (sprite_pattern_id << 4) | sprite_pattern_row
            } else {
                // Determine which of the two tiles in a 16 bit sprite should be shown
                let tile_id = ((self.scanline - sprite_y as u16) >> 3) ^ (self.sprite_attributes[sprite_index].bits >> 7) as u16;
                // For 16 pixel tall sprites, the pattern table is selected
                // based on the least significant bit of the pattern id instead
                // of the nametable select flag.
                ((sprite_pattern_id & 0x01) << 12) | (((sprite_pattern_id & 0xfe) + tile_id) << 4) | (sprite_pattern_row & 0x07)
            };

            self.sprite_shifters_lo[sprite_index] = self.vram_read(sprite_address, cartridge);
            self.sprite_shifters_hi[sprite_index] = self.vram_read(sprite_address + 8, cartridge);

            if self.sprite_attributes[sprite_index].intersects(SpriteAttribute::HORIZONTAL_MIRROR) {
                self.sprite_shifters_lo[sprite_index] = self.sprite_shifters_lo[sprite_index].swap_bits();
                self.sprite_shifters_hi[sprite_index] = self.sprite_shifters_hi[sprite_index].swap_bits();
            }
        }
    }

    /// Draws a 32 bit RGB value to the screen buffer based on the background and foreground bit shifters
    fn draw_pixel(&mut self, cartridge: &mut Cartridge) {
        let (background_pixel, background_palette) = self.calculate_background_pixel();

        let (foreground_pixel, foreground_palette, foreground_priority) = self.calculate_foreground_pixel(background_pixel);

        // Determine if the background or foreground pixel takes priority
        let (pixel, palette) = NesPpu::colour_priority(
            foreground_pixel,
            foreground_palette,
            background_pixel,
            background_palette,
            foreground_priority,
        );
        self.screen_buffer[((self.cycle - 1) as usize + (self.scanline as usize * 256)) as usize] =
            NES_COLOUR_MAP[self.vram_read(0x3f00 | ((palette as u16) << 2) | pixel as u16, cartridge) as usize]
    }

    /// Calculates that background pixel and palette based on the shifters
    fn calculate_background_pixel(&mut self) -> (u8, u8) {
        let mut background_pixel = 0x00;
        let mut background_palette = 0x00;

        if self.mask_flags.intersects(PpuMask::BACKGROUND_ENABLE)
            && (!(self.cycle > 0 && self.cycle <= 8) || self.mask_flags.intersects(PpuMask::BACKGROUND_LEFT_ENABLE))
        {
            background_pixel = ((((self.pattern_shifter_hi << self.fine_x_scroll) & 0x8000) >> 14)
                | (((self.pattern_shifter_lo << self.fine_x_scroll) & 0x8000) >> 15)) as u8;

            background_palette = ((((self.attribute_shifter_hi << self.fine_x_scroll) & 0x8000) >> 14)
                | (((self.attribute_shifter_lo << self.fine_x_scroll) & 0x8000) >> 15)) as u8;
        }

        return (background_pixel, background_palette);
    }

    /// Calculates that foreground pixel and palette based on the shifters and x positions.
    /// Also uses the background_pixel parameter to determine if the sprite zero hit has occurred
    /// and set the status flag accordingly.
    fn calculate_foreground_pixel(&mut self, background_pixel: u8) -> (u8, u8, bool) {
        let mut foreground_pixel = 0x00;
        let mut foreground_palette = 0x00;
        let mut foreground_priority = false;

        for i in 0..self.sprite_x_offsets.len() {
            // Decrement all the sprite x offsets from the current pixel
            if self.sprite_x_offsets[i] > -0x8 {
                self.sprite_x_offsets[i] -= 1;
            }

            // If the x offset is in range and a higher priority sprite isn't already on this pixel
            if self.sprite_x_offsets[i] <= 0 && self.sprite_x_offsets[i] > -0x8 && foreground_pixel == 0x00 {
                if self.mask_flags.intersects(PpuMask::SPRITE_ENABLE)
                    && (!(self.cycle > 0 && self.cycle <= 8) || self.mask_flags.intersects(PpuMask::SPRITE_LEFT_ENABLE))
                {
                    foreground_pixel = (((self.sprite_shifters_hi[i] << -self.sprite_x_offsets[i]) & 0x80) >> 6)
                        | (((self.sprite_shifters_lo[i] << -self.sprite_x_offsets[i]) & 0x80) >> 7);

                    foreground_palette = (self.sprite_attributes[i] & SpriteAttribute::PALETTE).bits + 0x04;
                    foreground_priority = !self.sprite_attributes[i].intersects(SpriteAttribute::PRIORITY);
                }

                // Check for Sprite Zero hit
                if self.mask_flags.intersects(PpuMask::BACKGROUND_ENABLE | PpuMask::SPRITE_ENABLE)
                    && self.sprite_attributes[i].intersects(SpriteAttribute::SPRITE_ZERO)
                    && foreground_pixel > 0
                    && background_pixel > 0
                    // There are a couple edge cases where sprite zero hit does not occur
                    && (!(self.cycle > 0
                    && self.cycle <= 8)
                    || self.mask_flags.intersects(PpuMask::SPRITE_LEFT_ENABLE | PpuMask::BACKGROUND_LEFT_ENABLE))
                    && self.cycle != 256
                {
                    self.status_flags.set(PpuStatus::SPRITE_0_HIT, true);
                }
            }
        }

        return (foreground_pixel, foreground_palette, foreground_priority);
    }

    /// Function for reading from the PPU. Any address passed to the function will be mapped to one of
    /// the eight valid ppu addresses ( address % 8), equivalent to only using the lowest three bits
    pub fn read(&mut self, cartridge: &mut Cartridge, address: u16) -> u8 {
        match address & 0x07 {
            // Mirroring first 3 bits
            0x0000 => {
                warn!("Attempting to read from ppu control flag");
                return 0x00; // TODO: Check this behaviour
            }
            0x0001 => {
                warn!("Attempting to read from ppu mask flag");
                return 0x00; // TODO: Check this behaviour
            }
            0x0002 => {
                // When the value of the status flag is read, the bottom values retain whatever was last
                // on the PPU bus
                let value = self.status_flags.bits | (self.read_buffer & 0x1f);
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
                // Reading from the PPU is delayed by a cycle*, so return data from the last address
                // that was read from.
                let mut temp = self.read_buffer;
                self.read_buffer = self.vram_read(self.current_vram_address, cartridge);

                // *Except for reads from tha palette memory
                if self.current_vram_address >= 0x3f00 {
                    temp = self.read_buffer
                }

                // Increment the address in the x or y direction depending on a ctrl flag
                self.current_vram_address += if self.ctrl_flags.intersects(PpuCtrl::VRAM_INCREMENT) {
                    0x20
                } else {
                    0x01
                };
                return temp;
            }
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
                self.temporary_vram_address |= (data as u16 & 0b11) << 10
            }
            0x0001 => self.mask_flags.bits = data,
            0x0002 => warn!("Ignored attempted write to the ppu status flag. Data: {:2X}", data), // TODO: Check this behaviour
            0x0003 => self.oam_address = data,
            0x0004 => self.oam_write(data),
            0x0005 => self.scroll_write(data),
            0x0006 => self.vram_address_write(data),
            0x0007 => {
                self.vram_write(self.current_vram_address, data, cartridge);
                // Increment the address in the x or y direction depending on a ctrl flag
                self.current_vram_address += if self.ctrl_flags.intersects(PpuCtrl::VRAM_INCREMENT) {
                    0x20
                } else {
                    0x01
                }
            }
            _ => warn!("Invalid PPU Write Address"), // TODO: Consider unreachable!()
        }
    }

    /// Function for reading from the Object Attribute Memory
    fn oam_read(&mut self) -> u8 {
        // During the first 64 cycles of each visible scanline, reading from oam always returns 0xff.
        // This is done to reset the secondary oam.
        return if !self.status_flags.intersects(PpuStatus::VERTICAL_BLANK) && self.cycle > 0 && self.cycle <= 64 {
            0xff
        } else {
            self.object_attribute_memory[self.oam_address as usize]
        };
    }

    /// Reads from the internal bus of the PPU
    fn vram_read(&mut self, address: u16, cartridge: &mut Cartridge) -> u8 {
        return match address {
            0x0000..=0x1fff => cartridge.character_read(address),
            0x2000..=0x3eff => self.name_table[self.apply_name_table_mirroring(cartridge, address)],
            0x3f00..=0x3fff => self.palette_ram[self.apply_palette_mirroring(address)],
            _ => panic!("Attempt to read from an invalid PPU bus address: 0x{:4X}!", address),
        };
    }

    /// Function for writing to the Object Attribute Memory
    fn oam_write(&mut self, data: u8) {
        self.object_attribute_memory[self.oam_address as usize] = data;
        self.oam_address += 1; // Writing to the oam address increments it
    }

    /// Function used by the CPU during DMA to write to the PPU's OAM
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
            self.write_latch = true;
        }
    }

    /// Increment the coarse x scroll position, accounting for wrapping and name table swapping.
    fn coarse_x_increment(&mut self) {
        if self.mask_flags.intersects(PpuMask::BACKGROUND_ENABLE | PpuMask::SPRITE_ENABLE) {
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
    }

    /// Increment the y scroll value, accounting for coarse/fine bits, wrapping, and nametable swapping
    fn y_increment(&mut self) {
        if self.mask_flags.intersects(PpuMask::BACKGROUND_ENABLE | PpuMask::SPRITE_ENABLE) {
            // If the fine y value isn't at its maximum...
            if (self.current_vram_address & FINE_Y_MASK) != FINE_Y_MASK {
                // Just increment it
                self.current_vram_address += 0x1 << FINE_Y_OFFSET;
            } else {
                // Otherwise, wrap the fine y value around to 0
                self.current_vram_address &= !FINE_Y_MASK;
                // And increment coarse y, wrapping if necessary
                match (self.current_vram_address & COARSE_Y_MASK) >> COARSE_Y_OFFSET {
                    // Wrap around and flip the nametable at 29, because the last two rows are used for
                    // other data, the attribute memory.
                    0x1d => {
                        // Flip the vertical nametable
                        self.current_vram_address ^= 0x0800;
                        // Wrap around
                        self.current_vram_address &= !COARSE_Y_MASK;
                    }
                    // But if it's at 31, wrap it around without changing vertical nametables.
                    // This is to replicate specific NES behaviour
                    0x1f => self.current_vram_address &= !COARSE_Y_MASK,
                    //Otherwise, just increment coarse y
                    _ => self.current_vram_address += 0x1 << COARSE_Y_OFFSET,
                }
            }
        }
    }

    /// Loads the data from the latches into the shifters.
    fn reload_shifters(&mut self) {
        // Set all eight bits of the low bits attribute shifter to the least significant bit in the
        // attribute latch.
        self.attribute_shifter_lo = (self.attribute_shifter_lo & 0xff00) | if self.attribute_latch & 0x1 == 1 { 0xff } else { 0x00 };
        // Set all eight bits of the high bits attribute shifter to the second least significant bit
        // in the attribute latch.
        self.attribute_shifter_hi = (self.attribute_shifter_hi & 0xff00) | if self.attribute_latch & 0x2 == 2 { 0xff } else { 0x00 };
        // Set the bottom eight bits of the low pattern shifter to the bits of the low pattern latch
        self.pattern_shifter_lo = (self.pattern_shifter_lo & 0xff00) | self.pattern_latch_lo as u16;
        // Set the bottom eight bits of the high pattern shifter to the bits of the high pattern latch
        self.pattern_shifter_hi = (self.pattern_shifter_hi & 0xff00) | self.pattern_latch_hi as u16;
    }

    /// Writes onto the internal bus of the PPU.
    fn vram_write(&mut self, address: u16, data: u8, cartridge: &mut Cartridge) {
        match address {
            0x0000..=0x1fff => cartridge.character_write(address, data),
            0x2000..=0x3eff => self.name_table[self.apply_name_table_mirroring(cartridge, address)] = data,
            0x3f00..=0x3fff => self.palette_ram[self.apply_palette_mirroring(address)] = data,
            _ => panic!("Attempt to write to an invalid PPU bus address: 0x{:4X}!", address),
        }
    }

    /// Gets the screen buffer from the PPU.
    pub(super) fn get_screen(&mut self) -> &[u32; super::NES_SCREEN_DIMENSIONS] {
        return &self.screen_buffer;
    }

    /// Maps an address to a name table address by applying mirroring.
    fn apply_name_table_mirroring(&mut self, cartridge: &mut Cartridge, address: u16) -> usize {
        return match cartridge.get_mirroring() {
            Mirroring::OneScreenLower | Mirroring::OneScreenUpper => (address & 0x3ff) as usize,
            Mirroring::Vertical | Mirroring::Horizontal => {
                ((address & 0x3ff) | ((address >> (0xa | (cartridge.get_mirroring() == Mirroring::Horizontal) as u16) & 0x1) << 0xa)) as usize
            }
        };
    }

    /// Mirror palette addresses to show the universal background colour when necessary.
    /// Returns the index in the palette ram array that the address points to.
    fn apply_palette_mirroring(&self, address: u16) -> usize {
        return if address >= 0x3f10 && address & 0x03 == 0x0 {
            // Address 0x3f00 is the universal background colour, background palettes 0x3f01 through
            // 0x3f0d mirror the universal background colour with their last byte. This means
            // that a value of zero in the bitmap of a background sprite will always return the
            // universal background colour.
            address & !0x10
        } else {
            address
        } as usize
            & 0x1f; // Apply mirroring
    }

    /// Calculates the address of the attribute table byte for a location in the name table.
    fn read_attribute_table_byte(&mut self, cartridge: &mut Cartridge) -> u8 {
        // Select the correct attribute table based on the nametable bits in the vram address
        let mut attribute_address = 0x23c0 | (self.current_vram_address & 0xc00);
        // Select the attribute byte in the x direction (top 3 bits of the x component)
        attribute_address |= (self.current_vram_address & COARSE_X_MASK) >> 2;
        // Select the attribute byte in the y direction (top 3 bits of the y component moved into the correct position)
        attribute_address |= (self.current_vram_address & 0x380) >> 4;
        // Return the two bits of the attribute byte that refer to the correct quadrant
        return self.vram_read(attribute_address, cartridge)
            >> (
                // Shift the attribute byte right by four bits if we're selecting one of the bottom tiles
                (((self.current_vram_address >> COARSE_Y_OFFSET) & 0x02) << 0x1) |
                // Shift the attribute byte right two bits we're selecting one of the right tiles
                ((self.current_vram_address) & 0x02)
            )
            & 0x03; // Only return the last two bits
    }

    /// Function that determines whether the sprite or the background colour will be used for a pixel.
    fn colour_priority(
        foreground_pixel: u8,
        foreground_palette: u8,
        background_pixel: u8,
        background_palette: u8,
        foreground_priority: bool,
    ) -> (u8, u8) {
        return match (foreground_pixel, background_pixel, foreground_priority) {
            (0x00, 0x00, _) => (0x00, 0x00),
            (0x00, 0x01..=0x03, _) => (background_pixel, background_palette),
            (0x01..=0x03, 0x00, _) => (foreground_pixel, foreground_palette),
            (0x01..=0x03, 0x01..=0x03, false) => (background_pixel, background_palette),
            (0x01..=0x03, 0x01..=0x03, true) => (foreground_pixel, foreground_palette),
            _ => panic!("Invalid colour values"), // Consider unreachable!()
        };
    }

    /// Resets the state of the PPU
    pub(super) fn reset(&mut self) {
        self.ctrl_flags = Default::default();
        self.mask_flags = Default::default();
        self.status_flags = self.status_flags & PpuStatus::VERTICAL_BLANK;
        self.write_latch = false;
        self.read_buffer = 0x00;
        self.cycle = 0;
        self.scanline = 0;
        self.fine_x_scroll = 0x00;
        self.temporary_vram_address = 0x00;
    }
}

bitflags! {
    #[derive(Default)]
    struct PpuCtrl: u8 { // Labels from https://wiki.nesdev.com/w/index.php/PPU_registers
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
    struct PpuMask: u8 { // Labels from https://wiki.nesdev.com/w/index.php/PPU_registers
        const EMPHASIZE_BLUE = 0b1000_0000;
        const EMPHASIZE_GREEN = 0b0100_0000;
        const EMPHASIZE_RED = 0b0010_0000;
        const SPRITE_ENABLE = 0b0001_0000;// 1: Show sprites
        const BACKGROUND_ENABLE = 0b0000_1000;// 1: Show background
        const SPRITE_LEFT_ENABLE = 0b0000_0100;// 1: Show sprites in leftmost 8 pixels of screen, 0: Hide
        const BACKGROUND_LEFT_ENABLE = 0b0000_0010; // 1: Show background in leftmost 8 pixels of screen, 0: Hide
        const GREYSCALE = 0b0000_0001; // Greyscale (0: normal color, 1: produce a greyscale display)
    }
}

bitflags! {
    struct PpuStatus: u8 { // Labels from https://wiki.nesdev.com/w/index.php/PPU_registers
        const VERTICAL_BLANK = 0b1000_0000; // Vertical blank has started (0: not in vblank; 1: in vblank)
        const SPRITE_0_HIT = 0b0100_0000; // Sprite 0 Hit.  Set when a nonzero pixel of sprite 0 overlaps a nonzero background pixel
        const SPRITE_OVERFLOW = 0b0010_0000; // In theory is set when more than 8 sprites appear on a scanline
    }
}

bitflags! {
    #[derive(Default)]
    struct SpriteAttribute: u8 {
        const VERTICAL_MIRROR = 0b1000_0000;
        const HORIZONTAL_MIRROR = 0b0100_0000;
        /// So this is a little hack to keep track of which sprite is sprite zero. These bits are
        /// unused in the real PPU, so I am going to re-purpose them so that I can indicate in
        /// secondary OAM if a sprite is sprite zero. This is a workaround since I don't have access
        /// to the sprites original positions in OAM, which is what determines if sprite zero has
        /// been hit.
        const SPRITE_ZERO = 0b0001_1100;
        const PRIORITY = 0b0010_0000; // 1: Show background
        const PALETTE = 0b0000_0011;
    }
}

impl Default for PpuStatus {
    fn default() -> Self {
        PpuStatus::VERTICAL_BLANK | PpuStatus::SPRITE_OVERFLOW
    }
}

#[allow(clippy::unreadable_literal)] // Allow standard 6 character colour hex codes
const NES_COLOUR_MAP: [u32; 0x40] = [
    0x464646, 0x00065a, 0x000678, 0x020673, 0x35034c, 0x57000e, 0x5a0000, 0x410000, 0x120200, 0x001400, 0x001e00, 0x001e00, 0x001521, 0x000000,
    0x000000, 0x000000, 0x9d9d9d, 0x004ab9, 0x0530e1, 0x5718da, 0x9f07a7, 0xcc0255, 0xcf0b00, 0xa42300, 0x5c3f00, 0x0b5800, 0x006600, 0x006713,
    0x005e6e, 0x000000, 0x000000, 0x000000, 0xfeffff, 0x1f9eff, 0x5376ff, 0x9865ff, 0xfc67ff, 0xff6cb3, 0xff7466, 0xff8014, 0xc49a00, 0x71b300,
    0x28c421, 0x00c874, 0x00bfd0, 0x2b2b2b, 0x000000, 0x000000, 0xfeffff, 0x9ed5ff, 0xafc0ff, 0xd0b8ff, 0xfebfff, 0xffc0e0, 0xffc3bd, 0xffca9c,
    0xe7d58b, 0xc5df8e, 0xa6e6a3, 0x94e8c5, 0x92e4eb, 0xa7a7a7, 0x000000, 0x000000,
];

#[cfg(test)]
mod test {
    use super::*;
    use crate::cartridge::test_utils::*;
    use crate::nes::NES_SCREEN_DIMENSIONS;
    use std::fmt::{Debug, Formatter, Result};

    #[test]
    fn test_select_next_background_tile_cycle_0() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b0111000_10100111,
            mask_flags: PpuMask::BACKGROUND_ENABLE,
            write_latch: false,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(Default::default());

        let mut ppu_expected = NesPpu { ..ppu_base.clone() };
        ppu_expected.coarse_x_increment();

        ppu_base.select_next_background_tile(&mut cartridge);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_select_next_background_tile_cycle_1() {
        let mut ppu_base = NesPpu {
            cycle: 9,
            current_vram_address: 0b0111000_10100111,
            attribute_latch: 0b01,
            pattern_latch_lo: 0xcf,
            pattern_latch_hi: 0x4a,
            attribute_shifter_lo: 0x0000,
            attribute_shifter_hi: 0xff00,
            pattern_shifter_lo: 0x1700,
            pattern_shifter_hi: 0xa500,
            ..Default::default()
        };
        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });
        ppu_base.vram_write(0x28a7, 0x20, &mut cartridge);

        let mut ppu_expected = NesPpu {
            nametable_id: 0x20,
            ..ppu_base.clone()
        };
        ppu_expected.reload_shifters();

        ppu_base.select_next_background_tile(&mut cartridge);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_select_next_background_tile_cycle_3() {
        let mut ppu_base = NesPpu {
            cycle: 3,
            current_vram_address: 0b00000011_11000000,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });

        ppu_base.vram_write(0x23f8, 0x1 << 4, &mut cartridge);

        let mut ppu_expected = NesPpu {
            attribute_latch: 0x1,
            ..ppu_base.clone()
        };
        ppu_expected.read_attribute_table_byte(&mut cartridge);

        ppu_base.select_next_background_tile(&mut cartridge);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_select_next_background_tile_cycle_5() {
        let mut ppu_base = NesPpu {
            cycle: 5,
            ctrl_flags: PpuCtrl::BACKGROUND_SELECT,
            nametable_id: 0b10001011,
            current_vram_address: 0b10100011_11000000,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            character_read_stub: |address, _| {
                assert_eq!(0b0001_1000_1011_1010, address);
                return 0x4a;
            },
            ..Default::default()
        });

        let ppu_expected = NesPpu {
            pattern_latch_lo: 0x4a,
            ..ppu_base.clone()
        };

        ppu_base.select_next_background_tile(&mut cartridge);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_select_next_background_tile_cycle_7() {
        let mut ppu_base = NesPpu {
            cycle: 7,
            ctrl_flags: PpuCtrl::BACKGROUND_SELECT,
            nametable_id: 0b10001011,
            current_vram_address: 0b10100011_11000000,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            character_read_stub: |address, _| {
                assert_eq!(0b0001_1000_1011_1010 + 8, address);
                return 0x4a;
            },
            ..Default::default()
        });

        let ppu_expected = NesPpu {
            pattern_latch_hi: 0x4a,
            ..ppu_base.clone()
        };

        ppu_base.select_next_background_tile(&mut cartridge);
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_perform_sprite_evaluation_cycles_1_to_64() {
        let mut ppu_base = NesPpu {
            secondary_sprite_evaluation_index: 0x71,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            cycle: 64,
            secondary_sprite_evaluation_index: 32,
            secondary_object_attribute_memory: [0xff; 32],
            ..ppu_base.clone()
        };

        for i in 0..=64 {
            ppu_base.cycle = i;
            ppu_base.perform_sprite_evaluation();
        }
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_perform_sprite_evaluation_cycle_66() {
        let mut ppu_base = NesPpu {
            cycle: 66,
            scanline: 20,
            ..Default::default()
        };
        ppu_base.object_attribute_memory[0..4].copy_from_slice(&[40, 0x16, SpriteAttribute::PALETTE.bits, 100]);

        let ppu_expected = NesPpu {
            secondary_sprite_evaluation_index: 0,
            sprite_evaluation_index: 4,
            ..ppu_base.clone()
        };

        ppu_base.perform_sprite_evaluation();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_perform_sprite_evaluation_on_scanline() {
        let mut ppu_base = NesPpu {
            cycle: 68,
            scanline: 40,
            sprite_evaluation_index: 4,
            ..Default::default()
        };
        ppu_base.object_attribute_memory[4..8].copy_from_slice(&[40, 0x16, SpriteAttribute::PALETTE.bits, 100]);

        let mut ppu_expected = NesPpu {
            secondary_sprite_evaluation_index: 4,
            sprite_evaluation_index: 8,
            ..ppu_base.clone()
        };
        ppu_expected.secondary_object_attribute_memory[0..4].copy_from_slice(&ppu_base.object_attribute_memory[4..8]);

        ppu_base.perform_sprite_evaluation();
        assert_eq!(
            ppu_expected.secondary_object_attribute_memory[0..4],
            ppu_base.secondary_object_attribute_memory[0..4],
        );
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_perform_sprite_evaluation_on_scanline_sprite_zero() {
        let mut ppu_base = NesPpu {
            cycle: 66,
            scanline: 40,
            ..Default::default()
        };
        ppu_base.object_attribute_memory[0..4].copy_from_slice(&[40, 0x16, SpriteAttribute::PALETTE.bits, 100]);

        let mut ppu_expected = NesPpu {
            secondary_sprite_evaluation_index: 4,
            sprite_evaluation_index: 4,
            ..ppu_base.clone()
        };
        ppu_expected.secondary_object_attribute_memory[0..4].copy_from_slice(&ppu_base.object_attribute_memory[0..4]);
        ppu_expected.secondary_object_attribute_memory[2] |= SpriteAttribute::SPRITE_ZERO.bits;

        ppu_base.perform_sprite_evaluation();
        assert_eq!(
            ppu_expected.secondary_object_attribute_memory[0..4],
            ppu_base.secondary_object_attribute_memory[0..4],
        );
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_perform_sprite_evaluation_overflow() {
        let mut ppu_base = NesPpu {
            cycle: 122,
            scanline: 40,
            sprite_evaluation_index: 32,
            secondary_sprite_evaluation_index: 32,
            status_flags: PpuStatus::from_bits_truncate(0),
            ..Default::default()
        };
        ppu_base.object_attribute_memory[32..36].copy_from_slice(&[40, 0x16, SpriteAttribute::PALETTE.bits, 100]);

        let ppu_expected = NesPpu {
            sprite_evaluation_index: 37,
            status_flags: PpuStatus::SPRITE_OVERFLOW,
            ..ppu_base.clone()
        };

        ppu_base.perform_sprite_evaluation();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_perform_sprite_evaluation_overflow_search_bug() {
        let mut ppu_base = NesPpu {
            cycle: 122,
            scanline: 40,
            sprite_evaluation_index: 32,
            secondary_sprite_evaluation_index: 32,
            status_flags: PpuStatus::from_bits_truncate(0),
            ..Default::default()
        };
        ppu_base.object_attribute_memory[32..36].copy_from_slice(&[80, 0x16, SpriteAttribute::PALETTE.bits, 100]);

        let ppu_expected = NesPpu {
            sprite_evaluation_index: 37,
            ..ppu_base.clone()
        };

        ppu_base.perform_sprite_evaluation();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_background_pixel() {
        let mut ppu_base = NesPpu {
            pattern_shifter_lo: 0b0001_0000_0000_0000,
            pattern_shifter_hi: 0b0001_0000_0000_0000,
            attribute_shifter_lo: 0b1111_1111_1111_1111,
            attribute_shifter_hi: 0b0000_0000_0000_0000,
            fine_x_scroll: 3,
            mask_flags: PpuMask::BACKGROUND_ENABLE | PpuMask::BACKGROUND_LEFT_ENABLE,
            cycle: 6,
            ..Default::default()
        };

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!((0b11, 0b01), ppu_base.calculate_background_pixel());
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_background_pixel_background_disabled() {
        let mut ppu_base = NesPpu {
            pattern_shifter_lo: 0b0001_0000_0000_0000,
            pattern_shifter_hi: 0b0001_0000_0000_0000,
            attribute_shifter_lo: 0b1111_1111_1111_1111,
            attribute_shifter_hi: 0b0000_0000_0000_0000,
            fine_x_scroll: 3,
            mask_flags: PpuMask::from_bits_truncate(0),
            cycle: 6,
            ..Default::default()
        };

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!((0b00, 0b00), ppu_base.calculate_background_pixel());
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_background_pixel_background_left_disabled_cycle_6() {
        let mut ppu_base = NesPpu {
            pattern_shifter_lo: 0b0001_0000_0000_0000,
            pattern_shifter_hi: 0b0001_0000_0000_0000,
            attribute_shifter_lo: 0b1111_1111_1111_1111,
            attribute_shifter_hi: 0b0000_0000_0000_0000,
            fine_x_scroll: 3,
            mask_flags: PpuMask::BACKGROUND_ENABLE,
            cycle: 6,
            ..Default::default()
        };

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!((0b00, 0b00), ppu_base.calculate_background_pixel());
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_background_pixel_cycle_9() {
        let mut ppu_base = NesPpu {
            pattern_shifter_lo: 0b0001_0000_0000_0000,
            pattern_shifter_hi: 0b0001_0000_0000_0000,
            attribute_shifter_lo: 0b1111_1111_1111_1111,
            attribute_shifter_hi: 0b0000_0000_0000_0000,
            fine_x_scroll: 3,
            mask_flags: PpuMask::BACKGROUND_ENABLE,
            cycle: 9,
            ..Default::default()
        };

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!((0b11, 0b01), ppu_base.calculate_background_pixel());
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_foreground_pixel() {
        let mut ppu_base = NesPpu {
            mask_flags: PpuMask::SPRITE_ENABLE | PpuMask::SPRITE_LEFT_ENABLE,
            cycle: 9,
            sprite_x_offsets: [-8; 8],
            sprite_attributes: [SpriteAttribute::from_bits_truncate(0); 8],
            sprite_shifters_lo: [0; 8],
            sprite_shifters_hi: [0; 8],
            status_flags: PpuStatus::from_bits(0).unwrap(),
            ..Default::default()
        };
        ppu_base.sprite_x_offsets[2] = 4;
        ppu_base.sprite_shifters_lo[2] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[2] = 0b0000_1000;
        ppu_base.sprite_attributes[2] = SpriteAttribute::from_bits(0).unwrap() | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[3] = -3;
        ppu_base.sprite_shifters_lo[3] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[3] = 0b0000_1000;
        ppu_base.sprite_attributes[3] = SpriteAttribute::PALETTE | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[4] = -3;
        ppu_base.sprite_shifters_lo[4] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[4] = 0b0000_1000;
        ppu_base.sprite_attributes[4] = SpriteAttribute::from_bits(1).unwrap() | SpriteAttribute::PRIORITY;

        let mut ppu_expected = NesPpu { ..ppu_base.clone() };
        ppu_expected.sprite_x_offsets[2..5].clone_from_slice(&[3, -4, -4]);

        assert_eq!((0b11, 0b11 + 4, false), ppu_base.calculate_foreground_pixel(0b01));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_foreground_pixel_foreground_disabled() {
        let mut ppu_base = NesPpu {
            mask_flags: PpuMask::from_bits(0).unwrap(),
            cycle: 9,
            sprite_x_offsets: [-8; 8],
            sprite_attributes: [SpriteAttribute::from_bits_truncate(0); 8],
            sprite_shifters_lo: [0; 8],
            sprite_shifters_hi: [0; 8],
            status_flags: PpuStatus::from_bits(0).unwrap(),
            ..Default::default()
        };
        ppu_base.sprite_x_offsets[2] = -2;
        ppu_base.sprite_shifters_lo[2] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[2] = 0b0000_1000;
        ppu_base.sprite_attributes[2] = SpriteAttribute::from_bits(0).unwrap() | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[3] = -3;
        ppu_base.sprite_shifters_lo[3] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[3] = 0b0000_1000;
        ppu_base.sprite_attributes[3] = SpriteAttribute::PALETTE | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[4] = -3;
        ppu_base.sprite_shifters_lo[4] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[4] = 0b0000_1000;
        ppu_base.sprite_attributes[4] = SpriteAttribute::from_bits(1).unwrap() | SpriteAttribute::PRIORITY;

        let mut ppu_expected = NesPpu { ..ppu_base.clone() };
        ppu_expected.sprite_x_offsets[2..5].clone_from_slice(&[-3, -4, -4]);

        assert_eq!((0b00, 0b00, false), ppu_base.calculate_foreground_pixel(0b01));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_foreground_pixel_foreground_left_disabled() {
        let mut ppu_base = NesPpu {
            mask_flags: PpuMask::SPRITE_ENABLE,
            cycle: 6,
            sprite_x_offsets: [-8; 8],
            sprite_attributes: [SpriteAttribute::from_bits_truncate(0); 8],
            sprite_shifters_lo: [0; 8],
            sprite_shifters_hi: [0; 8],
            status_flags: PpuStatus::from_bits(0).unwrap(),
            ..Default::default()
        };
        ppu_base.sprite_x_offsets[2] = -2;
        ppu_base.sprite_shifters_lo[2] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[2] = 0b0000_1000;
        ppu_base.sprite_attributes[2] = SpriteAttribute::from_bits(0).unwrap() | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[3] = -3;
        ppu_base.sprite_shifters_lo[3] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[3] = 0b0000_1000;
        ppu_base.sprite_attributes[3] = SpriteAttribute::PALETTE | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[4] = -3;
        ppu_base.sprite_shifters_lo[4] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[4] = 0b0000_1000;
        ppu_base.sprite_attributes[4] = SpriteAttribute::from_bits(1).unwrap() | SpriteAttribute::PRIORITY;

        let mut ppu_expected = NesPpu { ..ppu_base.clone() };
        ppu_expected.sprite_x_offsets[2..5].clone_from_slice(&[-3, -4, -4]);

        assert_eq!((0b00, 0b00, false), ppu_base.calculate_foreground_pixel(0b01));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_calculate_foreground_pixel_sprite_zero() {
        let mut ppu_base = NesPpu {
            mask_flags: PpuMask::SPRITE_ENABLE,
            cycle: 9,
            sprite_x_offsets: [-8; 8],
            sprite_attributes: [SpriteAttribute::from_bits_truncate(0); 8],
            sprite_shifters_lo: [0; 8],
            sprite_shifters_hi: [0; 8],
            status_flags: PpuStatus::from_bits(0).unwrap(),
            ..Default::default()
        };
        ppu_base.sprite_x_offsets[2] = -3;
        ppu_base.sprite_shifters_lo[2] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[2] = 0b0000_1000;
        ppu_base.sprite_attributes[2] = SpriteAttribute::from_bits(2).unwrap() | SpriteAttribute::PRIORITY | SpriteAttribute::SPRITE_ZERO;

        ppu_base.sprite_x_offsets[3] = -3;
        ppu_base.sprite_shifters_lo[3] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[3] = 0b0000_1000;
        ppu_base.sprite_attributes[3] = SpriteAttribute::PALETTE | SpriteAttribute::PRIORITY;

        ppu_base.sprite_x_offsets[4] = -3;
        ppu_base.sprite_shifters_lo[4] = 0b0000_1000;
        ppu_base.sprite_shifters_hi[4] = 0b0000_1000;
        ppu_base.sprite_attributes[4] = SpriteAttribute::from_bits(1).unwrap() | SpriteAttribute::PRIORITY;

        let mut ppu_expected = NesPpu {
            status_flags: PpuStatus::SPRITE_0_HIT,
            ..ppu_base.clone()
        };
        ppu_expected.sprite_x_offsets[2..5].clone_from_slice(&[-4, -4, -4]);

        assert_eq!((0b11, 0b10 + 4, false), ppu_base.calculate_foreground_pixel(0b01));
        assert_eq!(ppu_expected, ppu_base)
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
            mask_flags: PpuMask::BACKGROUND_ENABLE,
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
            mask_flags: PpuMask::BACKGROUND_ENABLE,
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
    fn test_coarse_x_increment_disabled() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b0111000_10111111,
            mask_flags: PpuMask::from_bits(0x00).unwrap(),
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0111000_10111111,
            ..ppu_base.clone()
        };

        ppu_base.coarse_x_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_y_increment_fine_4() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b1001000_10111111,
            mask_flags: PpuMask::BACKGROUND_ENABLE,
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
            mask_flags: PpuMask::BACKGROUND_ENABLE,
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
            mask_flags: PpuMask::BACKGROUND_ENABLE,
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
            mask_flags: PpuMask::BACKGROUND_ENABLE,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0000000_00011111,
            ..ppu_base.clone()
        };

        ppu_base.y_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_y_increment_disabled() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b0111000_10111111,
            mask_flags: PpuMask::from_bits(0x00).unwrap(),
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            current_vram_address: 0b0111000_10111111,
            ..ppu_base.clone()
        };

        ppu_base.y_increment();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_read_attribute_table_byte_top_left() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b00000000_00000000,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });

        ppu_base.vram_write(0x23C0, 0x2, &mut cartridge);

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x2, ppu_base.read_attribute_table_byte(&mut cartridge));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_read_attribute_table_byte_top_right() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b00000000_00011110,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });

        ppu_base.vram_write(0x23C7, 0x3 << 2, &mut cartridge);

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x3, ppu_base.read_attribute_table_byte(&mut cartridge));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_read_attribute_table_byte_bottom_left() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b00000011_11000000,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });

        ppu_base.vram_write(0x23f8, 0x1 << 4, &mut cartridge);

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x1, ppu_base.read_attribute_table_byte(&mut cartridge));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_read_attribute_table_byte_bottom_right() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b00000011_11011110,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });

        ppu_base.vram_write(0x23ff, 0x2 << 6, &mut cartridge);

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x2, ppu_base.read_attribute_table_byte(&mut cartridge));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_read_attribute_table_byte_other_nametable() {
        let mut ppu_base = NesPpu {
            current_vram_address: 0b00001011_11011110,
            ..Default::default()
        };

        let mut cartridge = get_mock_cartridge(MapperMock {
            get_mirroring_stub: |_| Mirroring::Horizontal,
            ..Default::default()
        });

        ppu_base.vram_write(0x2bff, 0x2 << 6, &mut cartridge);

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x2, ppu_base.read_attribute_table_byte(&mut cartridge));
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_reload_shifters_attribute_0b01() {
        let mut ppu_base = NesPpu {
            attribute_latch: 0b01,
            pattern_latch_lo: 0xcf,
            pattern_latch_hi: 0x4a,
            attribute_shifter_lo: 0x0000,
            attribute_shifter_hi: 0xff00,
            pattern_shifter_lo: 0x1700,
            pattern_shifter_hi: 0xa500,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            attribute_latch: 0b01,
            pattern_latch_lo: 0xcf,
            pattern_latch_hi: 0x4a,
            attribute_shifter_lo: 0x00ff,
            attribute_shifter_hi: 0xff00,
            pattern_shifter_lo: 0x1700 | 0xcf,
            pattern_shifter_hi: 0xa500 | 0x4a,
            ..ppu_base.clone()
        };

        ppu_base.reload_shifters();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_reload_shifters_attribute_0b10() {
        let mut ppu_base = NesPpu {
            attribute_latch: 0b10,
            pattern_latch_lo: 0x91,
            pattern_latch_hi: 0xaa,
            attribute_shifter_lo: 0x0000,
            attribute_shifter_hi: 0xff00,
            pattern_shifter_lo: 0x00,
            pattern_shifter_hi: 0xcd00,
            ..Default::default()
        };

        let ppu_expected = NesPpu {
            attribute_latch: 0b10,
            pattern_latch_lo: 0x91,
            pattern_latch_hi: 0xaa,
            attribute_shifter_lo: 0x0000,
            attribute_shifter_hi: 0xffff,
            pattern_shifter_lo: 0x91,
            pattern_shifter_hi: 0xcd00 | 0xaa,
            ..ppu_base.clone()
        };

        ppu_base.reload_shifters();
        assert_eq!(ppu_expected, ppu_base)
    }

    #[test]
    fn test_oam_read() {
        let mut ppu_base = NesPpu {
            oam_address: 0x00,
            status_flags: PpuStatus::SPRITE_0_HIT,
            ..Default::default()
        };

        ppu_base.object_attribute_memory[ppu_base.oam_address as usize] = 0x20;

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x20, ppu_base.oam_read());
        assert_eq!(ppu_expected, ppu_base);
    }

    #[test]
    fn test_oam_read_cycle_64() {
        let mut ppu_base = NesPpu {
            oam_address: 0x00,
            cycle: 64,
            status_flags: PpuStatus::SPRITE_0_HIT,
            ..Default::default()
        };

        ppu_base.object_attribute_memory[ppu_base.oam_address as usize] = 0x20;

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0xff, ppu_base.oam_read());
        assert_eq!(ppu_expected, ppu_base);
    }

    #[test]
    fn test_oam_read_cycle_64_vblank() {
        let mut ppu_base = NesPpu {
            oam_address: 0x00,
            cycle: 64,
            status_flags: PpuStatus::VERTICAL_BLANK,
            ..Default::default()
        };

        ppu_base.object_attribute_memory[ppu_base.oam_address as usize] = 0x20;

        let ppu_expected = NesPpu { ..ppu_base.clone() };

        assert_eq!(0x20, ppu_base.oam_read());
        assert_eq!(ppu_expected, ppu_base);
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
                read_buffer: 0,
                palette_ram: Box::new([0; 32]),
                name_table: Box::new([0; 2048]),
                object_attribute_memory: Box::new([0; 256]),
                secondary_object_attribute_memory: [0; 32],
                screen_buffer: Box::new([0; NES_SCREEN_DIMENSIONS]),
                scanline: 0,
                cycle: 0,
                frame_count: 0,
                pattern_latch_lo: 0,
                pattern_latch_hi: 0,
                pattern_shifter_lo: 0,
                pattern_shifter_hi: 0,
                attribute_latch: 0,
                attribute_shifter_lo: 0,
                attribute_shifter_hi: 0,
                nametable_id: 0,
                sprite_evaluation_index: 0,
                secondary_sprite_evaluation_index: 0,
                sprite_evaluation_wrapped: false,
                sprite_shifters_lo: [0; 8],
                sprite_shifters_hi: [0; 8],
                sprite_attributes: [Default::default(); 8],
                sprite_x_offsets: [0; 8],
            }
        }
    }

    impl Debug for NesPpu {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("NesPPU")
                .field("ctrl_flags", &self.ctrl_flags)
                .field("mask_flags", &self.mask_flags)
                .field("status_flags", &self.status_flags)
                .field("temporary_vram_address", &self.temporary_vram_address)
                .field("current_vram_address", &self.current_vram_address)
                .field("fine_x_scroll", &self.fine_x_scroll)
                .field("ppu_write_latch", &self.write_latch)
                .field("ppu_data_buffer", &self.read_buffer)
                .field("scanline", &self.scanline)
                .field("cycle", &self.cycle)
                .field("frame_count", &self.frame_count)
                .field("pattern_latch_lo", &self.pattern_latch_lo)
                .field("pattern_latch_hi", &self.pattern_latch_hi)
                .field("pattern_shifter_lo", &self.pattern_shifter_lo)
                .field("pattern_shifter_hi", &self.pattern_shifter_hi)
                .field("attribute_latch", &self.attribute_latch)
                .field("attribute_shifter_lo", &self.attribute_shifter_lo)
                .field("attribute_shifter_hi", &self.attribute_shifter_hi)
                .field("nametable_id", &self.nametable_id)
                .field("sprite_evaluation_index", &self.sprite_evaluation_index)
                .field("secondary_sprite_evaluation_index", &self.secondary_sprite_evaluation_index)
                .field("sprite_evaluation_wrapped", &self.sprite_evaluation_wrapped)
                .field("sprite_shifters_lo", &self.sprite_shifters_lo)
                .field("sprite_shifters_hi", &self.sprite_shifters_hi)
                .field("sprite_attributes", &self.sprite_attributes)
                .field("sprite_x_offsets", &self.sprite_x_offsets)
                .finish()
            //TODO: Add additional fields
        }
    }

    impl PartialEq for NesPpu {
        fn eq(&self, other: &Self) -> bool {
            self.ctrl_flags == other.ctrl_flags
                && self.mask_flags == other.mask_flags
                && self.status_flags == other.status_flags
                && self.temporary_vram_address == other.temporary_vram_address
                && self.current_vram_address == other.current_vram_address
                && self.fine_x_scroll == other.fine_x_scroll
                && self.write_latch == other.write_latch
                && self.read_buffer == other.read_buffer
                && self.scanline == other.scanline
                && self.cycle == other.cycle
                && self.frame_count == other.frame_count
                && self.pattern_latch_lo == other.pattern_latch_lo
                && self.pattern_latch_hi == other.pattern_latch_hi
                && self.pattern_shifter_lo == other.pattern_shifter_lo
                && self.pattern_shifter_hi == other.pattern_shifter_hi
                && self.attribute_latch == other.attribute_latch
                && self.attribute_shifter_lo == other.attribute_shifter_lo
                && self.attribute_shifter_hi == other.attribute_shifter_hi
                && self.nametable_id == other.nametable_id
                && self.sprite_evaluation_index == other.sprite_evaluation_index
                && self.secondary_sprite_evaluation_index == other.secondary_sprite_evaluation_index
                && self.sprite_evaluation_wrapped == other.sprite_evaluation_wrapped
                && self.sprite_shifters_lo == other.sprite_shifters_lo
                && self.sprite_shifters_hi == other.sprite_shifters_hi
                && self.sprite_attributes == other.sprite_attributes
                && self.sprite_x_offsets == other.sprite_x_offsets
            //TODO: Add additional fields
        }
    }
}
