use std::ops::{Deref, DerefMut};

use crate::node_3d::Node3D;
use perro_structs::{AudioListenerOptions, BitMask, PostProcessSet};

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

#[derive(Clone, Debug)]
pub struct Camera3D {
    pub base: Node3D,
    pub active: bool,
    pub render_mask: BitMask,
    pub projection: CameraProjection,
    pub post_processing: PostProcessSet,
    pub audio_options: AudioListenerOptions,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            base: Node3D::new(),
            active: false,
            render_mask: BitMask::NONE,
            projection: CameraProjection::default(),
            post_processing: PostProcessSet::new(),
            audio_options: AudioListenerOptions::new(),
        }
    }
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
            far: 1_000_000.0,
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
    #[deprecated(note = "use Camera3D::default()")]
    pub fn new() -> Self {
        Self::default()
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
        (near + 1_000_000.0).max(near + 0.001)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_camera_3d_masks_no_render_layers() {
        assert_eq!(Camera3D::default().render_mask, BitMask::NONE);
    }
}
