use std::ops::{Deref, DerefMut};

use crate::CameraProjection;
use perro_structs::{Color, PostProcessSet, Quaternion, UVector2, Vector2, Vector3};
use perro_ui::{UiImageScaleMode, UiNode, UiNodeBase};

/// UI-owned local render scope.
///
/// Spatial descendants render into this node's UI rect instead of the main
/// world. The view lives on the viewport, so no Camera2D/Camera3D child is
/// required.
#[derive(Clone, Debug)]
pub struct UiViewport {
    pub base: UiNode,
    /// Zero axes follow the computed UI rect size.
    pub resolution: UVector2,
    pub aspect_ratio: f32,
    pub aspect_mode: UiImageScaleMode,
    pub view_position: Vector3,
    pub view_rotation: Quaternion,
    pub projection: CameraProjection,
    pub view_2d_position: Vector2,
    pub view_2d_rotation: f32,
    pub view_2d_zoom: f32,
    pub post_processing: PostProcessSet,
    pub background: Color,
    pub tint: Color,
    pub corner_radius: f32,
    pub enabled: bool,
    pub suspend_when_hidden: bool,
}

impl Default for UiViewport {
    fn default() -> Self {
        Self {
            base: UiNode::new(),
            resolution: UVector2::new(0, 0),
            aspect_ratio: 0.0,
            aspect_mode: UiImageScaleMode::Fit,
            view_position: Vector3::new(0.0, 0.0, 5.0),
            view_rotation: Quaternion::IDENTITY,
            projection: CameraProjection::default(),
            view_2d_position: Vector2::ZERO,
            view_2d_rotation: 0.0,
            view_2d_zoom: 1.0,
            post_processing: PostProcessSet::new(),
            background: Color::TRANSPARENT,
            tint: Color::WHITE,
            corner_radius: 0.0,
            enabled: true,
            suspend_when_hidden: true,
        }
    }
}

impl Deref for UiViewport {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiViewport {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiViewport {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}
