use perro_structs::{BitMask, Color, Transform2D};

#[derive(Clone, Debug)]
pub struct AmbientLight2D {
    pub transform: Transform2D,
    pub visible: bool,
    pub color: Color,
    pub intensity: f32,
    pub cast_shadows: bool,
    pub active: bool,
    pub render_layers: BitMask,
}

impl AmbientLight2D {
    pub const fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            visible: true,
            color: Color::WHITE,
            intensity: 0.0,
            cast_shadows: false,
            active: true,
            render_layers: BitMask::ALL,
        }
    }
}

impl Default for AmbientLight2D {
    fn default() -> Self {
        Self::new()
    }
}
