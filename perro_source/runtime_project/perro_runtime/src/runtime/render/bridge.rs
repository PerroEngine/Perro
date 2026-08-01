//! Render bridge result intake and retained command output.

use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use ahash::{AHashMap, AHashSet};
use glam::{Mat3, Mat4};
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
    Water2DState, Water3DState, arc_slice_from_vec, empty_arc_slice,
};
use perro_runtime_render::{decode_3d_mesh_request_node, decode_render_request_node_from_event};
use perro_structs::{BitMask, Color, Vector2};
use perro_ui::ComputedUiRect;
use std::sync::Arc;

use crate::runtime::render_2d::{
    TilemapSpriteBuild, build_tilemap_sprites, derived_particle_budget, direction_from_rotation_2d,
    resolve_particle_profile_2d, resolve_particle_sim_mode_2d, resolve_tileset_2d,
    shadow_softness_2d, water_idle_mode_state as water_idle_mode_state_2d,
    water_render_size as water_render_size_2d, water_shape_state as water_shape_state_2d,
};
use crate::runtime::render_3d::{
    dense_instance_signature, derived_particle_budget_3d,
    resolve_particle_profile as resolve_particle_profile_3d,
    resolve_particle_render_mode as resolve_particle_render_mode_3d,
    resolve_particle_sim_mode as resolve_particle_sim_mode_3d, sky_3d_state_matches,
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

/// Per-refresh localization 4 stream member transforms. `SubView2D/3D` roots
/// localize members into root space; the root inverse is computed ONCE per
/// stream refresh here instead of per member per collector.
pub(super) enum StreamLocalize2D {
    /// non-sub-view stream: members keep their global transform.
    Global,
    /// sub-view root has no global transform: members fall back to their
    /// local transform (legacy `stream_render_transform_2d` returned None).
    Fail,
    Inverse(Mat3),
}

pub(super) enum StreamLocalize3D {
    Global,
    Fail,
    Inverse(Mat4),
}

/// Reuse a retained lane `Arc` when the freshly built slice equals it;
/// otherwise store + return a new `Arc` (shared empty 4 the empty case).
/// `None` slot = retention off (one-shot camera captures).
pub(super) fn retained_arc_lane<T>(slot: Option<&mut Arc<[T]>>, built: &[T]) -> Arc<[T]>
where
    T: PartialEq + Clone + perro_render_bridge::EmptyArcSlice,
{
    let fresh = |built: &[T]| -> Arc<[T]> {
        if built.is_empty() {
            empty_arc_slice()
        } else {
            Arc::from(built)
        }
    };
    match slot {
        Some(slot) => {
            if slot.as_ref() != built {
                *slot = fresh(built);
            }
            slot.clone()
        }
        None => fresh(built),
    }
}

/// Whole-state equality w/ `Arc::ptr_eq` fast paths on the lane slices. The
/// per-lane retention hands back ptr-identical `Arc`s 4 unchanged lanes, so
/// the steady-state compare is pointer checks + small scalar fields instead
/// of a deep walk of every draw. Exhaustive destructure: a new
/// `CameraStreamState` field breaks this compile instead of silently matching.
fn camera_stream_state_matches(prev: &CameraStreamState, next: &CameraStreamState) -> bool {
    #[inline]
    fn lane_eq<T: PartialEq>(a: &Arc<[T]>, b: &Arc<[T]>) -> bool {
        Arc::ptr_eq(a, b) || a == b
    }
    let CameraStreamState {
        source,
        tone_map_output,
        overlay_camera_2d,
        transparent_background,
        clear_color,
        resolution,
        aspect_ratio,
        post_processing,
        output_texture,
        sprites_2d,
        lights_2d,
        point_particles_2d,
        waters_2d,
        draws_3d,
        lighting_3d,
        point_particles_3d,
        waters_3d,
    } = prev;
    *tone_map_output == next.tone_map_output
        && *transparent_background == next.transparent_background
        && *clear_color == next.clear_color
        && *resolution == next.resolution
        && *aspect_ratio == next.aspect_ratio
        && *output_texture == next.output_texture
        && lane_eq(post_processing, &next.post_processing)
        && lane_eq(sprites_2d, &next.sprites_2d)
        && lane_eq(lights_2d, &next.lights_2d)
        && lane_eq(point_particles_2d, &next.point_particles_2d)
        && lane_eq(waters_2d, &next.waters_2d)
        && lane_eq(draws_3d, &next.draws_3d)
        && lane_eq(point_particles_3d, &next.point_particles_3d)
        && lane_eq(waters_3d, &next.waters_3d)
        && *source == next.source
        && *overlay_camera_2d == next.overlay_camera_2d
        && *lighting_3d == next.lighting_3d
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
    /// Flattened post-processing effects for a camera node. Unchanged sets
    /// (the steady state) hand out a refcount clone; a change re-flattens and
    /// replaces the cached pair. Entries drop with the node in
    /// `note_removed_render_node`.
    pub(crate) fn camera_postfx_arc(
        &mut self,
        node: NodeID,
        set: &perro_structs::PostProcessSet,
    ) -> Arc<[perro_structs::PostProcessEffect]> {
        if let Some((cached_set, cached)) = self.camera_postfx_cache.get(&node)
            && cached_set == set
        {
            return cached.clone();
        }
        let effects = arc_slice_from_vec(set.to_effects_vec());
        self.camera_postfx_cache
            .insert(node, (set.clone(), effects.clone()));
        effects
    }

    /// All CameraStream upsert/remove traffic for stream + sub-view nodes
    /// funnels through these two: `camera_stream_active` mirrors what the gpu
    /// retains, so idle passes stop re-sending RemoveNode for streams that
    /// were never (or are no longer) upserted — each command wakes a full gpu
    /// frame. One-shot camera captures bypass this (always paired).
    /// Returns the retained Arc actually sent, so the paired
    /// `Command3D`/`Command2D::UpsertCameraStream` at the call site rides the
    /// same dedup instead of shipping its own fresh allocation.
    pub(crate) fn queue_camera_stream_upsert(
        &mut self,
        node: NodeID,
        state: Arc<CameraStreamState>,
    ) -> Arc<CameraStreamState> {
        // Whole-state retention: an unchanged rebuild re-sends the previously
        // upserted Arc, so the gpu-side upsert hits Arc::ptr_eq instead of a
        // deep compare. Value-gated (never skips the command), so streams can
        // never go stale through this path.
        let state = match self.stream_retention.states.get(&node) {
            Some(prev) if camera_stream_state_matches(prev, &state) => prev.clone(),
            _ => {
                self.stream_retention.states.insert(node, state.clone());
                state
            }
        };
        self.camera_stream_active.insert(node);
        self.queue_render_command(RenderCommand::CameraStream(CameraStreamCommand::Upsert {
            node,
            state: state.clone(),
        }));
        state
    }

    pub(crate) fn queue_camera_stream_remove(&mut self, node: NodeID) {
        if self.camera_stream_active.remove(&node) {
            self.queue_render_command(RenderCommand::CameraStream(
                CameraStreamCommand::RemoveNode { node },
            ));
        }
    }

    /// Append main-world stream/sub-view nodes whose WATCHED world holds
    /// dirty nodes while the node itself is clean (content moved under a
    /// static camera; transform-only churn never marks the watcher). Dirty
    /// stream nodes already sit in the traversal, and full scans include
    /// everything, so callers skip this when include_all is set.
    fn append_dirty_world_stream_nodes(&mut self, traversal: &mut Vec<NodeID>, two_d: bool) {
        let mut dirty_worlds = std::mem::take(&mut self.dirty_world_scratch);
        self.collect_dirty_worlds(&mut dirty_worlds);
        if !dirty_worlds.is_empty() {
            let mut stream_nodes = std::mem::take(&mut self.stream_node_scratch);
            self.fill_stream_nodes(&mut stream_nodes);
            for node in stream_nodes.drain(..) {
                let Some(scene_node) = self.nodes.get(node) else {
                    continue;
                };
                let watched = match (&scene_node.data, two_d) {
                    (SceneNodeData::CameraStream2D(stream), true) => {
                        self.node_world(stream.stream.camera)
                    }
                    (SceneNodeData::SubView2D(_), true) => Some(node),
                    (SceneNodeData::CameraStream3D(stream), false) => {
                        self.node_world(stream.stream.camera)
                    }
                    (SceneNodeData::SubView3D(_), false) => Some(node),
                    _ => continue,
                };
                if self.dirty.is_node_dirty(node) || self.node_world(node) != Some(NodeID::nil()) {
                    continue;
                }
                if watched.is_some_and(|world| dirty_worlds.contains(&world)) {
                    traversal.push(node);
                }
            }
            stream_nodes.clear();
            self.stream_node_scratch = stream_nodes;
        }
        dirty_worlds.clear();
        self.dirty_world_scratch = dirty_worlds;
    }

    pub(crate) fn append_dirty_world_stream_nodes_2d(&mut self, traversal: &mut Vec<NodeID>) {
        self.append_dirty_world_stream_nodes(traversal, true);
    }

    pub(crate) fn append_dirty_world_stream_nodes_3d(&mut self, traversal: &mut Vec<NodeID>) {
        self.append_dirty_world_stream_nodes(traversal, false);
    }

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
        let stream_world = if self.is_sub_view_node(stream_node) {
            stream_node
        } else {
            self.node_world(stream_node).unwrap_or(NodeID::nil())
        };
        self.node_world(node) != Some(stream_world)
    }

    /// Once-per-refresh member localization 4 a stream root. Replaces the
    /// legacy per-member `root.to_mat3().inverse()` (up 2 4x members per
    /// refresh) with one inverse here.
    pub(super) fn stream_localizer_2d(&mut self, stream_node: NodeID) -> StreamLocalize2D {
        let localize = self
            .nodes
            .get(stream_node)
            .is_some_and(|root| matches!(root.data, SceneNodeData::SubView2D(_)));
        if !localize {
            return StreamLocalize2D::Global;
        }
        match self.get_render_global_transform_2d(stream_node) {
            Some(root) => StreamLocalize2D::Inverse(root.to_mat3().inverse()),
            None => StreamLocalize2D::Fail,
        }
    }

    pub(super) fn stream_localizer_3d(&mut self, stream_node: NodeID) -> StreamLocalize3D {
        let localize = self
            .nodes
            .get(stream_node)
            .is_some_and(|root| matches!(root.data, SceneNodeData::SubView3D(_)));
        if !localize {
            return StreamLocalize3D::Global;
        }
        match self.get_render_global_transform_3d(stream_node) {
            Some(root) => StreamLocalize3D::Inverse(root.to_mat4().inverse()),
            None => StreamLocalize3D::Fail,
        }
    }

    pub(super) fn stream_localized_transform_2d(
        &mut self,
        node: NodeID,
        localize: &StreamLocalize2D,
    ) -> Option<perro_structs::Transform2D> {
        let child = self.get_render_global_transform_2d(node)?;
        match localize {
            StreamLocalize2D::Global => Some(child),
            StreamLocalize2D::Fail => None,
            StreamLocalize2D::Inverse(inverse) => {
                let local = *inverse * child.to_mat3();
                local
                    .is_finite()
                    .then(|| perro_structs::Transform2D::from_mat3(local))
                    .or(Some(child))
            }
        }
    }

    pub(super) fn stream_localized_transform_3d(
        &mut self,
        node: NodeID,
        localize: &StreamLocalize3D,
    ) -> Option<perro_structs::Transform3D> {
        let child = self.get_render_global_transform_3d(node)?;
        match localize {
            StreamLocalize3D::Global => Some(child),
            StreamLocalize3D::Fail => None,
            StreamLocalize3D::Inverse(inverse) => {
                let local = *inverse * child.to_mat4();
                local
                    .is_finite()
                    .then(|| perro_structs::Transform3D::from_mat4(local))
                    .or(Some(child))
            }
        }
    }

    /// Camera-stream skinning palette, stamp-gated. Shares the retained
    /// builder (`build_skeleton_palette`) so the inverse-bind lane + 3-row
    /// affine packing stay in one place. The palette reads only the skeleton
    /// node's own data, so `node_change_stamp` is exact: same stamp -> reuse
    /// the retained Arc w/o rebuild (one entry serves every stream + refresh);
    /// changed stamp -> rebuild, then still reuse the Arc when contents match.
    pub(super) fn stream_skeleton_palette(
        &mut self,
        skeleton_id: NodeID,
        global_scratch: &mut Vec<Mat4>,
        palette_scratch: &mut Vec<[[f32; 4]; 3]>,
    ) -> Option<perro_render_bridge::SkeletonPalette> {
        let stamp = self.nodes.node_change_stamp(skeleton_id)?;
        if let Some((cached_stamp, palette)) =
            self.stream_retention.skeleton_palettes.get(&skeleton_id)
            && *cached_stamp == stamp
        {
            return Some(palette.clone());
        }
        crate::runtime::render_3d::build_skeleton_palette(
            &self.nodes,
            skeleton_id,
            global_scratch,
            palette_scratch,
        )?;
        let palette = match self.stream_retention.skeleton_palettes.get(&skeleton_id) {
            Some((_, prev)) if prev.matrices.as_ref() == palette_scratch.as_slice() => prev.clone(),
            _ => perro_render_bridge::SkeletonPalette {
                matrices: Arc::from(palette_scratch.as_slice()),
            },
        };
        self.stream_retention
            .skeleton_palettes
            .insert(skeleton_id, (stamp, palette.clone()));
        Some(palette)
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
