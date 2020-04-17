//! The nes module contains the connective tissue of the NES.
//! It contains the the code for communication between the CPU
//! and the PPU.

extern crate emulator_6502;

use crate::cartridge::Cartridge;
use crate::input::{NesInput, NesInputDevice};
use crate::nes::apu::NesApu;
use crate::nes::ppu::NesPpu;
use emulator_6502::{Interface6502, MOS6502};

mod apu;
mod ppu;

/// The dimensions of NES screen in pixels
pub const NES_SCREEN_DIMENSIONS: usize = 256 * 240;

/// Struct that represents the NES itself
pub struct Nes {
    // NES Components-----------------------------------------------------------------------------------------------------------------
    /// The cpu of the NES
    ///
    /// The actual NES used a 2A03 which combined the cpu and apu functionality, but they are represented separately here
    cpu: MOS6502,
    /// The bus of the NES, which holds ownership of the other components
    bus: Bus,
    // Additional Tracking Information------------------------------------------------------------------------------------------------
    /// The number of cycles that have been executed so far
    cycle_count: u64,
}

/// Struct that represents the NES components that are connected to the main bus.
/// The primary reasons for this classes existence is to allow for reading and writing by the cpu
/// after the NES has been decomposed.
struct Bus {
    /// The cartridge loaded into the NES
    cartridge: Box<Cartridge>,
    /// The picture processing unit of the NES
    ppu: NesPpu,
    /// The audio processing unit of the NES             
    apu: NesApu,
    /// The NES' two kilobytes of ram               
    ram: Box<[u8; 0x0800]>,
    /// The first input device connected to the NES
    input_device_one: NesInput,
    /// The second input device connected to the NES
    input_device_two: NesInput,
    /// The status of the OAM DMA process. When OAM DMA is activated the value is set to Some(DmaStatus)
    dma_status: Option<DmaStatus>,
}

/// Struct that wraps an option to represent if oam dma is in progress and how far along it is.
/// If the value is None, no DMA is in progress.
/// If the value is Some(n), DMA has been running for n cycles.
#[derive(Debug, Clone, Copy)]
struct DmaStatus {
    /// A latch to ensure that DMA waits 1 or 2 cycles before beginning to copy data
    dma_wait: bool,
    /// The address that DMA begins to copy from
    dma_start_address: u16,
    /// The number of bytes that DMA has copied so far
    dma_count: u8,
    /// A buffer for data read from RAM that will be written to OAM on the next cycle
    dma_buffer: u8,
}

impl Nes {
    /// Creates a new NES instance with no connected controllers
    pub fn new(cartridge: Cartridge) -> Self {
        let mut bus = Bus {
            cartridge: Box::new(cartridge),
            ppu: NesPpu::new(),
            apu: NesApu::new(),
            ram: Box::new([0; 0x0800]),
            input_device_one: NesInput::Disconnected,
            input_device_two: NesInput::Disconnected,
            dma_status: None,
        };

        Nes {
            cpu: MOS6502::new_reset_position(&mut bus),
            bus,
            cycle_count: 0,
        }
    }

    /// Executes a single cycle of the NES
    pub fn cycle(&mut self) {
        if self.cycle_count % 3 == 0 {
            //Copy the dma_status so that the bus is not decomposed which would prevent calling methods on it in the match statement
            let mut dma_status = self.bus.dma_status;
            // This was created as a personal exercise in pattern matching, but isn't very readable.
            // I should consider alternatives.
            match (self.cycle_count, &mut dma_status) {
                // DMA disabled, CPU cycles every third ppu dot
                (_, None) => {
                    self.cpu.cycle(&mut self.bus);
                    // DMA status may have been changed, copy it back
                    dma_status = self.bus.dma_status;
                }
                // DMA ENABLED ------------------------------------------------------------------------------------------------------------
                // DMA can only start on an even clock cycle
                (c, Some(DmaStatus { dma_wait: wait @ true, .. })) if c % 2 == 1 => {
                    trace!("DMA Initiated on cycle: {}!", self.cycle_count);
                    *wait = false;
                }
                // DMA must wait a clock cycle for reads to be resolved
                (_, Some(DmaStatus { dma_wait: true, .. })) => (),
                // DMA reads from memory on even clock cycles
                (
                    c,
                    Some(DmaStatus {
                        dma_wait: false,
                        dma_start_address,
                        dma_count,
                        dma_buffer,
                    }),
                ) if c % 2 == 0 => {
                    *dma_buffer = self.bus.read(*dma_start_address + *dma_count as u16);
                }
                // And writes to OAM on odd clock cycles
                (
                    _,
                    Some(DmaStatus {
                        dma_wait: false,
                        dma_count,
                        dma_buffer,
                        ..
                    }),
                ) => {
                    self.bus.ppu.oam_dma_write(*dma_count, *dma_buffer);
                    *dma_count = dma_count.wrapping_add(1);
                    // When the count has wrapped around, the DMA is over
                    if *dma_count == 0 {
                        dma_status = None;
                        trace!("DMA ended on cycle: {}!", self.cycle_count);
                    }
                }
            }
            self.bus.dma_status = dma_status;
        }
        // PPU cycle runs regardless
        self.bus.ppu.cycle(&mut self.bus.cartridge, &mut self.cpu);

        // Check if the Cartridge is triggering an interrupt
        if self.bus.cartridge.get_pending_interrupt_request() {
            self.cpu.interrupt_request();
        }

        self.cycle_count += 1;
    }

