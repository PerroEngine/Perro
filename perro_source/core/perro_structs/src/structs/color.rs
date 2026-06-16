use super::{Unit, UnitVector4};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: Unit,
    pub g: Unit,
    pub b: Unit,
    pub a: Unit,
}

impl Color {
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    pub const GRAY: Self = Self::new(0.5, 0.5, 0.5, 1.0);
    pub const GREY: Self = Self::GRAY;
    pub const LIGHT_GRAY: Self = Self::new(0.75, 0.75, 0.75, 1.0);
    pub const LIGHT_GREY: Self = Self::LIGHT_GRAY;
    pub const DARK_GRAY: Self = Self::new(0.25, 0.25, 0.25, 1.0);
    pub const DARK_GREY: Self = Self::DARK_GRAY;
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const MAROON: Self = Self::new(0.5, 0.0, 0.0, 1.0);
    pub const CRIMSON: Self = Self::new(0.86, 0.08, 0.24, 1.0);
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const LIME: Self = Self::GREEN;
    pub const FOREST_GREEN: Self = Self::new(0.13, 0.55, 0.13, 1.0);
    pub const OLIVE: Self = Self::new(0.5, 0.5, 0.0, 1.0);
    pub const MINT: Self = Self::new(0.6, 1.0, 0.6, 1.0);
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);
    pub const NAVY: Self = Self::new(0.0, 0.0, 0.5, 1.0);
    pub const ROYAL_BLUE: Self = Self::new(0.25, 0.41, 0.88, 1.0);
    pub const SKY_BLUE: Self = Self::new(0.53, 0.81, 0.92, 1.0);
    pub const CORNFLOWER_BLUE: Self = Self::new(0.39, 0.58, 0.93, 1.0);
    pub const ORANGE: Self = Self::new(1.0, 0.5, 0.0, 1.0);
    pub const YELLOW: Self = Self::new(1.0, 1.0, 0.0, 1.0);
    pub const INDIGO: Self = Self::new(0.29, 0.0, 0.51, 1.0);
    pub const VIOLET: Self = Self::new(0.56, 0.0, 1.0, 1.0);
    pub const CYAN: Self = Self::new(0.0, 1.0, 1.0, 1.0);
    pub const TEAL: Self = Self::new(0.0, 0.5, 0.5, 1.0);
    pub const TURQUOISE: Self = Self::new(0.25, 0.88, 0.82, 1.0);
    pub const MAGENTA: Self = Self::new(1.0, 0.0, 1.0, 1.0);
    pub const PINK: Self = Self::new(1.0, 0.75, 0.8, 1.0);
    pub const PURPLE: Self = Self::new(0.5, 0.0, 0.5, 1.0);
    pub const BROWN: Self = Self::new(0.59, 0.29, 0.0, 1.0);
    pub const GOLD: Self = Self::new(1.0, 0.84, 0.0, 1.0);
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    #[inline]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: Unit::new(r),
            g: Unit::new(g),
            b: Unit::new(b),
            a: Unit::new(a),
        }
    }

    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    #[inline]
    pub const fn from_rgba_u8(v: [u8; 4]) -> Self {
        Self {
            r: Unit::from_u8(v[0]),
            g: Unit::from_u8(v[1]),
            b: Unit::from_u8(v[2]),
            a: Unit::from_u8(v[3]),
        }
    }

    #[inline]
    pub const fn from_unit_vector4(v: UnitVector4) -> Self {
        Self::from_rgba_u8(v.to_u8())
    }

    #[inline]
    pub const fn from_unit_slice(v: UnitVector4) -> Self {
        Self::from_unit_vector4(v)
    }

    #[inline]
    pub const fn r(self) -> f32 {
        self.r.to_f32()
    }

    #[inline]
    pub const fn g(self) -> f32 {
        self.g.to_f32()
    }

    #[inline]
    pub const fn b(self) -> f32 {
        self.b.to_f32()
    }

    #[inline]
    pub const fn a(self) -> f32 {
        self.a.to_f32()
    }

    #[inline]
    pub const fn to_rgba(self) -> [f32; 4] {
        [self.r(), self.g(), self.b(), self.a()]
    }

    #[inline(always)]
    pub const fn to_float_slice(self) -> [f32; 4] {
        self.to_rgba()
    }

    #[inline]
    pub const fn from_rgba(v: [f32; 4]) -> Self {
        Self::new(v[0], v[1], v[2], v[3])
    }

    #[inline(always)]
    pub const fn from_float_slice(v: [f32; 4]) -> Self {
        Self::from_rgba(v)
    }

    #[inline]
    pub const fn to_rgb(self) -> [f32; 3] {
        [self.r(), self.g(), self.b()]
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let raw = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
        match raw.len() {
            3 => {
                let r = nibble(raw.as_bytes()[0])?;
                let g = nibble(raw.as_bytes()[1])?;
                let b = nibble(raw.as_bytes()[2])?;
                Some(Self::from_rgba_u8([
                    (r << 4) | r,
                    (g << 4) | g,
                    (b << 4) | b,
                    255,
                ]))
            }
            4 => {
                let r = nibble(raw.as_bytes()[0])?;
                let g = nibble(raw.as_bytes()[1])?;
                let b = nibble(raw.as_bytes()[2])?;
                let a = nibble(raw.as_bytes()[3])?;
                Some(Self::from_rgba_u8([
                    (r << 4) | r,
                    (g << 4) | g,
                    (b << 4) | b,
                    (a << 4) | a,
                ]))
            }
            6 => Some(Self::from_rgba_u8([
                byte(&raw[0..2])?,
                byte(&raw[2..4])?,
                byte(&raw[4..6])?,
                255,
            ])),
            8 => Some(Self::from_rgba_u8([
                byte(&raw[0..2])?,
                byte(&raw[2..4])?,
                byte(&raw[4..6])?,
                byte(&raw[6..8])?,
            ])),
            _ => None,
        }
    }

    pub fn to_hex_rgb(self) -> String {
        let [r, g, b, _] = self.to_rgba_u8();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    pub fn to_hex_rgba(self) -> String {
        let [r, g, b, a] = self.to_rgba_u8();
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    #[inline]
    pub const fn to_unit_vector4(self) -> UnitVector4 {
        UnitVector4::from_u8(self.to_rgba_u8())
    }

    #[inline]
    pub const fn to_unit_slice(self) -> UnitVector4 {
        self.to_unit_vector4()
    }

    #[inline]
    pub const fn to_rgba_u8(self) -> [u8; 4] {
        [
            self.r.to_u8(),
            self.g.to_u8(),
            self.b.to_u8(),
            self.a.to_u8(),
        ]
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Color({}, {}, {}, {})",
            self.r(),
            self.g(),
            self.b(),
            self.a()
        )
    }
}

