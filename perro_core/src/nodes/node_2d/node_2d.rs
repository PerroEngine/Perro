use crate::Transform2D;

#[derive(Clone, Debug, Default)]
pub struct Node2D {
    pub transform: Transform2D,
    pub z_index: i32,
    pub visible: bool,
}

impl Node2D {
    pub fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            visible: true,
            z_index: 0,
        }
    }
}
