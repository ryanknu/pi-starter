mod bresenham;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use image::{load_from_memory, DynamicImage, ImageReader};
use itertools::Itertools;
use memmap2::{MmapMut, MmapOptions};
use rusttype::{point, Font, Scale};

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

struct Screen {
    map: MmapMut
}

impl Screen {
    unsafe fn new() -> Result<Self, Box<dyn Error>> {
        // TODO: use map_err() here to get better errors.
        let file = File::options().read(true).write(true).open("/dev/fb0")?;

        let map = MmapOptions::new()
            .offset(0)
            .len(BUFFER_SIZE)
            .map_mut(&file)?;

        Ok(Self { map })
    }

    /// Packs a 24-bit color (3 8-bit channels) into a 16-bit color.
    #[inline]
    fn color_to_16_bits(r: u8, g: u8, b: u8) -> (u8, u8) {
        let h = (g & 0b0001_1100) << 3;
        let h = h | ((b & 0b1111_1000) >> 3);
        let l = r & 0b1111_1000;
        let l = l | ((g & 0b1110_0000) >> 5);

        (h, l)
    }

    /// Blends a color with alpha channel with an opaque color.
    fn blend(r: u8, g: u8, b: u8, a: u8, cr: u8, cg: u8, cb: u8) -> (u8, u8, u8) {
        let nr = (((a as u16 * r as u16) + ((255 - a as u16) * cr as u16)) / 256) as u8;
        let ng = (((a as u16 * g as u16) + ((255 - a as u16) * cg as u16)) / 256) as u8;
        let nb = (((a as u16 * b as u16) + ((255 - a as u16) * cb as u16)) / 256) as u8;
        (nr, ng, nb)
    }

    /// Retrieves the buffer coordinate of the given X and Y coordinate.
    #[inline]
    fn buffer_offset(&self, x: usize, y: usize) -> usize {
        (SCREEN_W * 2 * y) + x * 2
    }

    /// Sets a specified pixel to a color.
    #[inline]
    fn set_px(&mut self, x: usize, y: usize, color: &impl Colorful) {
        let (r, g, b, _a) = color.as_rgba();
        let (h, l) = Self::color_to_16_bits(r, g, b);
        let b_off = self.buffer_offset(x, y);

        self.map[b_off] = h;
        self.map[b_off + 1] = l;
    }

    /// Updates a specified pixel's color by blending it with its new color.
    /// https://en.wikipedia.org/wiki/Alpha_compositing#Alpha_blending
    fn blend_px(&mut self, x: usize, y: usize, color: &impl Colorful) {
        // alpha * new color + (1 - alpha) * prev color
        let (r, g, b, a) = color.as_rgba();

        // Short-cut if pixel is fully opaque. Hot path in images.
        if a == 255 {
            self.set_px(x, y, color);
            return;
        }

        // Short-cut if pixel is fully transparent.
        if a == 0 {
            return;
        }

        // Retrieve the current the color
        let b_off = self.buffer_offset(x, y);
        let (ch, cl) = (self.map[b_off], self.map[b_off + 1]);

        let cr = cl & 0b1111_1000;
        let cg = ((cl & 0b0000_0111) << 5) | ((ch & 0b1110_0000) >> 3);
        let cb = (ch & 0b0001_1111) << 3;

        let (nr, ng, nb) = Self::blend(r, g, b, a, cr, cg, cb);

        self.set_px(x, y, &[nr, ng, nb]);
    }

    /// Draws a line (kinda) from (x1, y1) to (x2, y2).
    fn draw_line(&mut self, x1: usize, y1: usize, x2: usize, y2: usize, color: &impl Colorful) {
        bresenham::draw_line(self, x1 as i32, y1 as i32, x2 as i32, y2 as i32, color);
    }

