use glam::Quat;

/// A quaternion representing rotation in 3D space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl std::fmt::Display for Quaternion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Quaternion({}, {}, {}, {})",
            self.x, self.y, self.z, self.w
        )
    }
}

impl Quaternion {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Convert to glam Quat
    #[inline]
    pub fn to_quat(self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }

    /// Create from glam Quat
    #[inline]
    pub fn from_quat(quat: Quat) -> Self {
        Self {
            x: quat.x,
            y: quat.y,
            z: quat.z,
            w: quat.w,
        }
    }

    /// Rotate around local X axis by `radians`.
    #[inline]
    pub fn rotate_x(&mut self, radians: f32) -> &mut Self {
        let rotated = Quat::from_rotation_x(radians) * self.to_quat();
        *self = rotated.into();
        self
    }

    /// Rotate around local Y axis by `radians`.
    #[inline]
    pub fn rotate_y(&mut self, radians: f32) -> &mut Self {
        let rotated = Quat::from_rotation_y(radians) * self.to_quat();
        *self = rotated.into();
        self
    }

    /// Rotate around local Z axis by `radians`.
    #[inline]
    pub fn rotate_z(&mut self, radians: f32) -> &mut Self {
        let rotated = Quat::from_rotation_z(radians) * self.to_quat();
        *self = rotated.into();
        self
    }

    /// Apply X, then Y, then Z local-axis rotations.
    #[inline]
    pub fn rotate_xyz(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.rotate_x(x).rotate_y(y).rotate_z(z)
    }
}

// Convenient conversions using From/Into traits
impl From<Quat> for Quaternion {
    #[inline]
    fn from(quat: Quat) -> Self {
        Self::from_quat(quat)
    }
}

impl From<Quaternion> for Quat {
    #[inline]
    fn from(q: Quaternion) -> Self {
        q.to_quat()
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::IDENTITY
    }
}
