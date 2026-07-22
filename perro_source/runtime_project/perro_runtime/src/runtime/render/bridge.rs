//! Render bridge result intake and retained command output.

use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use ahash::{AHashMap, AHashSet};
use glam::Mat4;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_nodes::{
    CameraProjection, CameraStream, NodeType, Renderable, SceneNodeData, Spatial, SubView,
};
use perro_render_bridge::{
    AmbientLight2DState, AmbientLight3DState, Camera2DState, Camera3DState, CameraProjectionState,
    CameraStreamCommand, CameraStreamDraw3DState, CameraStreamLighting3DState,
    CameraStreamSourceState, CameraStreamState, Command2D, Command3D, DenseInstancePose3D,
    EnvironmentMap3DState, LODOptions3D, Light2DState, MeshBlendOptions3D, PointLight2DState,
    PointLight3DState, PointParticles2DState, PointParticles3DState, RayLight2DState,
    RayLight3DState, RenderCommand, RenderEvent, RenderRequestID, ResourceCommand, Sky3DState,
    SkyShaderPass3DState, SkyTime3DState, SpotLight2DState, SpotLight3DState, Sprite2DCommand,
    Water2DState, Water3DState,
};
use perro_runtime_render::{decode_3d_mesh_request_node, decode_render_request_node_from_event};
use perro_structs::{BitMask, Color};
use std::sync::Arc;

use crate::runtime::render_2d::{
    TilemapSpriteBuild, build_tilemap_sprites, derived_particle_budget, direction_from_rotation_2d,
    resolve_particle_profile_2d, resolve_particle_sim_mode_2d, resolve_tileset_2d,
    shadow_softness_2d, water_idle_mode_state as water_idle_mode_state_2d,
    water_render_size as water_render_size_2d, water_shape_state as water_shape_state_2d,
};
use crate::runtime::render_3d::{
    derived_particle_budget_3d, resolve_particle_profile as resolve_particle_profile_3d,
    resolve_particle_render_mode as resolve_particle_render_mode_3d,
    resolve_particle_sim_mode as resolve_particle_sim_mode_3d,
    water_idle_mode_state as water_idle_mode_state_3d, water_render_size as water_render_size_3d,
    water_shape_state as water_shape_state_3d,
};

fn is_ui_node_data(data: &SceneNodeData) -> bool {
    matches!(
        data,
        SceneNodeData::UiNode(_)
            | SceneNodeData::UiCameraStream(_)
            | SceneNodeData::UiSubView(_)
            | SceneNodeData::UiPanel(_)
            | SceneNodeData::UiProgressBar(_)
            | SceneNodeData::UiButton(_)
            | SceneNodeData::UiCheckbox(_)
            | SceneNodeData::UiColorPicker(_)
            | SceneNodeData::UiImage(_)
            | SceneNodeData::UiVideoPlayer(_)
            | SceneNodeData::UiImageButton(_)
            | SceneNodeData::UiNineSliceButton(_)
            | SceneNodeData::UiNineSlice(_)
            | SceneNodeData::UiAnimatedImage(_)
            | SceneNodeData::UiLabel(_)
            | SceneNodeData::UiTextBox(_)
            | SceneNodeData::UiTextBlock(_)
            | SceneNodeData::UiScrollContainer(_)
            | SceneNodeData::UiLayout(_)
            | SceneNodeData::UiHLayout(_)
            | SceneNodeData::UiVLayout(_)
            | SceneNodeData::UiGrid(_)
            | SceneNodeData::UiTreeList(_)
    )
}

#[path = "bridge/commands.rs"]
mod commands;
#[path = "bridge/stream_2d.rs"]
mod stream_2d;
#[path = "bridge/stream_3d.rs"]
mod stream_3d;
#[path = "bridge/stream_state.rs"]
mod stream_state;

