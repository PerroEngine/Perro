use perro_structs::Transform2D;

#[derive(Clone, Debug)]
pub struct AmbientLight2D {
    pub transform: Transform2D,
    pub visible: bool,
    pub color: [f32; 3],
    pub intensity: f32,
    pub cast_shadows: bool,
    pub active: bool,
}

impl AmbientLight2D {
    pub const fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            visible: true,
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            cast_shadows: false,
            active: true,
        }
    }
}

impl Default for AmbientLight2D {
    fn default() -> Self {
        Self::new()
    }
}