    /// Draws a rectangle with rounded corners and a border.
    fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, radius: usize, fill: &impl Colorful, border: &impl Colorful) {
        const MASK_SIZE: usize = 8;
        const CORNER_MASK: [u8; MASK_SIZE * (MASK_SIZE + 1)] = [
            0, 0, 0, 0, 0, 0, 0, 0,
            1, 0, 0, 0, 0, 0, 0, 0,
            2, 1, 0, 0, 0, 0, 0, 0,
            3, 2, 1, 0, 0, 0, 0, 0,
            4, 2, 1, 1, 0, 0, 0, 0,
            5, 3, 2, 1, 1, 0, 0, 0,
            6, 4, 2, 2, 1, 1, 0, 0,
            7, 5, 4, 3, 2, 1, 1, 0,
            8, 6, 4, 3, 2, 2, 1, 1,
        ];

        // If rectangle has no volume, do not do anything.
        if x < 2 || y < 2 {
            return;
        }

        // If there is not enough room to fully render the corner radius, lower the radius to a safe value.
        let radius = if h < radius * 2 || w < radius * 2 {
            h.min(w) / 3
        } else {
            radius
        };

        // TODO: don't let j > SCREEN_H
        //`mask` is an 8-element array
        let mask = &CORNER_MASK[radius.min(MASK_SIZE) * MASK_SIZE .. radius.min(MASK_SIZE) * MASK_SIZE + MASK_SIZE];
        for j in 0..h {
            let sx_masked = x + mask[j.min(MASK_SIZE - 1)] as usize + mask[(h - j - 1).min(MASK_SIZE - 1)] as usize;
            let ex_masked = (x + w).min(SCREEN_W) - mask[j.min(MASK_SIZE - 1)] as usize - mask[(h - j - 1).min(MASK_SIZE - 1)] as usize;
            self.draw_line(sx_masked, y + j, ex_masked, y + j, fill);

            // Draw border
            self.blend_px(sx_masked, y + j, border);
            self.blend_px(ex_masked, y + j, border);
            if j == 0 || j == h - 1 {
                self.draw_line(sx_masked, y + j, ex_masked, y + j, border);
            }
        }
    }

    /// Fills the entire framebuffer with a single color.
    fn fill(&mut self, color: &impl Colorful) {
        let (r, g, b, _a) = color.as_rgba();
        // I am in 16 bit mode, so, I need to use 5 bits per pixel (I guess). I would prefer to set
        // 24-bit color mode.
        let (h, l) = Self::color_to_16_bits(r, g, b);

        for i in 0..BUFFER_SIZE {
            self.map[i] = if i % 2 == 0 { h } else { l };
        }
    }

    /// Copies the provided image data in `[r, g, b, a, r, g, b, a, ...]` format to the screen's
    /// current color space, for use with `blit`. The `image` crate's `DynamicImage::as_rgba8()`
    /// function provides the correct format for this.
    fn render(&self, data: &[u8], background: &impl Colorful) -> Vec<u8> {
        let (br, bg, bb, _) = background.as_rgba();
        let (hi, lo): (Vec<_>, Vec<_>) = data.chunks(4)
            .into_iter()
            .map(|n| Self::blend(n[0], n[1], n[2], n[3], br, bg, bb))
            .map(|(r, g, b)| Self::color_to_16_bits(r, g, b))
            .unzip();

        hi.into_iter().interleave(lo.into_iter()).collect()
    }

    /// Draws the provided texture to the screen at the given coordinate and width. Blitting
    /// pre-rendered text is the preferred way to display text. `data` is expected to be in the
    /// correct format for the buffer. Use `render` to prepare images for this.
    fn blit(&mut self, x: usize, y: usize, w: usize, data: &[u8]) {
        for (idx, &byte) in data.iter().enumerate() {
            self.map[(x * 2) + idx % (w * 2) + idx / (w * 2) * SCREEN_W * 2 + (y * SCREEN_W * 2)] = byte;
        }
    }
}

/// Ergonomic rename of 4-tuple of bytes.
type RGBA = (u8, u8, u8, u8);

/// Trait which can be applied to anything that can represent a color.
trait Colorful {
    /// Returns the struct as a 24-bit color with alpha channel.
    fn as_rgba(&self) -> RGBA;
}

impl Colorful for [u8; 3] {
    fn as_rgba(&self) -> RGBA {
        (self[0], self[1], self[2], u8::MAX)
    }
}

impl Colorful for [u8; 4] {
    fn as_rgba(&self) -> RGBA {
        (self[0], self[1], self[2], self[3])
    }
}

enum NamedColor {
    Red,
    Green,
    Blue,
    Black,
    White,
    Yellow,
    Cyan,
}

