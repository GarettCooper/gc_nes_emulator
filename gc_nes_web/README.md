[![NPM](https://img.shields.io/crates/v/gc_nes_web)](https://www.npmjs.com/package/gc_nes_web)

# gc_nes_web

gc_nes_web wraps the public functions exposed by my gc_nes_core crate for use
in the browser through Web Assembly - Javascript interop. It provides an interface
to load and run NES ROMs, provide input, and extract rendered image data.
Audio is currently unsupported.

#### Install in an NPM Project
`npm install gc_nes_web`

#### Using the NES Emulator with Javascript
```javascript
// Import the package
const wasm = await import ("gc_nes_web");
// Create the NES object
let nes = this.state.wasm.nes(romArrayOfBytes);
// Run the emulator to the completion of the next frame and retrieve it
let frame = nes.frame();
// Or run just one cycle and get the frame separately
nes.cycle();
let frame = nes.get_screen();
// Drawing to a Canvas
let offscreenCanvas = new OffscreenCanvas(256, 240);
let offscreenCanvasContext = offscreenCanvas.getContext("2d");
let imageData = offscreenCanvasContext?.createImageData(256, 240);
imageData.data.set(frame);
offscreenCanvasContext.putImageData(imageData, 0, 0);
// mainCanvasContext is the 2D context for the Canvas you actually want to draw to.
mainCanvasContext.drawImage(offscreenCanvas, 0, 0);
```

Through wasm-pack, gc_nes_web has full Typescript support

Current version: 0.1.0
