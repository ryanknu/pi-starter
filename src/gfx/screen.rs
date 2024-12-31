use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use itertools::Itertools;
use memmap2::{MmapMut, MmapOptions};
use crate::{BUFFER_SIZE, SCREEN_W};
use crate::gfx::bresenham;
use crate::gfx::color::Colorful;

pub struct Screen {
    map: MmapMut
}

impl Screen {
    pub unsafe fn new(path: PathBuf) -> Result<Self, Box<dyn Error>> {
        // TODO: use map_err() here to get better errors.
        let file = File::options().read(true).write(true).open(path)?;

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
    pub(crate) fn blend_px(&mut self, x: usize, y: usize, color: &impl Colorful) {
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
    pub(crate) fn draw_line(&mut self, x1: usize, y1: usize, x2: usize, y2: usize, color: &impl Colorful) {
        bresenham::draw_line(self, x1 as i32, y1 as i32, x2 as i32, y2 as i32, color);
    }

    /// Draws a rectangle with rounded corners and a border.
    pub(crate) fn draw_rect(&mut self, x: usize, y: usize, w: usize, h: usize, radius: usize, fill: &impl Colorful, border: &impl Colorful) {
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
    pub(crate) fn fill(&mut self, color: &impl Colorful) {
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
    pub(crate) fn render_image(&self, data: &[u8], background: &impl Colorful) -> Vec<u8> {
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
    pub(crate) fn blit_image(&mut self, x: usize, y: usize, w: usize, data: &[u8]) {
        for (idx, &byte) in data.iter().enumerate() {
            self.map[(x * 2) + idx % (w * 2) + idx / (w * 2) * SCREEN_W * 2 + (y * SCREEN_W * 2)] = byte;
        }
    }

    /// More expensive image copy call that blends all the pixels together. Needed for images with
    /// partial transparency that can't be pre-blended with a fixed color.
    pub(crate) fn blend_image(&mut self, x: usize, y: usize, w: usize, data: &[u8]) {
        for (idx, chunk) in data.chunks(4).into_iter().enumerate() {
            let rgba = [chunk[0], chunk[1], chunk[2], chunk[3]];
            self.blend_px(x + idx % w, y + (idx / w), &rgba);
        }
    }
}