impl Runtime {
    pub(crate) const UI_DIRTY_TRANSFORM: u16 = crate::runtime::state::DirtyState::DIRTY_TRANSFORM;
    pub(crate) const UI_DIRTY_LAYOUT_SELF: u16 =
        crate::runtime::state::DirtyState::DIRTY_LAYOUT_SELF;
    pub(crate) const UI_DIRTY_LAYOUT_PARENT: u16 =
        crate::runtime::state::DirtyState::DIRTY_LAYOUT_PARENT;
    pub(crate) const UI_DIRTY_COMMANDS: u16 = crate::runtime::state::DirtyState::DIRTY_COMMANDS;
    pub(crate) const UI_DIRTY_TEXT: u16 = crate::runtime::state::DirtyState::DIRTY_TEXT;
}

/// Camera-stream skinning palette. Shares the retained builder
/// (`build_skeleton_palette`) so the inverse-bind lane and 3-row affine packing
/// stay in one place; scratch buffers are threaded in from the caller to avoid
/// a per-draw allocation.
fn stream_skeleton_palette(
    nodes: &crate::cns::NodeArena,
    skeleton_id: NodeID,
    global_scratch: &mut Vec<Mat4>,
    palette_scratch: &mut Vec<[[f32; 4]; 3]>,
) -> Option<perro_render_bridge::SkeletonPalette> {
    crate::runtime::render_3d::build_skeleton_palette(
        nodes,
        skeleton_id,
        global_scratch,
        palette_scratch,
    )?;
    Some(perro_render_bridge::SkeletonPalette {
        matrices: Arc::from(palette_scratch.as_slice()),
    })
}

enum StreamMeshInstanceKind {
    Single,
    Dense {
        instance_scale: f32,
        poses: Arc<[DenseInstancePose3D]>,
    },
}

#[inline]
fn stream_render_mask_matches(camera_mask: BitMask, render_layers: BitMask) -> bool {
    !camera_mask.intersects(render_layers)
}

fn stream_quaternion_forward(rotation: perro_structs::Quaternion) -> [f32; 3] {
    let q = glam::Quat::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w).normalize();
    let forward = q * glam::Vec3::NEG_Z;
    [forward.x, forward.y, forward.z]
}

fn stream_sprite_region_uv(region: Option<[f32; 4]>) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let Some([x, y, w, h]) = region else {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    };
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    }
    ([x, y], [x + w, y + h], [w, h])
}

fn camera_stream_projection_state(projection: &CameraProjection) -> CameraProjectionState {
    match projection {
        CameraProjection::Perspective {
            fov_y_degrees,
            near,
            far,
        } => CameraProjectionState::Perspective {
            fov_y_degrees: *fov_y_degrees,
            near: *near,
            far: *far,
        },
        CameraProjection::Orthographic { size, near, far } => CameraProjectionState::Orthographic {
            size: *size,
            near: *near,
            far: *far,
        },
        CameraProjection::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => CameraProjectionState::Frustum {
            left: *left,
            right: *right,
            bottom: *bottom,
            top: *top,
            near: *near,
            far: *far,
        },
    }
}

impl Runtime {
    fn is_sub_view_node(&self, node: NodeID) -> bool {
        self.nodes.get(node).is_some_and(|node| {
            matches!(
                node.data,
                SceneNodeData::UiSubView(_)
                    | SceneNodeData::SubView2D(_)
                    | SceneNodeData::SubView3D(_)
            )
        })
    }

    fn stream_skips_isolated_child(&self, node: NodeID, stream_node: NodeID) -> bool {
        !self.is_sub_view_node(stream_node) && self.is_under_sub_view(node)
    }

    fn stream_render_transform_2d(
        &mut self,
        node: NodeID,
        stream_node: NodeID,
    ) -> Option<perro_structs::Transform2D> {
        let child = self.get_render_global_transform_2d(node)?;
        let localize = self
            .nodes
            .get(stream_node)
            .is_some_and(|root| matches!(root.data, SceneNodeData::SubView2D(_)));
        if !localize {
            return Some(child);
        }
        let root = self.get_render_global_transform_2d(stream_node)?;
        let local = root.to_mat3().inverse() * child.to_mat3();
        local
            .is_finite()
            .then(|| perro_structs::Transform2D::from_mat3(local))
            .or(Some(child))
    }

