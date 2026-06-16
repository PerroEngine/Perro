use bytemuck::{Pod, Zeroable};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Pod, Zeroable)]
pub struct Unit(pub u8);

impl Unit {
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

impl From<u8> for Unit {
    #[inline]
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<Unit> for u8 {
    #[inline]
    fn from(value: Unit) -> Self {
        value.to_u8()
    }
}

impl From<Unit> for f32 {
    #[inline]
    fn from(value: Unit) -> Self {
        value.to_f32()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Pod, Zeroable)]
pub struct UnitVector4 {
    pub x: Unit,
    pub y: Unit,
    pub z: Unit,
    pub w: Unit,
}

impl UnitVector4 {
    pub const ZERO: Self = Self {
        x: Unit::MIN,
        y: Unit::MIN,
        z: Unit::MIN,
        w: Unit::MIN,
    };
    pub const ONE: Self = Self {
        x: Unit::MAX,
        y: Unit::MAX,
        z: Unit::MAX,
        w: Unit::MAX,
    };

    #[inline]
    pub const fn new(v: [f32; 4]) -> Self {
        Self {
            x: Unit::new(v[0]),
            y: Unit::new(v[1]),
            z: Unit::new(v[2]),
            w: Unit::new(v[3]),
        }
    }

    #[inline]
    pub const fn from_u8(v: [u8; 4]) -> Self {
        Self {
            x: Unit::from_u8(v[0]),
            y: Unit::from_u8(v[1]),
            z: Unit::from_u8(v[2]),
            w: Unit::from_u8(v[3]),
        }
    }

    #[inline]
    pub const fn to_u8(self) -> [u8; 4] {
        [
            self.x.to_u8(),
            self.y.to_u8(),
            self.z.to_u8(),
            self.w.to_u8(),
        ]
    }

    #[inline]
    pub const fn to_f32(self) -> [f32; 4] {
        [
            self.x.to_f32(),
            self.y.to_f32(),
            self.z.to_f32(),
            self.w.to_f32(),
        ]
    }

    #[inline]
    pub const fn to_le_u32(self) -> u32 {
        u32::from_le_bytes(self.to_u8())
    }
}

impl From<[f32; 4]> for UnitVector4 {
    #[inline]
    fn from(value: [f32; 4]) -> Self {
        Self::new(value)
    }
}

impl From<[u8; 4]> for UnitVector4 {
    #[inline]
    fn from(value: [u8; 4]) -> Self {
        Self::from_u8(value)
    }
}

impl From<UnitVector4> for [u8; 4] {
    #[inline]
    fn from(value: UnitVector4) -> Self {
        value.to_u8()
    }
}

impl From<UnitVector4> for [f32; 4] {
    #[inline]
    fn from(value: UnitVector4) -> Self {
        value.to_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_const_rounds_and_clamps() {
        const ZERO: Unit = Unit::new(-1.0);
        const HALF: Unit = Unit::new(0.5);
        const ONE: Unit = Unit::new(2.0);

        assert_eq!(ZERO.to_u8(), 0);
        assert_eq!(HALF.to_u8(), 128);
        assert_eq!(ONE.to_u8(), 255);
    }

    #[test]
    fn unit_vector4_packs_le_u32() {
        const COLOR: UnitVector4 = UnitVector4::new([1.0, 0.5, 0.0, 1.0]);

        assert_eq!(COLOR.to_u8(), [255, 128, 0, 255]);
        assert_eq!(COLOR.to_le_u32(), 0xFF00_80FF);
    }
}
