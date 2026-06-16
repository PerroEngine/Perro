use super::Vector3;
use crate::Unit;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct UnitVector3 {
    pub x: Unit,
    pub y: Unit,
    pub z: Unit,
}

impl fmt::Display for UnitVector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UnitVector3({}, {}, {})",
            self.x.to_f32(),
            self.y.to_f32(),
            self.z.to_f32()
        )
    }
}

impl UnitVector3 {
    pub const ZERO: Self = Self {
        x: Unit::MIN,
        y: Unit::MIN,
        z: Unit::MIN,
    };
    pub const ONE: Self = Self {
        x: Unit::MAX,
        y: Unit::MAX,
        z: Unit::MAX,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x: Unit::new(x),
            y: Unit::new(y),
            z: Unit::new(z),
        }
    }

    #[inline]
    pub const fn to_array(self) -> [f32; 3] {
        [self.x.to_f32(), self.y.to_f32(), self.z.to_f32()]
    }

    #[inline]
    pub const fn to_tuple(self) -> (f32, f32, f32) {
        (self.x.to_f32(), self.y.to_f32(), self.z.to_f32())
    }

    #[inline]
    pub const fn as_vector3(self) -> Vector3 {
        Vector3::new(self.x.to_f32(), self.y.to_f32(), self.z.to_f32())
    }
}

impl From<Vector3> for UnitVector3 {
    #[inline]
    fn from(value: Vector3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

impl From<UnitVector3> for Vector3 {
    #[inline]
    fn from(value: UnitVector3) -> Self {
        value.as_vector3()
    }
}