    fn stream_render_transform_3d(
        &mut self,
        node: NodeID,
        stream_node: NodeID,
    ) -> Option<perro_structs::Transform3D> {
        let child = self.get_render_global_transform_3d(node)?;
        let localize = self
            .nodes
            .get(stream_node)
            .is_some_and(|root| matches!(root.data, SceneNodeData::SubView3D(_)));
        if !localize {
            return Some(child);
        }
        let root = self.get_render_global_transform_3d(stream_node)?;
        let local = root.to_mat4().inverse() * child.to_mat4();
        local
            .is_finite()
            .then(|| perro_structs::Transform3D::from_mat4(local))
            .or(Some(child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::{Node3D, SceneNode};
    use std::sync::Arc;

    #[test]
    fn force_rerender_visits_corrupt_child_cycle_once() {
        let mut runtime = Runtime::new();
        let a = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        let b = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        runtime
            .nodes
            .get_mut(a)
            .expect("test or bench setup must succeed")
            .add_child(b);
        runtime
            .nodes
            .get_mut(b)
            .expect("test or bench setup must succeed")
            .add_child(a);
        runtime.clear_dirty_flags();

        runtime.force_rerender(a);

        assert_eq!(runtime.dirty_node_count(), 2);
    }

    #[test]
    fn texture_loaded_rescans_but_texels_updated_does_not() {
        let mut runtime = Runtime::new();
        let texture = perro_ids::TextureID::from_parts(5, 0);

        // first load: full 2d + 3d scan + resource-ref recount.
        runtime.render_2d.force_full_scan_once = false;
        runtime.render_3d.force_full_scan_once = false;
        runtime.scene_resource_refs_dirty = false;
        runtime.apply_render_event(RenderEvent::TextureLoaded { id: texture });
        assert!(runtime.render_2d.full_scan_pending());
        assert!(runtime.render_3d.full_scan_pending());
        assert!(runtime.scene_resource_refs_dirty);

        // repeat texel write: no rescan, no ref recount.
        runtime.render_2d.force_full_scan_once = false;
        runtime.render_3d.force_full_scan_once = false;
        runtime.scene_resource_refs_dirty = false;
        runtime.apply_render_event(RenderEvent::TextureTexelsUpdated { id: texture });
        assert!(!runtime.render_2d.full_scan_pending());
        assert!(!runtime.render_3d.full_scan_pending());
        assert!(!runtime.scene_resource_refs_dirty);
    }

    #[test]
    fn water_body_samples_derive_vertical_velocity_from_height_delta() {
        let mut runtime = Runtime::new();
        let water = NodeID::from_parts(10, 0);
        let body = NodeID::from_parts(20, 0);

        runtime.time.elapsed = 1.0;
        runtime.apply_render_event(RenderEvent::WaterBodySamples {
            samples: Arc::from([perro_render_bridge::WaterBodySampleState {
                water,
                body,
                point: 0,
                local: [0.0, 0.0],
                height: 1.0,
                velocity: [0.0, 0.0],
                foam: 0.0,
            }]),
        });
        runtime.time.elapsed = 1.1;
        runtime.apply_render_event(RenderEvent::WaterBodySamples {
            samples: Arc::from([perro_render_bridge::WaterBodySampleState {
                water,
                body,
                point: 0,
                local: [0.0, 0.0],
                height: 1.3,
                velocity: [0.0, 0.0],
                foam: 0.0,
            }]),
        });

        let cached = runtime
            .water_body_samples
            .get(&crate::runtime::WaterBodySampleKey {
                water,
                body,
                point: 0,
            })
            .copied()
            .expect("cached water body sample");
        assert!(cached.velocity.y > 2.9);
    }
}
