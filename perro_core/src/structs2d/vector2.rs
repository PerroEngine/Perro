use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
    pub fn one () -> Self { Self { x: 1.0, y: 1.0 } }
}