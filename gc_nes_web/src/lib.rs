//! gc_nes_web wraps the public functions exposed by my gc_nes_core crate for use
//! in the browser through Web Assembly - Javascript interop. It provides an interface
//! to load and run NES ROMs, provide input, and extract rendered image data.
//! Audio is currently unsupported.
//!
//! ### Using the NES Emulator with Javascript
//! ```javascript
//! // Import the package
//! const wasm = await import ("gc_nes_web");
//! // Create the NES object
//! let nes = this.state.wasm.nes(romArrayOfBytes);
//! // Run the emulator to the completion of the next frame and retrieve it
//! let frame = nes.frame();
//! // Or run just one cycle and get the frame separately
//! nes.cycle();
//! let frame = nes.get_screen();
//! // Drawing to a Canvas
//! let offscreenCanvas = new OffscreenCanvas(256, 240);
//! let offscreenCanvasContext = offscreenCanvas.getContext("2d");
//! let imageData = offscreenCanvasContext?.createImageData(256, 240);
//! imageData.data.set(frame);
//! offscreenCanvasContext.putImageData(imageData, 0, 0);
//! // mainCanvasContext is the 2D context for the Canvas you actually want to draw to.
//! mainCanvasContext.drawImage(offscreenCanvas, 0, 0);
//! ```

mod utils;

use gc_nes_core::cartridge::Cartridge;
use gc_nes_core::nes::{Nes, NES_SCREEN_DIMENSIONS};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
/// Structure used the represent the NES itself in WASM.
pub struct WebNes {
    nes: Nes,
}

#[wasm_bindgen]
impl WebNes {
    /// Creates a new NES instance with no connected controllers.
    pub fn new(cartridge: WebCartridge) -> WebNes {
        WebNes {
            nes: Nes::new(cartridge.cartridge),
        }
    }

    /// Runs a single cycle on the NES
    pub fn cycle(&mut self) {
        self.nes.cycle();
    }

    /// Runs as many cycles as necessary to complete the current frame.
    /// Returns the frame as a Vector of bytes, with each pixel of the
    /// NES screen represented by four bytes in RGBA order.
    pub fn frame(&mut self) -> Vec<u8> {
        self.nes.frame().to_vec()
    }

    /// Gets the current state of the screen from the PPU's screen buffer.
    /// Returns the frame as a Vector of bytes, with each pixel of the
    /// NES screen represented by four bytes in RGBA order.
    pub fn get_screen(&mut self) -> Vec<u8> {
        self.nes.get_screen().to_vec()
    }

    /// Updates the state of the input device connected to the first port.
    pub fn update_controller_one(&mut self, controller_state: u8) {
        self.nes.update_controller_one(Some(controller_state));
    }

    /// Updates the state of the input device connected to the first port.
    pub fn update_controller_two(&mut self, controller_state: u8) {
        self.nes.update_controller_one(Some(controller_state));
    }

    /// Resets the state of the NES.
    pub fn reset(&mut self) {
        self.nes.reset();
    }
}

#[wasm_bindgen]
/// Structure used to represent a NES Cartridge in WASM.
pub struct WebCartridge {
    cartridge: Cartridge,
}

#[wasm_bindgen]
impl WebCartridge {
    /// Loads a NES ROM from an array of bytes into a WebCartridge struct
    pub fn load(rom: &[u8]) -> WebCartridge {
        WebCartridge {
            cartridge: Cartridge::load_from_reader(rom).unwrap(),
        }
    }
}

#[wasm_bindgen]
/// Creates a new NES instance, loading the passed array of bytes as the ROM
pub fn nes(rom: &[u8]) -> WebNes {
    WebNes::new(WebCartridge::load(rom))
}

#[wasm_bindgen]
/// Gets the screen dimensions of the NES
pub fn get_screen_dimensions() -> usize {
    NES_SCREEN_DIMENSIONS
}

#[wasm_bindgen]
/// Enables the panic hook that logs panic messages to the browser console
pub fn set_panic_hook() {
    utils::set_panic_hook()
}
