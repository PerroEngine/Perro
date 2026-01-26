use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Shape2D {
    Rectangle { width: f32, height: f32 },
    Circle { radius: f32 },
    Square { size: f32 },
    Triangle { base: f32, height: f32 },
}