impl Colorful for NamedColor {
    fn as_rgba(&self) -> RGBA {
        match self {
            NamedColor::Red   => (u8::MAX, 0, 0, u8::MAX),
            NamedColor::Green => (0, u8::MAX, 0, u8::MAX),
            NamedColor::Blue  => (0, 0, u8::MAX, u8::MAX),
            NamedColor::Black => (0, 0, 0, u8::MAX),
            NamedColor::White => (u8::MAX, u8::MAX, u8::MAX, u8::MAX),
            NamedColor::Cyan => (0, u8::MAX, u8::MAX, u8::MAX),
            NamedColor::Yellow => (u8::MAX, u8::MAX, 0, u8::MAX),
        }
    }
}

struct ColorfulCycle {
    generator: RefCell<Box<dyn Iterator<Item=RGBA>>>
}

impl Colorful for ColorfulCycle {
    fn as_rgba(&self) -> RGBA {
        self.generator.borrow_mut().next().unwrap()
    }
}

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

/// Represents a raw input event from the Linux evdev system.
struct InputEvent {
    r#type: u8,
    code: u8,
    value: i32,
}

/// Represents a generic input device
struct InputDevice<T: ReadInputStream> {
    /// Handle to file for the input device.
    file: File,
    /// Buffer for reading input data. Event structure is 8 bytes timestamp, 2 bytes type, 2 bytes
    /// code, and 4 bytes for value. 16 bytes per event with a 16 byte buffer.
    data: [u8; EVENT_SIZE * EVENT_BUFFER_LEN],
    /// ..
    device: T,
}

impl<T> InputDevice<T> where T: ReadInputStream + Default {
    /// Creates an Input device with the default implementation of the InputDevice.
    fn new(file: File) -> Self {
        Self {
            file,
            data: [0; EVENT_SIZE * EVENT_BUFFER_LEN],
            device: T::default(),
        }
    }

    /// Polls the input device for any new data. Returns true, false, or an error. Bool return types
    /// indicate whether or not there's anything to process on the user's end. An error return type
    /// indicates that the user should terminate the process, or gracefully handle the error.
    pub fn poll(&mut self) -> Result<bool, Box<dyn Error>> {
        // Read up to N events into buffer.
        let bytes_read = self.file.read(&mut self.data)?;
        if bytes_read == 0 {
            return Ok(false);
        }

        // Take a reference to the slice that only contains data read.
        let events = &self.data[..bytes_read];

        // Turn a simple array of bytes into an iterator over well-formed events.
        // TODO: Check to make sure that creating InputEvent structs from this is not slow.
        let events = events.chunks(EVENT_SIZE).map(|raw_event| InputEvent {
            r#type: raw_event[8],
            code: raw_event[10],
            value: i32::from_le_bytes([raw_event[12], raw_event[13], raw_event[14], raw_event[15]]),
        });

        // Pass to the device abstraction and return the result.
        Ok(self.device.read_events(events)?)
    }
}

/// Implementing Deref allows us to use the inner device as if it's fields are part of the base
/// struct.
impl<T: ReadInputStream> Deref for InputDevice<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

/// See: `deref`
impl<T: ReadInputStream> DerefMut for InputDevice<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}

trait ReadInputStream {
    fn read_events(&mut self, stream: impl Iterator<Item = InputEvent>) -> Result<bool, Box<dyn Error>>;
}

/// Represents a touchscreen interface.
#[derive(Default)]
struct Touchscreen {
    /// A ring buffer for the current touch's points. Acts as a stack where the first position is
    /// the most recently touched. Using a `VecDeque` lets us iterate freely in both directions and
    /// is space efficient.
    trail: VecDeque<(usize, usize)>,
    /// Since each event only comes with one part of the coordinate pair, we need to buffer one
    /// until the other is received. When both values are `Some`, we push to the trail and reset
    /// these values to None.
    next_x: Option<usize>,
    /// See: `next_x`.
    next_y: Option<usize>,
    /// The delta holds the coordinate vector difference from the origin of the last touch trail.
    /// It should technically be the sum of the entire `trail`. This field in particular is useful
    /// for implementing touch-and-drag interfaces where the element is offset by the touch offset.
    delta: (usize, usize),
    /// Indicates to the user that the user has lifted their finger, and that they should stop
    /// dragging, process a tap, or stop connecting lines.
    touches_ended: bool,
}

