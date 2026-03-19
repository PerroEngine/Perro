use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::node_3d::Node3D;
use perro_structs::PostProcessEffect;

impl Deref for Camera3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Camera3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Camera3D {
    pub base: Node3D,
    pub active: bool,
    pub projection: CameraProjection,
    pub post_processing: Cow<'static, [PostProcessEffect]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CameraProjection {
    Perspective {
        fov_y_degrees: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        size: f32,
        near: f32,
        far: f32,
    },
    Frustum {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

impl Default for CameraProjection {
    fn default() -> Self {
        Self::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl CameraProjection {
    pub fn perspective(fov_y_degrees: f32, near: f32, far: f32) -> Self {
        let (near, far) = sanitize_near_far(near, far);
        Self::Perspective {
            fov_y_degrees: fov_y_degrees.clamp(10.0, 180.0),
            near,
            far,
        }
    }

    pub fn orthographic(size: f32, near: f32, far: f32) -> Self {
        let (near, far) = sanitize_near_far(near, far);
        Self::Orthographic {
            size: size.abs().max(0.001),
            near,
            far,
        }
    }

    pub fn frustum(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let (near, far) = sanitize_near_far(near, far);
        let (left, right) = sanitize_range(left, right, -1.0, 1.0);
        let (bottom, top) = sanitize_range(bottom, top, -1.0, 1.0);
        Self::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        }
    }
}

impl Camera3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            active: false,
            projection: CameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 1000.0,
            },
            post_processing: Cow::Borrowed(&[]),
        }
    }

    pub fn set_projection(&mut self, projection: CameraProjection) {
        self.projection = projection;
    }

    pub fn with_projection(mut self, projection: CameraProjection) -> Self {
        self.projection = projection;
        self
    }

    pub fn set_perspective(&mut self, fov_y_degrees: f32, near: f32, far: f32) {
        self.projection = CameraProjection::perspective(fov_y_degrees, near, far);
    }

    pub fn set_orthographic(&mut self, size: f32, near: f32, far: f32) {
        self.projection = CameraProjection::orthographic(size, near, far);
    }

    pub fn set_frustum(
        &mut self,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) {
        self.projection = CameraProjection::frustum(left, right, bottom, top, near, far);
    }
}

fn sanitize_near_far(near: f32, far: f32) -> (f32, f32) {
    let near = if near.is_finite() {
        near.max(0.001)
    } else {
        0.1
    };
    let far = if far.is_finite() {
        far.max(near + 0.001)
    } else {
        (near + 1000.0).max(near + 0.001)
    };
    (near, far)
}

fn sanitize_range(min: f32, max: f32, fallback_min: f32, fallback_max: f32) -> (f32, f32) {
    let mut a = if min.is_finite() { min } else { fallback_min };
    let mut b = if max.is_finite() { max } else { fallback_max };
    if (b - a).abs() < 1.0e-6 {
        a = fallback_min;
        b = fallback_max;
    }
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}
