use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Shape2D {
    Rectangle { width: f32, height: f32 },
    Circle { radius: f32 },
    Square { size: f32 },
    Triangle { base: f32, height: f32 },
}

impl fmt::Display for Shape2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shape2D::Rectangle { width, height } => write!(f, "Shape2D::Rectangle(w:{}, h:{})", width, height),
            Shape2D::Circle { radius } => write!(f, "Shape2D::Circle(r:{})", radius),
            Shape2D::Square { size } => write!(f, "Shape2D::Square(s:{})", size),
            Shape2D::Triangle { base, height } => write!(f, "Shape2D::Triangle(base:{}, height:{})", base, height),
        }
    }
}
