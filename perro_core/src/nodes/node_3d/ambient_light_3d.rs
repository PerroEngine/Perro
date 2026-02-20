use crate::Transform3D;

#[derive(Clone, Debug)]
pub struct AmbientLight3D {
    pub transform: Transform3D,
    pub visible: bool,
    pub color: [f32; 3],
    pub intensity: f32,
    pub active: bool,
}

impl AmbientLight3D {
    pub const fn new() -> Self {
        Self {
            transform: Transform3D::IDENTITY,
            visible: true,
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            active: true,
        }
    }
}

impl Default for AmbientLight3D {
    fn default() -> Self {
        Self::new()
    }
}
