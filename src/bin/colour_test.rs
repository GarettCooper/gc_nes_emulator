use minifb::{Window, Key, WindowOptions};

fn main() {
    let mut window = Window::new("Colour Test", 16 * 16, 16 * 4, WindowOptions::default()).expect("Error opening window");
    let mut buffer: [u32; 0x10 * 0x10 * 0x40] = [0x494949; 0x10 * 0x10 * 0x40];

    for colour in 0..64 {
        for i in 0..16 {
            for j in 0..16 {
                let row = (i * 16 * 16);
                buffer[j + (i * 16 * 16) + (16 * (colour % 16)) + (16 * 16 * 16 * (colour / 16))] = NES_COLOUR_MAP[colour];
            }
        }
    }
    window.update_with_buffer(&buffer);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update()
    }
}

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