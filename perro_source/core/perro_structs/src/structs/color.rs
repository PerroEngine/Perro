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

    /// Returns a copy with alpha overridden. `const` and allocation-free.
    #[inline]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: Unit::new(a),
        }
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

    /// `const` hex parser for the [`color!`] macro. Accepts `#RGB`, `#RGBA`,
    /// `#RRGGBB`, `#RRGGBBAA`, or the same without the leading `#`.
    ///
    /// Returns `None` on malformed input so the macro can panic at compile time.
    pub const fn try_from_hex_const(hex: &str) -> Option<Self> {
        let bytes = hex.as_bytes();
        let start = if !bytes.is_empty() && bytes[0] == b'#' {
            1
        } else {
            0
        };
        let len = bytes.len() - start;
        match len {
            3 => match (
                nibble_const(bytes[start]),
                nibble_const(bytes[start + 1]),
                nibble_const(bytes[start + 2]),
            ) {
                (Some(r), Some(g), Some(b)) => Some(Self::from_rgba_u8([
                    (r << 4) | r,
                    (g << 4) | g,
                    (b << 4) | b,
                    255,
                ])),
                _ => None,
            },
            4 => match (
                nibble_const(bytes[start]),
                nibble_const(bytes[start + 1]),
                nibble_const(bytes[start + 2]),
                nibble_const(bytes[start + 3]),
            ) {
                (Some(r), Some(g), Some(b), Some(a)) => Some(Self::from_rgba_u8([
                    (r << 4) | r,
                    (g << 4) | g,
                    (b << 4) | b,
                    (a << 4) | a,
                ])),
                _ => None,
            },
            6 => match (
                byte_const(bytes[start], bytes[start + 1]),
                byte_const(bytes[start + 2], bytes[start + 3]),
                byte_const(bytes[start + 4], bytes[start + 5]),
            ) {
                (Some(r), Some(g), Some(b)) => Some(Self::from_rgba_u8([r, g, b, 255])),
                _ => None,
            },
            8 => match (
                byte_const(bytes[start], bytes[start + 1]),
                byte_const(bytes[start + 2], bytes[start + 3]),
                byte_const(bytes[start + 4], bytes[start + 5]),
                byte_const(bytes[start + 6], bytes[start + 7]),
            ) {
                (Some(r), Some(g), Some(b), Some(a)) => Some(Self::from_rgba_u8([r, g, b, a])),
                _ => None,
            },
            _ => None,
        }
    }

    /// `const` hex parser that panics on malformed input. Used by [`color!`].
    pub const fn from_hex_const(hex: &str) -> Self {
        match Self::try_from_hex_const(hex) {
            Some(color) => color,
            None => panic!("color! expects `#RGB`, `#RGBA`, `#RRGGBB`, or `#RRGGBBAA`"),
        }
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
            6 => {
                let bytes = raw.as_bytes();
                Some(Self::from_rgba_u8([
                    byte_const(bytes[0], bytes[1])?,
                    byte_const(bytes[2], bytes[3])?,
                    byte_const(bytes[4], bytes[5])?,
                    255,
                ]))
            }
            8 => {
                let bytes = raw.as_bytes();
                Some(Self::from_rgba_u8([
                    byte_const(bytes[0], bytes[1])?,
                    byte_const(bytes[2], bytes[3])?,
                    byte_const(bytes[4], bytes[5])?,
                    byte_const(bytes[6], bytes[7])?,
                ]))
            }
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

/// Builds a compile-time-validated [`Color`] from a hex literal.
///
/// Signature:
/// - `color!(&str) -> Color`
///
/// Usage:
/// - `const PANEL_BG: Color = color!("#0B1018");`
/// - `let tint = color!("#88AADDFF");`
///
/// Malformed literals fail at compile time. Accepts `#RGB`, `#RGBA`,
/// `#RRGGBB`, `#RRGGBBAA`, or the same forms without the leading `#`.
#[macro_export]
macro_rules! color {
    ($hex:expr) => {
        $crate::Color::from_hex_const($hex)
    };
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
fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[inline]
const fn nibble_const(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[inline]
const fn byte_const(hi: u8, lo: u8) -> Option<u8> {
    match (nibble_const(hi), nibble_const(lo)) {
        (Some(h), Some(l)) => Some((h << 4) | l),
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
    fn color_from_hex_rejects_multibyte_text_without_panicking() {
        assert_eq!(Color::from_hex("1é234"), None);
        assert_eq!(Color::from_hex("1é23456"), None);
    }

    #[test]
    fn with_alpha_overrides_only_alpha() {
        let base = Color::from_hex("#336699").unwrap();
        let faded = base.with_alpha(0.5);

        assert_eq!(faded.to_rgba_u8()[0..3], base.to_rgba_u8()[0..3]);
        assert_eq!(faded.a.to_u8(), 128);
        // Source stays untouched (value semantics).
        assert_eq!(base.a.to_u8(), 255);
    }

    #[test]
    fn with_alpha_clamps_and_is_const() {
        const OPAQUE: Color = Color::WHITE.with_alpha(2.0);
        const CLEAR: Color = Color::WHITE.with_alpha(-1.0);

        assert_eq!(OPAQUE.a.to_u8(), 255);
        assert_eq!(CLEAR.a.to_u8(), 0);
    }

    #[test]
    fn color_macro_parses_hex_at_compile_time() {
        const PANEL: Color = color!("#0B1018");
        const SHORT: Color = color!("#FFF");
        const WITH_ALPHA: Color = color!("#336699CC");
        const NO_HASH: Color = color!("336699");

        assert_eq!(PANEL.to_rgba_u8(), [0x0B, 0x10, 0x18, 0xFF]);
        assert_eq!(SHORT.to_rgba_u8(), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(WITH_ALPHA.to_rgba_u8(), [0x33, 0x66, 0x99, 0xCC]);
        assert_eq!(NO_HASH.to_rgba_u8(), [0x33, 0x66, 0x99, 0xFF]);
    }

    #[test]
    fn try_from_hex_const_matches_runtime_parser() {
        for hex in ["#336699CC", "#abc", "#ABCD", "112233", "#ffffff"] {
            assert_eq!(
                Color::try_from_hex_const(hex),
                Color::from_hex(hex),
                "{hex}"
            );
        }
        assert_eq!(Color::try_from_hex_const("#zzz"), None);
        assert_eq!(Color::try_from_hex_const("12"), None);
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
