use std::ops::{Deref, DerefMut};

use crate::{Node2D, Node3D};
use perro_ids::NodeID;
use perro_structs::{Color, PostProcessSet, UVector2};
use perro_ui::{UiBox, UiImageScaleMode, UiNodeBase};

#[derive(Clone, Debug, PartialEq)]
pub struct CameraStream {
    pub camera: NodeID,
    pub resolution: UVector2,
    pub aspect_ratio: f32,
    pub aspect_mode: UiImageScaleMode,
    pub post_processing: PostProcessSet,
    pub enabled: bool,
}

impl CameraStream {
    pub fn new() -> Self {
        Self {
            camera: NodeID::nil(),
            resolution: UVector2::new(512, 512),
            aspect_ratio: 0.0,
            aspect_mode: UiImageScaleMode::Fit,
            post_processing: PostProcessSet::new(),
            enabled: true,
        }
    }
}

impl Default for CameraStream {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct UiCameraStream {
    pub base: UiBox,
    pub stream: CameraStream,
    pub tint: Color,
}

impl UiCameraStream {
    pub fn new() -> Self {
        Self {
            base: UiBox::new(),
            stream: CameraStream::new(),
            tint: Color::WHITE,
        }
    }
}

impl Default for UiCameraStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiCameraStream {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiCameraStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiCameraStream {
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct CameraStream2D {
    pub base: Node2D,
    pub stream: CameraStream,
    pub tint: Color,
}

impl CameraStream2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            stream: CameraStream::new(),
            tint: Color::WHITE,
        }
    }
}

impl Default for CameraStream2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for CameraStream2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for CameraStream2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct CameraStream3D {
    pub base: Node3D,
    pub stream: CameraStream,
    pub size: [f32; 2],
    pub tint: Color,
}

impl CameraStream3D {
    pub fn new() -> Self {
        Self {
            base: Node3D::new(),
            stream: CameraStream::new(),
            size: [1.0, 1.0],
            tint: Color::WHITE,
        }
    }
}

impl Default for CameraStream3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for CameraStream3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for CameraStream3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
