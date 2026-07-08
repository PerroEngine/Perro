use perro_structs::{BitMask, Color, Transform3D};

#[derive(Clone, Debug)]
pub struct AmbientLight3D {
    pub transform: Transform3D,
    pub visible: bool,
    pub color: Color,
    pub intensity: f32,
    pub cast_shadows: bool,
    pub active: bool,
    pub render_layers: BitMask,
}

impl AmbientLight3D {
    pub const fn new() -> Self {
        Self {
            transform: Transform3D::IDENTITY,
            visible: true,
            color: Color::WHITE,
            intensity: 1.0,
            cast_shadows: true,
            active: true,
            render_layers: BitMask::ALL,
        }
    }
}

impl Default for AmbientLight3D {
    fn default() -> Self {
        Self::new()
    }
}
