use crate::Color;
use crate::Vector2;
use perro_ids::TextureID;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum DrawShape2D {
    Circle {
        radius: f32,
        color: Color,
        filled: bool,
        thickness: f32,
    },
    Rect {
        size: Vector2,
        color: Color,
        filled: bool,
        thickness: f32,
    },
    Line {
        end: Vector2,
        color: Color,
        thickness: f32,
    },
    Polyline {
        points: Arc<[Vector2]>,
        color: Color,
        thickness: f32,
        closed: bool,
    },
    Path {
        points: Arc<[Vector2]>,
        color: Color,
        thickness: f32,
    },
    Sprite {
        texture: TextureID,
        size: Vector2,
        tint: Color,
        texture_region: Option<[f32; 4]>,
    },
}

impl DrawShape2D {
    #[inline]
    pub const fn circle(radius: f32, color: Color) -> Self {
        Self::Circle {
            radius,
            color,
            filled: true,
            thickness: 1.0,
        }
    }

    #[inline]
    pub const fn ring(radius: f32, color: Color, thickness: f32) -> Self {
        Self::Circle {
            radius,
            color,
            filled: false,
            thickness,
        }
    }

    #[inline]
    pub const fn rect(size: Vector2, color: Color) -> Self {
        Self::Rect {
            size,
            color,
            filled: true,
            thickness: 1.0,
        }
    }

    #[inline]
    pub const fn rect_stroke(size: Vector2, color: Color, thickness: f32) -> Self {
        Self::Rect {
            size,
            color,
            filled: false,
            thickness,
        }
    }

    #[inline]
    pub const fn line(end: Vector2, color: Color, thickness: f32) -> Self {
        Self::Line {
            end,
            color,
            thickness,
        }
    }

    #[inline]
    pub fn polyline(points: impl Into<Arc<[Vector2]>>, color: Color, thickness: f32) -> Self {
        Self::Polyline {
            points: points.into(),
            color,
            thickness,
            closed: false,
        }
    }

    #[inline]
    pub fn polygon(points: impl Into<Arc<[Vector2]>>, color: Color, thickness: f32) -> Self {
        Self::Polyline {
            points: points.into(),
            color,
            thickness,
            closed: true,
        }
    }

    #[inline]
    pub fn path(points: impl Into<Arc<[Vector2]>>, color: Color, thickness: f32) -> Self {
        Self::Path {
            points: points.into(),
            color,
            thickness,
        }
    }

    #[inline]
    pub const fn sprite(texture: TextureID, size: Vector2, tint: Color) -> Self {
        Self::Sprite {
            texture,
            size,
            tint,
            texture_region: None,
        }
    }

    #[inline]
    pub const fn atlas_sprite(
        texture: TextureID,
        size: Vector2,
        tint: Color,
        texture_region: [f32; 4],
    ) -> Self {
        Self::Sprite {
            texture,
            size,
            tint,
            texture_region: Some(texture_region),
        }
    }
}
