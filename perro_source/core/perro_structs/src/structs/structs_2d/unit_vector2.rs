use super::Vector2;
use crate::Unit;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct UnitVector2 {
    pub x: Unit,
    pub y: Unit,
}

impl fmt::Display for UnitVector2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnitVector2({}, {})", self.x.to_f32(), self.y.to_f32())
    }
}

impl UnitVector2 {
    pub const ZERO: Self = Self {
        x: Unit::MIN,
        y: Unit::MIN,
    };
    pub const ONE: Self = Self {
        x: Unit::MAX,
        y: Unit::MAX,
    };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x: Unit::new(x),
            y: Unit::new(y),
        }
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

impl From<Vector2> for UnitVector2 {
    #[inline]
    fn from(value: Vector2) -> Self {
        Self::new(value.x, value.y)
    }
}

impl From<UnitVector2> for Vector2 {
    #[inline]
    fn from(value: UnitVector2) -> Self {
        value.as_vector2()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_vector2_converts_into_vector2() {
        let packed = UnitVector2::new(0.25, 0.75);
        let v: Vector2 = packed.into();

        assert!((v.x - 0.25).abs() <= 1.0 / 255.0);
        assert!((v.y - 0.75).abs() <= 1.0 / 255.0);
    }
}