    /// Runs as many cycles as necessary to complete the current frame.
    /// Returns the frame as an of 32 bit colour ARGB colour values.
    pub fn frame(&mut self) -> &[u32; NES_SCREEN_DIMENSIONS] {
        let current_frame = self.bus.ppu.frame_count;
        while self.bus.ppu.frame_count == current_frame {
            self.cycle();
        }
        return self.get_screen();
    }

    /// Updates the state of the input device connected to the first port
    pub fn update_controller_one(&mut self, input_state: Option<u8>) {
        match (&mut self.bus.input_device_one, input_state) {
            (NesInput::Disconnected, None) => {}
            (NesInput::Connected(_), None) => self.bus.input_device_one = NesInput::Disconnected,
            (NesInput::Disconnected, Some(state)) => self.bus.input_device_one = NesInput::Connected(NesInputDevice::new(state)),
            (NesInput::Connected(ref mut device), Some(state)) => device.update_state(state),
        }
    }

    /// Updates the state of the input device connected to the first port
    pub fn update_controller_two(&mut self, input_state: Option<u8>) {
        match (&mut self.bus.input_device_two, input_state) {
            (NesInput::Disconnected, None) => {}
            (NesInput::Connected(_), None) => self.bus.input_device_two = NesInput::Disconnected,
            (NesInput::Disconnected, Some(state)) => self.bus.input_device_two = NesInput::Connected(NesInputDevice::new(state)),
            (NesInput::Connected(ref mut device), Some(state)) => device.update_state(state),
        }
    }

    /// Gets the current state of the screen from the PPU's screen buffer
    pub fn get_screen(&mut self) -> &[u32; NES_SCREEN_DIMENSIONS] {
        self.bus.ppu.get_screen()
    }

    /// Resets the state of the console
    pub fn reset(&mut self) {
        self.cycle_count = 0;
        self.cpu.reset(&mut self.bus);
        self.bus.reset();
    }
}

impl Bus {
    /// Resets the state of the console components on the bus
    fn reset(&mut self) {
        self.ppu.reset();
        // self.apu.reset();
    }
}

impl Interface6502 for Bus {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            0x0000..=0x1fff => self.ram[usize::from(address) & 0x07ff], // Addresses 0x0800-0x1fff mirror the 2KiB of ram
            0x2000..=0x3fff => self.ppu.read(&mut self.cartridge, address), // Mirroring will be done by the ppu
            0x4000..=0x4015 => 0x00,                                    // self.apu.read(address)
            0x4016 => self.input_device_one.poll(0x00),                 // Read one bit from the first controller TODO: Open Bus Behaviour
            0x4017 => self.input_device_two.poll(0x00),                 // Read one bit from the second controller
            0x4018..=0x401f => 0x00,                                    // Usually disabled on the nes TODO: Decide how to handle these
            0x4020..=0xffff => self.cartridge.program_read(address),    // Addresses above 0x4020 read from the cartridge
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            0x0000..=0x1fff => self.ram[usize::from(address) & 0x07ff] = data, // Addresses 0x0800-0x1fff mirror the 2KiB of ram
            0x2000..=0x3fff => self.ppu.write(&mut self.cartridge, address, data), // Mirroring will be done by the ppu
            0x4000..=0x4013 => warn!("APU Write Unimplemented"),               // self.apu.write(address, data)
            0x4014 => self.dma_status = Some(DmaStatus::new(data)),            // Begins the OAM DMA operation at the data page
            0x4015 => warn!("APU Write Unimplemented"),
            0x4016 => {
                self.input_device_one.latch(data); // Set the shift register reload latch on the both controllers
                self.input_device_two.latch(data);
            }
            0x4017 => warn!("APU Write Unimplemented"), // Writing to the second controller address is undefined
            0x4018..=0x401f => unimplemented!(),        // Usually disabled on the nes
            0x4020..=0xffff => self.cartridge.program_write(address, data), // Addresses above 0x4020 write to the cartridge
        }
    }
}

impl DmaStatus {
    /// Create a new DmaStatus instance
    fn new(page: u8) -> Self {
        DmaStatus {
            dma_wait: true,
            dma_start_address: (page as u16) << 8,
            dma_count: 0,
            dma_buffer: 0,
        }
    }
}

// TODO: Write DMA tests
