use crate::Vector2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawShape2D {
    Circle {
        radius: f32,
        color: [f32; 4],
        filled: bool,
        thickness: f32,
    },
    Rect {
        size: Vector2,
        color: [f32; 4],
        filled: bool,
        thickness: f32,
    },
}

impl DrawShape2D {
    #[inline]
    pub const fn circle(radius: f32, color: [f32; 4]) -> Self {
        Self::Circle {
            radius,
            color,
            filled: true,
            thickness: 1.0,
        }
    }

    #[inline]
    pub const fn ring(radius: f32, color: [f32; 4], thickness: f32) -> Self {
        Self::Circle {
            radius,
            color,
            filled: false,
            thickness,
        }
    }

    #[inline]
    pub const fn rect(size: Vector2, color: [f32; 4]) -> Self {
        Self::Rect {
            size,
            color,
            filled: true,
            thickness: 1.0,
        }
    }

    #[inline]
    pub const fn rect_stroke(size: Vector2, color: [f32; 4], thickness: f32) -> Self {
        Self::Rect {
            size,
            color,
            filled: false,
            thickness,
        }
    }
}
