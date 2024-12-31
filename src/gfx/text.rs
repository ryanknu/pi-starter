use rusttype::{point, Font, Scale};
use crate::{Colorful, Screen};

/// Can render text. Uses static lifetime for Fonts as that is probably most accurate and simplifies
/// design.
#[derive(Default)]
pub struct TextRenderer {
    font_cache: Vec<(String, Font<'static>)>,
}

impl TextRenderer {
    /// Loads a font with a given name.
    pub fn load_font(&mut self, named: &str, bytes: &'static [u8]) {
        self.font_cache.push((named.to_owned(), Font::try_from_bytes(bytes).unwrap()));
    }

    /// Returns a font with a given name or dies trying.
    fn find_font(&self, named: &str) -> &'static Font {
        for (name, font) in self.font_cache.iter() {
            if name == named { return font; }
        }
        panic!("font not found");
    }

    pub fn render(&'static self, text: &str, font: &str, height: f32, color: &impl Colorful) -> Text {
        let font: &'static Font = self.find_font(font);

        // Render some text
        let font_h_int = height.ceil() as usize;
        let font_scale = Scale::uniform(height);

        // Get the height
        let v_metrics = font.v_metrics(font_scale);
        let offset = point(0.0, v_metrics.ascent);

        // Load glyphs
        let glyphs: Vec<_> = font.layout(text, font_scale, offset).collect();

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
                    let (r, g, b, a) = color.as_rgba();
                    let x = x as i32 + bb.min.x;
                    let y = y as i32 + bb.min.y;
                    // There's still a possibility that the glyph clips the boundaries of the bitmap
                    if x >= 0 && x < width as i32 && y >= 0 && y < font_h_int as i32 {
                        let off = (x as usize * 4) + y as usize * (width * 4);
                        pixel_data[off] = r;
                        pixel_data[off + 1] = g;
                        pixel_data[off + 2] = b;
                        pixel_data[off + 3] = ((a as f32 / u8::MAX as f32) * v * u8::MAX as f32) as u8;
                    }
                })
            }
        }

        Text {
            text: text.to_owned(),
            bitmap: pixel_data,
            width,
        }
    }
}

pub struct BlittableText {
    pub(crate) data: Vec<u8>,
    pub(crate) width: usize,
}

pub struct Text {
    text: String,
    bitmap: Vec<u8>,
    width: usize,
}

impl Text {
    /// Prepares the texture for blitting onto the given screen.
    pub(crate) fn into_blittable(self, screen: &Screen, background: &impl Colorful) -> BlittableText {
        BlittableText {
            data: screen.render_image(&self.bitmap, background),
            width: self.width,
        }
    }
}
