use crate::resources::ResourceStore;
use ahash::AHashMap;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{MeshID, NodeID};
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, DenseInstancePose3D, LODOptions3D,
    MeshBlendOptions3D, MeshSurfaceBinding3D, PointLight3DState, RayLight3DState, SkeletonPalette,
    Sky3DState, SpotLight3DState, Water3DState,
};
use rayon::slice::ParallelSliceMut;
use std::sync::Arc;
use std::time::Instant;

const SKY_DAY_SECONDS: f32 = 1580.0;
const SKY_CLOUD_TIME_SPEED_SCALE: f32 = 0.2;
const SKY_CLOUD_TIME_UPDATE_EVERY_FRAMES: u32 = 3;
const PARALLEL_PREP_SORT_MIN: usize = 10_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Draw3DKind {
    Mesh(MeshID),
    DebugPointCube,
    DebugEdgeCylinder,
}

#[derive(Debug, Clone)]
pub struct DenseMultiMeshDraw3D {
    pub node_model: [[f32; 4]; 4],
    pub instance_scale: f32,
    pub instances: Arc<[DenseInstancePose3D]>,
}

impl PartialEq for DenseMultiMeshDraw3D {
    fn eq(&self, other: &Self) -> bool {
        self.node_model == other.node_model
            && self.instance_scale == other.instance_scale
            && (Arc::ptr_eq(&self.instances, &other.instances) || self.instances == other.instances)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Draw3DInstance {
    pub node: NodeID,
    pub kind: Draw3DKind,
    pub surfaces: Arc<[MeshSurfaceBinding3D]>,
    pub instance_mats: Arc<[[[f32; 4]; 4]]>,
    pub debug_color: Option<[f32; 4]>,
    pub skeleton: Option<SkeletonPalette>,
    pub dense_multimesh: Option<DenseMultiMeshDraw3D>,
    pub meshlet_override: Option<bool>,
    pub lod: LODOptions3D,
    pub blend: MeshBlendOptions3D,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Renderer3DStats {
    pub accepted_draws: u32,
    pub rejected_draws: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lighting3DState {
    pub ambient_light: Option<AmbientLight3DState>,
    pub sky: Option<Sky3DState>,
    pub sky_cloud_time_seconds: f32,
    pub ray_lights: [Option<RayLight3DState>; MAX_RAY_LIGHTS],
    pub point_lights: [Option<PointLight3DState>; MAX_POINT_LIGHTS],
    pub spot_lights: [Option<SpotLight3DState>; MAX_SPOT_LIGHTS],
}

pub const MAX_RAY_LIGHTS: usize = 3;
pub const MAX_POINT_LIGHTS: usize = 8;
pub const MAX_SPOT_LIGHTS: usize = 8;

pub struct Renderer3D {
    queued_draws: Vec<Draw3DInstance>,
    retained_draws: Vec<Draw3DInstance>,
    node_to_draw_index: AHashMap<NodeID, usize>,
    ambient_lights: AHashMap<NodeID, AmbientLight3DState>,
    skies: AHashMap<NodeID, Sky3DState>,
    ray_lights: AHashMap<NodeID, RayLight3DState>,
    point_lights: AHashMap<NodeID, PointLight3DState>,
    spot_lights: AHashMap<NodeID, SpotLight3DState>,
    waters: AHashMap<NodeID, Water3DState>,
    ray_lights_sorted_cache: Vec<(NodeID, RayLight3DState)>,
    point_lights_sorted_cache: Vec<(NodeID, PointLight3DState)>,
    spot_lights_sorted_cache: Vec<(NodeID, SpotLight3DState)>,
    waters_sorted_cache: Vec<(NodeID, Water3DState)>,
    retained_draws_sorted_cache: Vec<Draw3DInstance>,
    ray_lights_dirty: bool,
    point_lights_dirty: bool,
    spot_lights_dirty: bool,
    waters_dirty: bool,
    waters_revision: u64,
    camera: Camera3DState,
    draw_revision: u64,
    last_frame_time: Option<Instant>,
    cloud_time_seconds: f32,
    cloud_time_pending_seconds: f32,
    cloud_time_pending_frames: u32,
}

impl Renderer3D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_camera(&mut self, camera: Camera3DState) {
        self.camera = camera;
    }

    pub fn reserve_queued_draws(&mut self, additional: usize) {
        self.queued_draws.reserve(additional);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn queue_draw(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        model: [[f32; 4]; 4],
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            instance_mats: Arc::from([model]),
            debug_color: None,
            skeleton,
            dense_multimesh: None,
            meshlet_override,
            lod,
            blend,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn queue_draw_multi(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        instance_mats: Arc<[[[f32; 4]; 4]]>,
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            instance_mats,
            debug_color: None,
            skeleton,
            dense_multimesh: None,
            meshlet_override,
            lod,
            blend,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn queue_draw_multi_dense(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        dense_draw: DenseMultiMeshDraw3D,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            // Dense path uploads compact pose data directly in GPU prepare.
            // Keep this empty to avoid N x matrix expansion in retained CPU state.
            instance_mats: Arc::from([]),
            debug_color: None,
            skeleton: None,
            dense_multimesh: Some(dense_draw),
            meshlet_override,
            lod,
            blend,
        });
    }

    pub fn queue_debug_point(
        &mut self,
        node: NodeID,
        position: [f32; 3],
        size: f32,
        color: [f32; 4],
    ) {
        let model = Mat4::from_scale_rotation_translation(
            Vec3::splat(size.max(0.001)),
            Quat::IDENTITY,
            Vec3::from(position),
        )
        .to_cols_array_2d();
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::DebugPointCube,
            surfaces: Arc::from([]),
            instance_mats: Arc::from([model]),
            debug_color: Some(color),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: Some(false),
            lod: LODOptions3D::default(),
            blend: MeshBlendOptions3D::default(),
        });
    }

    pub fn queue_debug_line(
        &mut self,
        node: NodeID,
        start: [f32; 3],
        end: [f32; 3],
        thickness: f32,
        color: [f32; 4],
    ) {
        let start = Vec3::from(start);
        let end = Vec3::from(end);
        let delta = end - start;
        let length = delta.length();
        if length <= 0.0001 {
            return;
        }
        let dir = delta / length;
        let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(thickness.max(0.001), length, thickness.max(0.001)),
            rotation,
            (start + end) * 0.5,
        )
        .to_cols_array_2d();
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::DebugEdgeCylinder,
            surfaces: Arc::from([]),
            instance_mats: Arc::from([model]),
            debug_color: Some(color),
            skeleton: None,
            dense_multimesh: None,
            meshlet_override: Some(false),
            lod: LODOptions3D::default(),
            blend: MeshBlendOptions3D::default(),
        });
    }

    pub fn remove_node(&mut self, node: NodeID) {
        if self.remove_retained_draw(node) {
            self.draw_revision = self.draw_revision.wrapping_add(1);
            self.rebuild_sorted_draws_cache();
        }
        self.ambient_lights.remove(&node);
        self.skies.remove(&node);
        if self.ray_lights.remove(&node).is_some() {
            self.ray_lights_dirty = true;
        }
        if self.point_lights.remove(&node).is_some() {
            self.point_lights_dirty = true;
        }
        if self.spot_lights.remove(&node).is_some() {
            self.spot_lights_dirty = true;
        }
        if self.waters.remove(&node).is_some() {
            self.waters_dirty = true;
            self.waters_revision = self.waters_revision.wrapping_add(1);
        }
    }

    pub fn set_ambient_light(&mut self, node: NodeID, light: AmbientLight3DState) {
        self.ambient_lights.insert(node, light);
    }

    pub fn set_sky(&mut self, node: NodeID, sky: Sky3DState) {
        self.skies.insert(node, sky);
    }

    pub fn set_ray_light(&mut self, node: NodeID, light: RayLight3DState) {
        if self.ray_lights.insert(node, light) != Some(light) {
            self.ray_lights_dirty = true;
        }
    }

    pub fn set_point_light(&mut self, node: NodeID, light: PointLight3DState) {
        if self.point_lights.insert(node, light) != Some(light) {
            self.point_lights_dirty = true;
        }
    }

    pub fn set_spot_light(&mut self, node: NodeID, light: SpotLight3DState) {
        if self.spot_lights.insert(node, light) != Some(light) {
            self.spot_lights_dirty = true;
        }
    }

    pub fn upsert_water(&mut self, node: NodeID, water: Water3DState) {
        match self.waters.get_mut(&node) {
            Some(existing) if *existing == water => {}
            Some(existing) => {
                *existing = water;
                self.waters_dirty = true;
                self.waters_revision = self.waters_revision.wrapping_add(1);
            }
            None => {
                self.waters.insert(node, water);
                self.waters_dirty = true;
                self.waters_revision = self.waters_revision.wrapping_add(1);
            }
        }
    }

    pub fn prepare_frame(
        &mut self,
        resources: &ResourceStore,
    ) -> (Camera3DState, Renderer3DStats, Lighting3DState) {
        let mut stats = Renderer3DStats::default();
        let mut draws_changed = false;
        let now = Instant::now();
        let dt = self
            .last_frame_time
            .map(|prev| now.duration_since(prev).as_secs_f32())
            .unwrap_or(0.0);
        let dt = dt.max(0.0);
        self.last_frame_time = Some(now);
        self.cloud_time_pending_seconds += dt;
        self.cloud_time_pending_frames = self.cloud_time_pending_frames.wrapping_add(1);
        if self.cloud_time_pending_frames >= SKY_CLOUD_TIME_UPDATE_EVERY_FRAMES {
            let cloud_step = self.cloud_time_pending_seconds * SKY_CLOUD_TIME_SPEED_SCALE;
            self.cloud_time_seconds = (self.cloud_time_seconds + cloud_step).rem_euclid(1.0e9);
            self.cloud_time_pending_seconds = 0.0;
            self.cloud_time_pending_frames = 0;
        }

        let queued = std::mem::take(&mut self.queued_draws);
        let used_sequential_draw_fast_path = if let Some((fast_stats, fast_changed)) =
            self.try_apply_sequential_draw_packets(queued.as_slice(), resources)
        {
            stats = fast_stats;
            draws_changed = fast_changed;
            true
        } else {
            for draw in queued {
                let (material_ready, mesh_ready, draw_ready) = draw_readiness(&draw, resources);
                if draw_ready {
                    let changed = self.retained_draw(draw.node).as_ref() != Some(&draw);
                    if changed {
                        self.upsert_retained_draw(draw);
                        draws_changed = true;
                    }
                    stats.accepted_draws = stats.accepted_draws.saturating_add(1);
                } else {
                    if let Some(retained) = self.retained_draw_mut(draw.node) {
                        // Keep previous mesh/material bindings until replacements exist,
                        // but continue applying latest transform updates.
                        draws_changed |= update_unready_retained_draw(
                            retained,
                            draw,
                            mesh_ready,
                            material_ready,
                        );
                    }
                    stats.rejected_draws = stats.rejected_draws.saturating_add(1);
                }
            }
            false
        };
        if draws_changed {
            self.draw_revision = self.draw_revision.wrapping_add(1);
            if !used_sequential_draw_fast_path {
                self.rebuild_sorted_draws_cache();
            }
        }

        let mut lighting = Lighting3DState::default();
        if let Some((_, ambient)) = self.ambient_lights.iter().next() {
            lighting.ambient_light = Some(*ambient);
        }
        if let Some((&sky_node, _)) = self.skies.iter().next()
            && let Some(sky) = self.skies.get_mut(&sky_node)
        {
            if !sky.time.paused {
                let scaled = dt.max(0.0) * sky.time.scale.max(0.0) / SKY_DAY_SECONDS;
                sky.time.time_of_day = (sky.time.time_of_day + scaled).rem_euclid(1.0);
            }
            lighting.sky = Some(sky.clone());
            lighting.sky_cloud_time_seconds = self.cloud_time_seconds;
        }
        self.rebuild_sorted_lights_if_dirty();
        for (slot, (_, light)) in lighting
            .ray_lights
            .iter_mut()
            .zip(self.ray_lights_sorted_cache.iter())
        {
            *slot = Some(*light);
        }

        for (slot, (_, light)) in lighting
            .point_lights
            .iter_mut()
            .zip(self.point_lights_sorted_cache.iter())
        {
            *slot = Some(*light);
        }

        for (slot, (_, light)) in lighting
            .spot_lights
            .iter_mut()
            .zip(self.spot_lights_sorted_cache.iter())
        {
            *slot = Some(*light);
        }

        (self.camera.clone(), stats, lighting)
    }

    fn try_apply_sequential_draw_packets(
        &mut self,
        queued: &[Draw3DInstance],
        resources: &ResourceStore,
    ) -> Option<(Renderer3DStats, bool)> {
        if queued.len() != self.retained_draws_sorted_cache.len() {
            return None;
        }
        if !queued
            .iter()
            .zip(self.retained_draws_sorted_cache.iter())
            .all(|(queued, retained)| queued.node == retained.node)
        {
            return None;
        }

        let mut stats = Renderer3DStats::default();
        let mut draws_changed = false;
        for (index, draw) in queued.iter().enumerate() {
            let (material_ready, mesh_ready, draw_ready) = draw_readiness(draw, resources);
            if draw_ready {
                let changed = self.retained_draws_sorted_cache[index] != *draw;
                if changed {
                    self.retained_draws_sorted_cache[index] = draw.clone();
                    self.upsert_retained_draw(draw.clone());
                    draws_changed = true;
                }
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                if let Some(retained) = self.retained_draw_mut(draw.node) {
                    draws_changed |= update_unready_retained_draw(
                        retained,
                        draw.clone(),
                        mesh_ready,
                        material_ready,
                    );
                    let retained_updated = retained.clone();
                    if self.retained_draws_sorted_cache[index] != retained_updated {
                        self.retained_draws_sorted_cache[index] = retained_updated;
                    }
                }
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        Some((stats, draws_changed))
    }

    pub fn retained_draw(&self, node: NodeID) -> Option<Draw3DInstance> {
        let idx = *self.node_to_draw_index.get(&node)?;
        self.retained_draws.get(idx).cloned()
    }

    pub fn retained_draw_count(&self) -> usize {
        self.retained_draws.len()
    }

    pub fn has_retained_non_draw_state(&self) -> bool {
        !self.ambient_lights.is_empty()
            || !self.skies.is_empty()
            || !self.ray_lights.is_empty()
            || !self.point_lights.is_empty()
            || !self.spot_lights.is_empty()
            || !self.waters.is_empty()
    }

    pub fn retained_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws.iter().cloned()
    }

    pub fn all_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws.iter().cloned()
    }

    pub fn retained_draws_sorted(&self) -> &[Draw3DInstance] {
        &self.retained_draws_sorted_cache
    }

    pub fn draw_revision(&self) -> u64 {
        self.draw_revision
    }

    pub fn retained_waters_sorted(&mut self) -> &[(NodeID, Water3DState)] {
        if self.waters_dirty {
            self.waters_sorted_cache.clear();
            self.waters_sorted_cache
                .extend(self.waters.iter().map(|(n, w)| (*n, w.clone())));
            self.waters_sorted_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.waters_dirty = false;
        }
        &self.waters_sorted_cache
    }

    #[inline]
    pub fn retained_waters_revision(&self) -> u64 {
        self.waters_revision
    }

    pub fn camera(&self) -> Camera3DState {
        self.camera.clone()
    }

    #[inline]
    pub fn has_active_sky_animation(&self) -> bool {
        self.skies.values().any(|sky| !sky.time.paused)
    }

    fn rebuild_sorted_lights_if_dirty(&mut self) {
        if self.ray_lights_dirty {
            self.ray_lights_sorted_cache.clear();
            self.ray_lights_sorted_cache
                .extend(self.ray_lights.iter().map(|(n, l)| (*n, *l)));
            self.ray_lights_sorted_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.ray_lights_dirty = false;
        }
        if self.point_lights_dirty {
            self.point_lights_sorted_cache.clear();
            self.point_lights_sorted_cache
                .extend(self.point_lights.iter().map(|(n, l)| (*n, *l)));
            self.point_lights_sorted_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.point_lights_dirty = false;
        }
        if self.spot_lights_dirty {
            self.spot_lights_sorted_cache.clear();
            self.spot_lights_sorted_cache
                .extend(self.spot_lights.iter().map(|(n, l)| (*n, *l)));
            self.spot_lights_sorted_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.spot_lights_dirty = false;
        }
    }

    fn rebuild_sorted_draws_cache(&mut self) {
        self.retained_draws_sorted_cache.clear();
        if self.retained_draws_sorted_cache.capacity() < self.retained_draws.len() {
            self.retained_draws_sorted_cache
                .reserve(self.retained_draws.len() - self.retained_draws_sorted_cache.capacity());
        }
        self.retained_draws_sorted_cache
            .extend(self.retained_draws.iter().cloned());
        if self.retained_draws_sorted_cache.len() >= PARALLEL_PREP_SORT_MIN {
            self.retained_draws_sorted_cache
                .par_sort_unstable_by_key(|draw| draw.node.as_u64());
        } else {
            self.retained_draws_sorted_cache
                .sort_unstable_by_key(|draw| draw.node.as_u64());
        }
    }

    fn upsert_retained_draw(&mut self, draw: Draw3DInstance) {
        if let Some(&idx) = self.node_to_draw_index.get(&draw.node) {
            self.retained_draws[idx] = draw;
            return;
        }
        let idx = self.retained_draws.len();
        self.retained_draws.push(draw);
        self.node_to_draw_index
            .insert(self.retained_draws[idx].node, idx);
    }

    fn retained_draw_mut(&mut self, node: NodeID) -> Option<&mut Draw3DInstance> {
        let idx = *self.node_to_draw_index.get(&node)?;
        self.retained_draws.get_mut(idx)
    }

    fn remove_retained_draw(&mut self, node: NodeID) -> bool {
        let Some(removed_idx) = self.node_to_draw_index.remove(&node) else {
            return false;
        };
        let last = self.retained_draws.len() - 1;
        self.retained_draws.swap_remove(removed_idx);
        if removed_idx != last {
            let moved_node = self.retained_draws[removed_idx].node;
            self.node_to_draw_index.insert(moved_node, removed_idx);
        }
        true
    }
}

