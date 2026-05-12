use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
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
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    #[inline]
    pub const fn to_rgba(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    #[inline]
    pub const fn from_rgba(v: [f32; 4]) -> Self {
        Self::new(v[0], v[1], v[2], v[3])
    }

    #[inline]
    pub const fn to_rgb(self) -> [f32; 3] {
        [self.r, self.g, self.b]
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let raw = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
        match raw.len() {
            3 => {
                let r = nibble(raw.as_bytes()[0])?;
                let g = nibble(raw.as_bytes()[1])?;
                let b = nibble(raw.as_bytes()[2])?;
                Some(Self::new(
                    ((r << 4) | r) as f32 / 255.0,
                    ((g << 4) | g) as f32 / 255.0,
                    ((b << 4) | b) as f32 / 255.0,
                    1.0,
                ))
            }
            4 => {
                let r = nibble(raw.as_bytes()[0])?;
                let g = nibble(raw.as_bytes()[1])?;
                let b = nibble(raw.as_bytes()[2])?;
                let a = nibble(raw.as_bytes()[3])?;
                Some(Self::new(
                    ((r << 4) | r) as f32 / 255.0,
                    ((g << 4) | g) as f32 / 255.0,
                    ((b << 4) | b) as f32 / 255.0,
                    ((a << 4) | a) as f32 / 255.0,
                ))
            }
            6 => Some(Self::new(
                byte(&raw[0..2])? as f32 / 255.0,
                byte(&raw[2..4])? as f32 / 255.0,
                byte(&raw[4..6])? as f32 / 255.0,
                1.0,
            )),
            8 => Some(Self::new(
                byte(&raw[0..2])? as f32 / 255.0,
                byte(&raw[2..4])? as f32 / 255.0,
                byte(&raw[4..6])? as f32 / 255.0,
                byte(&raw[6..8])? as f32 / 255.0,
            )),
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
    pub fn to_rgba_u8(self) -> [u8; 4] {
        [
            quantize(self.r),
            quantize(self.g),
            quantize(self.b),
            quantize(self.a),
        ]
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Color({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

#[inline]
fn quantize(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
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