impl ReadInputStream for Touchscreen {
    fn read_events(&mut self, stream: impl Iterator<Item = InputEvent>) -> Result<bool, Box<dyn Error>> {
        for event in stream {
            match (event.r#type, event.code) {
                (EV_SYN, _,) => {
                    if let (Some(x), Some(y)) = (self.next_x, self.next_y) {
                        self.trail.push_front((x, y));
                        self.next_x = None;
                        self.next_y = None;
                    }
                }
                (EV_KEY, ABSOLUTE_X_POS) => self.next_x = Some(event.value as usize),
                (EV_KEY, ABSOLUTE_Y_POS) => self.next_y = Some(event.value as usize),
                (EV_KEY, TOUCHES_BEGAN) => self.touches_ended = false,
                (EV_KEY, TOUCHES_ENDED) => self.touches_ended = true,
                _ => {}
            }
        }

        // We assume if this was called, we can signal the user to process events. However, we
        // really shouldn't if every event in `stream` fell into the default match case.
        Ok(true)
    }
}

/// Represents a single-touch touchscreen device.
impl Touchscreen {
    // TODO: fn click(&self) -> Option(usize, usize) : returns Some(pt) if the user clicked there.
    // TODO: Update `trail` to return a reference to the VecDeque. Returning an owned Vec is definitely slow :D
    fn trail(&mut self) -> Vec<(usize, usize)> {
        let res = self.trail.iter().cloned().collect();
        self.trail.truncate(0);
        res
    }

    /// Returns whether or not the user has lifted their finger. If `poll` returns true on a this
    /// device you can safely assume that touches have begun.
    fn touches_ended(&self) -> bool {
        self.touches_ended
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
    let mut screen = unsafe { Screen::new().unwrap() };

    // Load font
    let font = Font::try_from_bytes(include_bytes!("OpenSans-CondLight.ttf")).unwrap();

    // Render some text
    let font_h = 16.0f32;
    let font_h_int = font_h.ceil() as usize;
    let font_scale = Scale {
        x: font_h * 2.0,
        y: font_h,
    };

    // Get the height
    let v_metrics = font.v_metrics(font_scale);
    let offset = point(0.0, v_metrics.ascent);

    // Load glyphs
    let glyphs: Vec<_> = font.layout("Hello, World!", font_scale, offset).collect();

    // Get the width
    let width = glyphs
        .iter()
        .rev()
        .map(|g| g.position().x + g.unpositioned().h_metrics().advance_width)
        .next()
        .unwrap_or(0.0)
        .ceil() as usize;

    // Draw the text into a texture.
    let mut pixel_data = vec![0u8; width * font_h_int * 4];
    for g in glyphs {
        if let Some(bb) = g.pixel_bounding_box() {
            g.draw(|x, y, v| {
                let x = x as i32 + bb.min.x;
                let y = y as i32 + bb.min.y;
                // There's still a possibility that the glyph clips the boundaries of the bitmap
                if x >= 0 && x < width as i32 && y >= 0 && y < font_h_int as i32 {
                    let off = (x as usize * 4) + y as usize * (width * 4);
                    pixel_data[off] = 255 as u8;
                    pixel_data[off + 1] = 0 as u8;
                    pixel_data[off + 2] = 0 as u8;
                    pixel_data[off + 3] = (v * 255.0) as u8;
                }
            })
        }
    }

    // Load a bundled image asset
    let close_icon = screen.render(&decode_image(include_bytes!("close.png")).unwrap(), &NamedColor::Black);
    let sushi = screen.render(&decode_image(include_bytes!("sushi.png")).unwrap(), &NamedColor::Black);
    // Load the sample text
    let text = screen.render(&pixel_data, &NamedColor::Yellow);

    screen.fill(&NamedColor::Black);
    screen.blit(0, 0, 750, &sushi);
    screen.blit(0, 0, 50, &close_icon);
    screen.draw_rect(SCREEN_W - width - 30 - 2, 18, width + 2, font_h_int + 4, 4, &NamedColor::Yellow, &NamedColor::Yellow);
    screen.blit(SCREEN_W - width - 30, 20, width, &text);

    // Loop through values for corner radius
    for i in 0..9 {
        screen.draw_rect(75 + i * 50, 10, 30, 30, i, &[255, 255, 255, 255 / (i as u8 + 1)], &NamedColor::White);
    }

    let mut last_pos: Option<(usize, usize)> = None;
    let mut run = true;
    let rainbow = RainbowCycleBuilder::new().into_cycle();
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