fn draw_readiness(draw: &Draw3DInstance, resources: &ResourceStore) -> (bool, bool, bool) {
    let material_ready = draw.surfaces.iter().all(|surface| {
        surface
            .material
            .map(|id| resources.has_material(id))
            .unwrap_or(true)
    });
    let mesh_ready = match draw.kind {
        Draw3DKind::Mesh(mesh) => resources.has_mesh(mesh),
        Draw3DKind::DebugPointCube | Draw3DKind::DebugEdgeCylinder => true,
    };
    let draw_ready = match draw.kind {
        Draw3DKind::Mesh(_) => mesh_ready && material_ready,
        Draw3DKind::DebugPointCube | Draw3DKind::DebugEdgeCylinder => material_ready,
    };
    (material_ready, mesh_ready, draw_ready)
}

fn update_unready_retained_draw(
    retained: &mut Draw3DInstance,
    draw: Draw3DInstance,
    mesh_ready: bool,
    material_ready: bool,
) -> bool {
    let mut changed = false;
    if retained.instance_mats != draw.instance_mats {
        retained.instance_mats = draw.instance_mats;
        changed = true;
    }
    if mesh_ready && retained.kind != draw.kind {
        retained.kind = draw.kind;
        changed = true;
    }
    if material_ready && retained.surfaces != draw.surfaces {
        retained.surfaces = draw.surfaces;
        changed = true;
    }
    if draw.skeleton.is_some() && retained.skeleton != draw.skeleton {
        retained.skeleton = draw.skeleton;
        changed = true;
    }
    if draw.dense_multimesh.is_some() && retained.dense_multimesh != draw.dense_multimesh {
        retained.dense_multimesh = draw.dense_multimesh;
        changed = true;
    }
    if retained.meshlet_override != draw.meshlet_override {
        retained.meshlet_override = draw.meshlet_override;
        changed = true;
    }
    if retained.lod != draw.lod {
        retained.lod = draw.lod;
        changed = true;
    }
    changed
}

