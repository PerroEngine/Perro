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
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);
    pub const YELLOW: Self = Self::new(1.0, 1.0, 0.0, 1.0);
    pub const CYAN: Self = Self::new(0.0, 1.0, 1.0, 1.0);
    pub const MAGENTA: Self = Self::new(1.0, 0.0, 1.0, 1.0);
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
