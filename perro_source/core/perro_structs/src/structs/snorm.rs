use super::Vector2;
use bytemuck::{Pod, Zeroable};
use std::fmt;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Pod, Zeroable)]
pub struct SignedUnit(pub u8);

impl SignedUnit {
    pub const MIN: Self = Self(0);
    pub const ZERO: Self = Self(128);
    pub const MAX: Self = Self(u8::MAX);

    #[inline]
    pub const fn new(v: f32) -> Self {
        let v = if v < -1.0 {
            -1.0
        } else if v > 1.0 {
            1.0
        } else {
            v
        };
        Self(((v + 1.0) * 0.5 * 255.0 + 0.5) as u8)
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
        self.0 as f32 * (2.0 / 255.0) - 1.0
    }
}

impl From<u8> for SignedUnit {
    #[inline]
    fn from(value: u8) -> Self {
        Self::from_u8(value)
    }
}

impl From<SignedUnit> for u8 {
    #[inline]
    fn from(value: SignedUnit) -> Self {
        value.to_u8()
    }
}

impl From<SignedUnit> for f32 {
    #[inline]
    fn from(value: SignedUnit) -> Self {
        value.to_f32()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Pod, Zeroable)]
pub struct SignedUnitVector2 {
    pub x: SignedUnit,
    pub y: SignedUnit,
}

impl fmt::Display for SignedUnitVector2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SignedUnitVector2({}, {})",
            self.x.to_f32(),
            self.y.to_f32()
        )
    }
}

impl SignedUnitVector2 {
    pub const ZERO: Self = Self {
        x: SignedUnit::ZERO,
        y: SignedUnit::ZERO,
    };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x: SignedUnit::new(x),
            y: SignedUnit::new(y),
        }
    }

    #[inline]
    pub const fn from_u8(v: [u8; 2]) -> Self {
        Self {
            x: SignedUnit::from_u8(v[0]),
            y: SignedUnit::from_u8(v[1]),
        }
    }

    #[inline]
    pub const fn to_u8(self) -> [u8; 2] {
        [self.x.to_u8(), self.y.to_u8()]
    }

    #[inline]
    pub const fn to_array(self) -> [f32; 2] {
        [self.x.to_f32(), self.y.to_f32()]
    }

    #[inline]
    pub const fn to_tuple(self) -> (f32, f32) {
        (self.x.to_f32(), self.y.to_f32())
    }

    #[inline]
    pub const fn as_vector2(self) -> Vector2 {
        Vector2::new(self.x.to_f32(), self.y.to_f32())
    }
}

impl From<Vector2> for SignedUnitVector2 {
    #[inline]
    fn from(value: Vector2) -> Self {
        Self::new(value.x, value.y)
    }
}

impl From<SignedUnitVector2> for Vector2 {
    #[inline]
    fn from(value: SignedUnitVector2) -> Self {
        value.as_vector2()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_unit_rounds_and_clamps() {
        const LOW: SignedUnit = SignedUnit::new(-2.0);
        const MID: SignedUnit = SignedUnit::new(0.0);
        const HIGH: SignedUnit = SignedUnit::new(2.0);

        assert_eq!(LOW.to_u8(), 0);
        assert_eq!(MID.to_u8(), 128);
        assert_eq!(HIGH.to_u8(), 255);
    }

    #[test]
    fn signed_unit_vector2_packs_axes() {
        let v = SignedUnitVector2::new(-1.0, 1.0);

        assert_eq!(v.to_u8(), [0, 255]);
        assert_eq!(v.as_vector2().x, -1.0);
        assert_eq!(v.as_vector2().y, 1.0);
    }

    #[test]
    fn signed_units_convert_into_f32_and_vector2() {
        let axis: f32 = SignedUnit::new(1.0).into();
        let stick: Vector2 = SignedUnitVector2::new(-1.0, 1.0).into();

        assert_eq!(axis, 1.0);
        assert_eq!(stick.x, -1.0);
        assert_eq!(stick.y, 1.0);
    }
}
