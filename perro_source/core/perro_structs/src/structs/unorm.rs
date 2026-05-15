use bytemuck::{Pod, Zeroable};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Pod, Zeroable)]
pub struct Unorm8(pub u8);

impl Unorm8 {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(u8::MAX);

    #[inline]
    pub const fn new(v: f32) -> Self {
        let v = if v < 0.0 {
            0.0
        } else if v > 1.0 {
            1.0
        } else {
            v
        };
        Self((v * 255.0 + 0.5) as u8)
    }

    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        Self(v)
    }

    #[inline]
    pub const fn to_u8(self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn to_f32(self) -> f32 {
        self.0 as f32 / 255.0
    }
}

impl From<u8> for Unorm8 {
    #[inline]
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<Unorm8> for u8 {
    #[inline]
    fn from(value: Unorm8) -> Self {
        value.to_u8()
    }
}

impl From<Unorm8> for f32 {
    #[inline]
    fn from(value: Unorm8) -> Self {
        value.to_f32()
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Pod, Zeroable)]
pub struct Unorm8x4(pub [u8; 4]);

impl Unorm8x4 {
    pub const ZERO: Self = Self([0, 0, 0, 0]);
    pub const ONE: Self = Self([255, 255, 255, 255]);

    #[inline]
    pub const fn new(v: [f32; 4]) -> Self {
        Self([
            Unorm8::new(v[0]).to_u8(),
            Unorm8::new(v[1]).to_u8(),
            Unorm8::new(v[2]).to_u8(),
            Unorm8::new(v[3]).to_u8(),
        ])
    }

    #[inline]
    pub const fn from_u8(v: [u8; 4]) -> Self {
        Self(v)
    }

    #[inline]
    pub const fn to_u8(self) -> [u8; 4] {
        self.0
    }

    #[inline]
    pub const fn to_f32(self) -> [f32; 4] {
        [
            Unorm8(self.0[0]).to_f32(),
            Unorm8(self.0[1]).to_f32(),
            Unorm8(self.0[2]).to_f32(),
            Unorm8(self.0[3]).to_f32(),
        ]
    }

    #[inline]
    pub const fn to_le_u32(self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

impl From<[f32; 4]> for Unorm8x4 {
    #[inline]
    fn from(value: [f32; 4]) -> Self {
        Self::new(value)
    }
}

impl From<[u8; 4]> for Unorm8x4 {
    #[inline]
    fn from(value: [u8; 4]) -> Self {
        Self::from_u8(value)
    }
}

impl From<Unorm8x4> for [u8; 4] {
    #[inline]
    fn from(value: Unorm8x4) -> Self {
        value.to_u8()
    }
}

impl From<Unorm8x4> for [f32; 4] {
    #[inline]
    fn from(value: Unorm8x4) -> Self {
        value.to_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unorm8_const_rounds_and_clamps() {
        const ZERO: Unorm8 = Unorm8::new(-1.0);
        const HALF: Unorm8 = Unorm8::new(0.5);
        const ONE: Unorm8 = Unorm8::new(2.0);

        assert_eq!(ZERO.to_u8(), 0);
        assert_eq!(HALF.to_u8(), 128);
        assert_eq!(ONE.to_u8(), 255);
    }

    #[test]
    fn unorm8x4_packs_le_u32() {
        const COLOR: Unorm8x4 = Unorm8x4::new([1.0, 0.5, 0.0, 1.0]);

        assert_eq!(COLOR.to_u8(), [255, 128, 0, 255]);
        assert_eq!(COLOR.to_le_u32(), 0xFF00_80FF);
    }
}