impl Default for Renderer3D {
    fn default() -> Self {
        Self {
            queued_draws: Vec::new(),
            retained_draws: Vec::new(),
            node_to_draw_index: AHashMap::new(),
            ambient_lights: AHashMap::new(),
            skies: AHashMap::new(),
            ray_lights: AHashMap::new(),
            point_lights: AHashMap::new(),
            spot_lights: AHashMap::new(),
            waters: AHashMap::new(),
            ray_lights_sorted_cache: Vec::new(),
            point_lights_sorted_cache: Vec::new(),
            spot_lights_sorted_cache: Vec::new(),
            waters_sorted_cache: Vec::new(),
            retained_draws_sorted_cache: Vec::new(),
            ray_lights_dirty: false,
            point_lights_dirty: false,
            spot_lights_dirty: false,
            waters_dirty: false,
            waters_revision: 0,
            // Keep a usable fallback view if no Camera3D node is active.
            camera: Camera3DState {
                position: [0.0, 0.0, 6.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                projection: CameraProjectionState::Perspective {
                    fov_y_degrees: 60.0,
                    near: 0.1,
                    far: 1000.0,
                },
                render_mask: perro_structs::BitMask::NONE,
                post_processing: Arc::from([]),
                audio_options: perro_structs::AudioListenerOptions::new(),
            },
            draw_revision: 0,
            last_frame_time: None,
            cloud_time_seconds: 0.0,
            cloud_time_pending_seconds: 0.0,
            cloud_time_pending_frames: 0,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/three_d_renderer_tests.rs"]
mod tests;
