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
        self.push(DrawShape2D::circle(radius, color), center);
    }

    #[inline]
    pub fn ring(&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32) {
        self.push(DrawShape2D::ring(radius, color, thickness), center);
    }
}

#[macro_export]
macro_rules! draw {
    ($res:expr, $shape:expr, $position:expr) => {
        $res.Draw2D().push($shape, $position)
    };
}
