//! Draw2D resource API.
//!
//! Queues immediate 2D draw payloads through the resource layer.

use super::TextureAPI;
use perro_ids::TextureID;
use perro_structs::{DrawShape2D, Vector2};

pub trait Draw2DAPI {
    fn draw_2d_shape(&self, shape: DrawShape2D, position: Vector2);
}

pub struct Draw2DModule<'res, R: Draw2DAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: Draw2DAPI + ?Sized> Draw2DModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn push(&self, shape: DrawShape2D, position: Vector2) {
        self.api.draw_2d_shape(shape, position);
    }

    #[inline]
    pub fn circle(&self, center: Vector2, radius: f32, color: [f32; 4]) {
        self.push(DrawShape2D::circle(radius, color.into()), center);
    }

    #[inline]
    pub fn ring(&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32) {
        self.push(DrawShape2D::ring(radius, color.into(), thickness), center);
    }

    #[inline]
    pub fn rect(&self, center: Vector2, size: Vector2, color: [f32; 4]) {
        self.push(DrawShape2D::rect(size, color.into()), center);
    }

    #[inline]
    pub fn rect_stroke(&self, center: Vector2, size: Vector2, color: [f32; 4], thickness: f32) {
        self.push(
            DrawShape2D::rect_stroke(size, color.into(), thickness),
            center,
        );
    }

    #[inline]
    pub fn line(&self, start: Vector2, end: Vector2, color: [f32; 4], thickness: f32) {
        self.push(DrawShape2D::line(end, color.into(), thickness), start);
    }

    #[inline]
    pub fn polyline(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32) {
        self.push(
            DrawShape2D::polyline(points.into_boxed_slice(), color.into(), thickness),
            Vector2::ZERO,
        );
    }

    #[inline]
    pub fn polygon(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32) {
        self.push(
            DrawShape2D::polygon(points.into_boxed_slice(), color.into(), thickness),
            Vector2::ZERO,
        );
    }

    #[inline]
    pub fn path(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32) {
        self.push(
            DrawShape2D::path(points.into_boxed_slice(), color.into(), thickness),
            Vector2::ZERO,
        );
    }

    #[inline]
    pub fn sprite(&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4]) {
        self.push(DrawShape2D::sprite(texture, size, tint.into()), center);
    }

    #[inline]
    pub fn atlas_sprite(
        &self,
        center: Vector2,
        texture: TextureID,
        size: Vector2,
        tint: [f32; 4],
        texture_region: [f32; 4],
    ) {
        self.push(
            DrawShape2D::atlas_sprite(texture, size, tint.into(), texture_region),
            center,
        );
    }
}

impl<'res, R> Draw2DModule<'res, R>
where
    R: Draw2DAPI + TextureAPI + ?Sized,
{
    #[inline]
    pub fn sprite_path(&self, center: Vector2, source: &str, size: Vector2, tint: [f32; 4]) {
        let texture = self.api.load_texture(source);
        self.sprite(center, texture, size, tint);
    }
}

#[macro_export]
macro_rules! draw {
    ($res:expr, $shape:expr, $position:expr) => {
        $res.Draw2D().push($shape, $position)
    };
}
