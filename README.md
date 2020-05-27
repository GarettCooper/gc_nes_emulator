# gc_nes_emulator
The gc_nes_emulator is Rust implementation of a NES emulator, building off of my previous
[emulator_6502 project](https://github.com/GarettCooper/emulator_6502). It is provided as a Cargo crate ([gc_nes_core](https://github.com/GarettCooper/gc_nes_emulator/tree/master/gc_nes_core)),
an npm package ([gc_nes_web](https://github.com/GarettCooper/gc_nes_emulator/tree/master/gc_nes_web)), and as a standalone executable ([gc_nes_desktop](https://github.com/GarettCooper/gc_nes_emulator/tree/master/gc_nes_desktop)).

### Unsupported Features
At present, the most notable gap in the GC NES Emulator's features is the complete lack of audio. The main reason for this is
that I don't known enough about audio programming to determine how much effort would be required to add audio support. If you
know and are frustrated by my ignorance, feel free to open a Pull Request.
### Supported ROMs
The GC NES Emulator is currently only capable of running a subset of the NES' full game catalogue. This is because each NES 
cartridge could contain custom circuitry known as the Mapper, with each mapper needing to be implemented separately. At 
present, iNES mappers 000 through 003 are fully supported, along with a semi-functional implementation of Mapper 004 
(Super Mario Bros. 3 works perfectly). These five mappers cover just under 2000 of the games in the NES catalogue. If you'd
like to expand the list of supported games, feel free to open a Pull Request with new Mapper implementations.
### Accuracy
The GC NES Emulator is **not** cycle accurate, meaning that memory reads and writes do not occur with the exact same timing they would
have on a real NES. Like in emulator_6502, I opted for a less precise approach to simplify development. The GC NES emulator's bus behaviour
is also not likely to be completely accurate. In practice, these minor details are unlikely to affect the execution of NES ROMs
outside of extreme edge cases present in very few of the games produced for the NES. None of the games that I have personally
tested on the fully supported mappers have encountered any issues.
### Download
Download the latest version of the GC NES Emulator for your platform of choice from [the release page](https://github.com/GarettCooper/gc_nes_emulator/releases).
Alternatively, you can run the Web Assembly version of the emulator right in your browser on [my website, garettcooper.com](https://garettcooper.com/#/nes-emulator).
For the best experience, I recommend downloading it as scaling on the web version is not pixel perfect.

The emulator is a single executable that runs from the command line:

`gc_nes_desktop.exe --scale 4 SomeNesRom.nes`

### Licence
gc_nes_emulator is licensed under the [MIT Licence](https://github.com/GarettCooper/gc_nes_emulator/blob/master/LICENSE).