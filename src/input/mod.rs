/// Interface Trait for NES Input Devices
pub trait NesInputDevice {
    /// The lower three bits of the data byte will be held and control input device behaviour.
    /// On a standard NES controller, this will load the shift registers so that they can be polled
    fn latch(&mut self, latch: u8);
    /// Polls a single bit from the controller.
    /// On a standard NES controller, this will return the next bit in the controller's shift register.
    ///
    /// The bus parameter is used for simulating open bus behaviour. It should be |ed with the three
    /// bits that were polled.
    fn poll(&mut self, bus: u8) -> u8;
}

/// Enum for representing a NES input port
pub enum NesInput<'a> {
    Disconnected,
    Connected(&'a mut dyn NesInputDevice),
}

impl NesInput<'_> {
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
    /// The bus parameter is used for simulating open bus behaviour. It should be |ed with the three
    /// bits that were polled.
    pub(crate) fn poll(&mut self, bus: u8) -> u8 {
        match self {
            NesInput::Disconnected => 0x00 | (bus & 0xf4),
            NesInput::Connected(controller) => controller.poll(bus)
        }
    }
}