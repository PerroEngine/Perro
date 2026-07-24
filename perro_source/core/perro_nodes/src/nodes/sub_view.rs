use std::ops::{Deref, DerefMut};

use crate::{CameraProjection, Node2D, Node3D};
use perro_structs::{Color, PostProcessSet, Quaternion, UVector2, Vector2, Vector3};
use perro_ui::{UiImageScaleMode, UiNode, UiNodeBase};

/// Isolated child render scope with implicit 2D and 3D views.
///
/// A sub view owns the rendered meaning of its descendants. Both 2D and 3D
/// descendants are accepted; the host node only selects where the resulting
/// premultiplied texture is composited. An active descendant `Camera2D` or
/// `Camera3D` replaces the matching implicit view for this scope.
#[derive(Clone, Debug, PartialEq)]
pub struct SubView {
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
    pub enabled: bool,
    pub suspend_when_hidden: bool,
}

impl Default for SubView {
    fn default() -> Self {
        Self {
            resolution: UVector2::new(512, 512),
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
            enabled: true,
            suspend_when_hidden: true,
        }
    }
}

/// UI-space host for an isolated mixed 2D/3D child render scope.
#[derive(Clone, Debug)]
pub struct UiSubView {
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

impl Default for UiSubView {
    fn default() -> Self {
        let sub_view = SubView::default();
        Self {
            base: UiNode::new(),
            resolution: UVector2::new(0, 0),
            aspect_ratio: sub_view.aspect_ratio,
            aspect_mode: sub_view.aspect_mode,
            view_position: sub_view.view_position,
            view_rotation: sub_view.view_rotation,
            projection: sub_view.projection,
            view_2d_position: sub_view.view_2d_position,
            view_2d_rotation: sub_view.view_2d_rotation,
            view_2d_zoom: sub_view.view_2d_zoom,
            post_processing: sub_view.post_processing,
            background: sub_view.background,
            tint: Color::WHITE,
            corner_radius: 0.0,
            enabled: sub_view.enabled,
            suspend_when_hidden: sub_view.suspend_when_hidden,
        }
    }
}

impl Deref for UiSubView {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiSubView {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiSubView {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

impl From<&UiSubView> for SubView {
    fn from(value: &UiSubView) -> Self {
        Self {
            resolution: value.resolution,
            aspect_ratio: value.aspect_ratio,
            aspect_mode: value.aspect_mode,
            view_position: value.view_position,
            view_rotation: value.view_rotation,
            projection: value.projection.clone(),
            view_2d_position: value.view_2d_position,
            view_2d_rotation: value.view_2d_rotation,
            view_2d_zoom: value.view_2d_zoom,
            post_processing: value.post_processing.clone(),
            background: value.background,
            enabled: value.enabled,
            suspend_when_hidden: value.suspend_when_hidden,
        }
    }
}

/// 2D-space host for an isolated mixed 2D/3D child render scope.
#[derive(Clone, Debug)]
pub struct SubView2D {
    pub base: Node2D,
    pub sub_view: SubView,
    pub size: Vector2,
    pub tint: Color,
}

impl Default for SubView2D {
    fn default() -> Self {
        Self {
            base: Node2D::new(),
            sub_view: SubView::default(),
            size: Vector2::ONE,
            tint: Color::WHITE,
        }
    }
}

impl Deref for SubView2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for SubView2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

/// 3D-space host for an isolated mixed 2D/3D child render scope.
#[derive(Clone, Debug)]
pub struct SubView3D {
    pub base: Node3D,
    pub sub_view: SubView,
    pub size: Vector2,
    pub tint: Color,
}

impl Default for SubView3D {
    fn default() -> Self {
        Self {
            base: Node3D::new(),
            sub_view: SubView::default(),
            size: Vector2::ONE,
            tint: Color::WHITE,
        }
    }
}

impl Deref for SubView3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for SubView3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

/// Old Rust API name. Scene files using `[UiViewport]` also load as
/// `UiSubView` through the node-name compatibility alias.
#[deprecated(note = "use UiSubView")]
pub type UiViewport = UiSubView;
