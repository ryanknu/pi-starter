use std::cell::RefCell;

/// Ergonomic rename of 4-tuple of bytes.
pub type RGBA = (u8, u8, u8, u8);

/// Trait which can be applied to anything that can represent a color.
pub trait Colorful {
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

pub enum NamedColor {
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

pub struct ColorfulCycle {
    pub generator: RefCell<Box<dyn Iterator<Item=RGBA>>>
}

impl Colorful for ColorfulCycle {
    fn as_rgba(&self) -> RGBA {
        self.generator.borrow_mut().next().unwrap()
    }
}
