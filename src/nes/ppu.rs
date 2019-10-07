use crate::cartridge::Cartridge;

pub(super) struct NesPpu {
    ctrl_flags: PpuCtrl,
    mask_flags: PpuMask,
    status_flags: PpuStatus,
    oam_address: u8,
    ppu_scroll_x: u8,
    // These are one 16 bit register in the real ppu
    ppu_scroll_y: u8,
    /// Latch for multiple writes to ppu_scroll
    ppu_scroll_latch: bool,
    ppu_address: u16,
    /// Latch for multiple writes to ppu_address
    ppu_address_latch: bool,
    ppu_data_buffer: u8,
    /// Stores 4 bits of information about up to 64 sprites
    object_attribute_memory: Box<[u8; u8::max_value() as usize + 1]>,
}

impl NesPpu {
    /// Create a new instance of a NesPpu
    pub fn new() -> Self {
        NesPpu {
            ctrl_flags: Default::default(),
            mask_flags: Default::default(),
            status_flags: Default::default(),
            oam_address: 0x00,
            ppu_scroll_x: 0x00,
            ppu_scroll_y: 0x00,
            ppu_scroll_latch: false,
            ppu_address: 0x0000,
            ppu_address_latch: false,
            ppu_data_buffer: 0x00,
            object_attribute_memory: Box::new([0; u8::max_value() as usize + 1]),
        }
    }

    /// Runs a single PPU cycle, which draws a single dot into the frame buffer
    pub fn cycle(&mut self, cartridge: &Cartridge) {
        unimplemented!();
    }

    /// Function for reading from the PPU. Any address passed to the function will be mapped to one of
    /// the eight valid ppu addresses ( address % 8), equivalent to only using the lowest three bits
    pub fn read(&mut self, cartridge: &Cartridge, address: u16) -> u8 {
        match address & 0x07 {
            // Mirroring first 3 bits
            0x2000 => panic!("Attempting to read from ppu control flag"), // TODO: Check this behaviour
            0x2001 => panic!("Attempting to read from ppu mask flag"),    // TODO: Check this behaviour
            0x2002 => {
                let value = self.status_flags.bits;
                // Reset Vertical Blank flag and the two latches
                self.status_flags.set(PpuStatus::VERTICAL_BLANK, false);
                self.ppu_scroll_latch = false;
                self.ppu_address_latch = false;
                return value;
            }
            0x2003 => panic!("Attempting to read from ppu OAM address"), // TODO: Check this behaviour
            0x2004 => self.oam_read(),
            0x2005 => panic!("Attempting to read from ppu scroll address"), // TODO: Check this behaviour
            0x2006 => panic!("Attempting to read from ppu vram address"),   // TODO: Check this behaviour
            0x2007 => self.vram_read(),
            _ => panic!("Invalid PPU Read Address"), //This should never happen since its handled on a higher level
        }
    }

    /// Function for writing to the PPU. Any address passed to the function will be mapped to one of
    /// the eight valid ppu addresses ( address % 8), equivalent to only using the lowest three bits
    pub fn write(&mut self, cartridge: &mut Cartridge, address: u16, data: u8) {
        match address & 0x07 {
            // Mirroring first 3 bits
            0x0000 => self.ctrl_flags.bits = data,
            0x0001 => self.mask_flags.bits = data,
            0x0002 => warn!("Ignored attempted write to the ppu status flag. Data: {:2X}", data), // TODO: Check this behaviour
            0x0003 => self.oam_address = data,
            0x0004 => self.oam_write(data),
            0x0005 => self.scroll_write(data),
            0x0006 => self.ppu_address_write(data),
            0x0007 => self.vram_write(data),
            _ => panic!("Invalid PPU Write Address"), //This should never happen since its handled on a higher level
        }
    }

    fn oam_read(&mut self) -> u8 {
        self.object_attribute_memory[self.oam_address as usize]
    }

    fn vram_read(&mut self) -> u8 {
        unimplemented!()
    }

    fn oam_write(&mut self, data: u8) {
        self.object_attribute_memory[self.oam_address as usize] = data;
        self.oam_address += 1; // Writing to the oam address increments it
    }

    fn scroll_write(&mut self, data: u8) {
        if self.ppu_scroll_latch {
            self.ppu_scroll_y = data;
        } else {
            self.ppu_scroll_x = data;
        }
    }

    fn ppu_address_write(&mut self, data: u8) {
        if self.ppu_address_latch {
            // Second write to least significant byte
            self.ppu_address = (0xff00 & self.ppu_address) | u16::from(data)
        } else {
            // First write to most significant byte
            self.ppu_address = (0x00ff & self.ppu_address) | (u16::from(data) << 8);
            self.ppu_address_latch = true; // Set latch so next write is to lower byte
        }
    }

    fn vram_write(&mut self, data: u8) {
        unimplemented!()
    }
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
