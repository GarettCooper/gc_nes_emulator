/// The apu module holds the Audio Processing Unit of the NES,
/// which is responsible for all of the NES' sound. At present,
/// it is just an unimplemented stub.

/// Structure containing the registers and state of the NES'
/// Audio Processing Unit (In the real NES this is an extension
/// of the CPU, but I am representing it separately).
pub(super) struct NesApu {}

impl NesApu {
    /// Create a new instance of a NES APU
    pub fn new() -> Self {
        NesApu {}
    }
}
