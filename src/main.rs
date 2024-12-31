mod gfx;
mod input;

use crate::gfx::color::NamedColor::{Black, Yellow};
use crate::gfx::color::{Colorful, ColorfulCycle, NamedColor, RGBA};
use crate::gfx::screen::Screen;
use gfx::text::TextRenderer;
use image::{load_from_memory, ImageReader};
use itertools::Itertools;
use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use NamedColor::White;
use crate::input::device::InputDevice;
use crate::input::touchscreen::Touchscreen;
// Define some constants for the operation environment.

const SCREEN_W: usize = 800;
const SCREEN_H: usize = 480;
const BUFFER_SIZE: usize = SCREEN_W * SCREEN_H * 3;
const EVENT_SIZE: usize = 16;
const EVENT_BUFFER_LEN: usize = 16;
const EV_SYN: u8 = 0;
const EV_KEY: u8 = 3;
const ABSOLUTE_X_POS: u8 = 0;
const ABSOLUTE_Y_POS: u8 = 1;
const TOUCHES_BEGAN: u8 = 53;
const TOUCHES_ENDED: u8 = 57;

/// The RainbowCycle gradually changes through every maximum brightness color that can be
/// represented. It acts like color changing yarn, where you can't necessarily predict which pixels
/// will be which color, but could work nicely when randomly distributed.
struct RainbowCycleBuilder {
    hue: u8,
}

impl RainbowCycleBuilder {
    fn new() -> Self {
        Self { hue: 0 }
    }

    fn into_cycle(self) -> ColorfulCycle {
        ColorfulCycle { generator: RefCell::new(Box::new(self)) }
    }

    /// Converts HSL value from hue to RGBA.
    /// Source: https://github.com/judge2005/arduinoHSV/blob/master/arduinoHSV.c
    // I tried to figure it out but my brain is fried.
    fn as_rgba(&self) -> RGBA {
        let h = ((self.hue as u16 * 192) / 256) as u8;
        let i = h / 32;
        let f = (h % 32) * 8;

        let s_inv = 0;
        let f_inv = 255 - f;

        let pv = (128 * s_inv as u16 / 256) as u8;
        let qv = (128 * (256 - 255 * f as u16 / 256) / 256) as u8;
        let tv = (128 * (256 - 255 * f_inv as u16 / 256) / 256) as u8;

        match i {
            0 => (u8::MAX, tv, pv, u8::MAX),
            1 => (qv, u8::MAX, pv, u8::MAX),
            2 => (pv, u8::MAX, tv, u8::MAX),
            3 => (pv, qv, u8::MAX, u8::MAX),
            4 => (tv, pv, u8::MAX, u8::MAX),
            _ => (u8::MAX, pv, qv, u8::MAX),
        }
    }
}

impl Iterator for RainbowCycleBuilder {
    type Item = RGBA;

    fn next(&mut self) -> Option<Self::Item> {
        self.hue = self.hue.wrapping_add(1);
        Some(self.as_rgba())
    }
}

/// Loads an image from provided image data (such as from `include_bytes!()`). This uses the image
/// crates "guess format" method so if it doesn't work for your input, just modify this function.
fn decode_image(data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let image = load_from_memory(data)?;
    let rgba = image.to_rgba8();
    Ok(rgba.to_vec())
}

/// Loads an image from disk into memory.
fn load_image(path: PathBuf) -> Result<Vec<u8>, Box<dyn Error>> {
    let image = ImageReader::open(path)?.decode()?;
    let rgba = image.as_rgba8().unwrap();
    Ok(rgba.to_vec())
}

/// The cursor blink causes part of the screen to be redrawn. Reciting this particular incantation
/// seems to work.
/// TODO: Update this to work with *nix term package.
fn hide_cursor() {
    println!("\x1b[?25l");
}

/// Finds the touchscreen hidden amongst all input devices. Returns a path to it.
fn find_touchscreen() -> Option<PathBuf> {
    // TODO
    // reading from /proc/bus/input/devices *could* work.
    // H: Handlers=sysrq kbd leds event0
    Some(PathBuf::from("/dev/input/event3"))
}

// TODO: Touchscreen if - delta
//       Touchscreen if - click pos
//       Gfx - Render text
//       Gfx - blend alpha channel
// Stretch goals:
// - Detect screen resolution - requires ioctl
// - Set pixel depth to 24 bits - requires ioctl
// - Webcam interface (?)
// - Push button interface.
fn main() {
    // If any cursor is blinking, turn that off.
    hide_cursor();
    // Locate the touchscreen device
    let touchscreen_handle = find_touchscreen().unwrap();
    // Open the touchscreen device.
    let mut touchscreen: InputDevice<Touchscreen> = InputDevice::new(File::open(touchscreen_handle).unwrap());
    // Open the screen device. Unsafe because we need unrestricted write to a region of memory.
    let mut screen = unsafe { Screen::new("/dev/fb0".parse().unwrap()).unwrap() };

    // Set up rainbow color generator
    let rainbow = RainbowCycleBuilder::new().into_cycle();

    // Create a text renderer. You need to leak it to ensure that the bytes behind the fonts never
    // deallocate. Trust me, this actually is easier.
    let mut text_renderer = Box::leak(Box::new(TextRenderer::default()));

    // Load font - OpenSans Condensed Light can display *a lot* of text on the pi touchscreen.
    text_renderer.load_font("OpenSans-CondLight", include_bytes!("OpenSans-CondLight.ttf"));

    // Render some text into RGBA
    let hello_text = text_renderer.render("This is my Raspberry Pi Touchscreen project", "OpenSans-CondLight", 18.0, &rainbow);

    // Load a bundled image asset
    let close_icon = &decode_image(include_bytes!("close.png")).unwrap();
    let sushi = screen.render_image(&decode_image(include_bytes!("sushi.png")).unwrap(), &Black);
    // Load the sample text
    let text = hello_text.into_blittable(&screen, &Yellow);

    screen.fill(&Black);
    screen.blit_image(0, 0, 750, &sushi);
    screen.blend_image(0, 0, 50, &close_icon);
    screen.draw_rect(SCREEN_W - text.width - 30 - 2, 18, text.width + 2, 18 + 4, 4, &Yellow, &Yellow);
    screen.blit_image(SCREEN_W - text.width - 30, 20, text.width, &text.data);

    // Loop through values for corner radius
    for i in 0..9 {
        screen.draw_rect(75 + i * 50, 10, 30, 30, i, &[255, 255, 255, 255 / (i as u8 + 1)], &White);
    }

    let mut last_pos: Option<(usize, usize)> = None;
    let mut run = true;
    while run {
        if touchscreen.poll().unwrap() {
            for point in touchscreen.trail().into_iter().rev() {
                if let Some(last_pos) = last_pos {
                    screen.draw_line(point.0, point.1, last_pos.0, last_pos.1, &rainbow);
                    // screen.draw_line(last_pos.0, last_pos.1 + 1, point.0, point.1 - 1, &rainbow);
                }

                last_pos = Some(point);

                // Detect corner kill
                if point.0 < 50 && point.1 < 50 {
                    run = false;
                }
            }

            if touchscreen.touches_ended() {
                last_pos = None;
            }
        }

        sleep(Duration::from_millis(16));
    }
}
