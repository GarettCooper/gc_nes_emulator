//! The input module handles, as the name suggests, input devices
//! for the NES. There are a number of redundancies in this module
//! that are a remnant of an old input system but I haven't gotten
//! around to reworking it.

/// Enum for representing a NES input port
#[derive(Debug)]
pub(crate) enum NesInput {
    /// State representing no connected controller
    Disconnected,
    /// State wrapping a controller implementation
    Connected(NesInputDevice),
}

impl NesInput {
    /// The lower three bits of the data byte will be held and control input device behaviour.
    /// On a standard NES controller, this will load the shift registers so that they can be polled
    pub(crate) fn latch(&mut self, latch: u8) {
        if let NesInput::Connected(input_device) = self {
            input_device.latch(latch)
        }
    }
    /// Polls a single bit from the controller.
    /// On a standard NES controller, this will return the next bit in the controller's shift register.
    ///
    /// The bus parameter is used for simulating open bus behaviour. It should be ORed with the three
    /// bits that were polled.
    pub(crate) fn poll(&mut self, bus: u8) -> u8 {
        match self {
            NesInput::Disconnected => bus & 0xf4,
            NesInput::Connected(controller) => controller.poll(bus),
        }
    }
}

#[derive(Debug)]
pub(crate) struct NesInputDevice {
    /// Shift register that stores the button information
    shift_register: u8,
    /// Controller latch that reloads shift register when true
    reload_latch: bool,
    /// Stores the actual state of the controller
    input_state: u8,
}

impl NesInputDevice {
    /// Creates a new instance of a NesInputDevice with the starting input state
    pub(crate) fn new(input_state: u8) -> Self {
        NesInputDevice {
            shift_register: 0x00,
            reload_latch: false,
            input_state,
        }
    }

    /// Updates the internal state of the device
    pub(crate) fn update_state(&mut self, input_state: u8) {
        self.input_state = input_state;
    }

    /// The lower three bits of the data byte will be held and control input device behaviour.
    /// On a standard NES controller, this will load the shift registers so that they can be polled
    fn latch(&mut self, latch: u8) {
        self.reload_latch = latch & 0x01 == 0x01;
        self.reload_shift_register()
    }

    /// Polls a single bit from the controller.
    /// On a standard NES controller, this will return the next bit in the controller's shift register.
    ///
    /// The bus parameter is used for simulating open bus behaviour. It should be ORed with the three
    /// bits that were polled.
    fn poll(&mut self, bus: u8) -> u8 {
        self.reload_shift_register();
        // Select only the last bit of the
        let result = self.shift_register & 0x01;
        // Get the next bit in the shift register
        self.shift_register >>= 1;
        // Set the new bit to 1, which is returned after 8 polls on official NES controllers
        self.shift_register |= 0x80;
        // Return the result bit with the top 5 bits as the previous byte on the bus
        return result | (bus & 0xf8);
    }

    /// Reloads the shift register to the input state
    fn reload_shift_register(&mut self) {
        if self.reload_latch {
            self.shift_register = self.input_state;
        }
    }
}
