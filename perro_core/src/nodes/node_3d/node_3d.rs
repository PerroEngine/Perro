use crate::Transform3D;

#[derive(Clone, Debug, Default)]
pub struct Node3D {
    pub transform: Transform3D,
    pub visible: bool,
}

impl Node3D {
    pub const fn new() -> Self {
        Self {
            transform: Transform3D::IDENTITY,
            visible: true,
        }
    }
}