impl From<Color> for [f32; 4] {
    #[inline(always)]
    fn from(value: Color) -> Self {
        value.to_float_slice()
    }
}

impl From<[f32; 4]> for Color {
    #[inline(always)]
    fn from(value: [f32; 4]) -> Self {
        Self::from_float_slice(value)
    }
}

impl From<UnitVector4> for Color {
    #[inline(always)]
    fn from(value: UnitVector4) -> Self {
        Self::from_unit_vector4(value)
    }
}

impl From<Color> for UnitVector4 {
    #[inline(always)]
    fn from(value: Color) -> Self {
        value.to_unit_vector4()
    }
}

#[inline]
fn byte(hex: &str) -> Option<u8> {
    u8::from_str_radix(hex, 16).ok()
}

#[inline]
fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_stores_unorm8_channels() {
        let color = Color::new(1.0, 0.5, -1.0, 2.0);

        assert_eq!(color.to_rgba_u8(), [255, 128, 0, 255]);
        assert_eq!(color.r.to_u8(), 255);
        assert_eq!(color.g.to_u8(), 128);
        assert_eq!(color.b.to_u8(), 0);
        assert_eq!(color.a.to_u8(), 255);
    }

    #[test]
    fn color_from_hex_keeps_exact_bytes() {
        let color = Color::from_hex("#336699CC").unwrap();

        assert_eq!(color.to_rgba_u8(), [0x33, 0x66, 0x99, 0xCC]);
        assert_eq!(color.to_hex_rgba(), "#336699CC");
    }

    #[test]
    fn color_converts_from_unit_vector4() {
        let packed = UnitVector4::from_u8([0x33, 0x66, 0x99, 0xCC]);
        let color = Color::from_unit_vector4(packed);

        assert_eq!(color.to_rgba_u8(), [0x33, 0x66, 0x99, 0xCC]);
        assert_eq!(Color::from_unit_slice(packed), color);
        assert_eq!(UnitVector4::from(color).to_u8(), packed.to_u8());
    }
}
