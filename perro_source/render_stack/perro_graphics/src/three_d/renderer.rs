use crate::resources::ResourceStore;
use ahash::AHashMap;
use perro_ids::{MeshID, NodeID};
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, DenseInstancePose3D,
    MeshSurfaceBinding3D, PointLight3DState, RayLight3DState, SkeletonPalette, Sky3DState,
    SpotLight3DState,
};
use std::sync::Arc;
use std::time::Instant;

const SKY_DAY_SECONDS: f32 = 1580.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Draw3DKind {
    Mesh(MeshID),
    DebugPointCube,
    DebugEdgeCylinder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DenseMultiMeshDraw3D {
    pub node_model: [[f32; 4]; 4],
    pub instance_scale: f32,
    pub instances: Arc<[DenseInstancePose3D]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Draw3DInstance {
    pub node: NodeID,
    pub kind: Draw3DKind,
    pub surfaces: Arc<[MeshSurfaceBinding3D]>,
    pub instance_mats: Arc<[[[f32; 4]; 4]]>,
    pub skeleton: Option<SkeletonPalette>,
    pub dense_multimesh: Option<DenseMultiMeshDraw3D>,
    pub meshlet_override: Option<bool>,
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
    retained_draws: AHashMap<NodeID, Draw3DInstance>,
    ambient_lights: AHashMap<NodeID, AmbientLight3DState>,
    skies: AHashMap<NodeID, Sky3DState>,
    ray_lights: AHashMap<NodeID, RayLight3DState>,
    point_lights: AHashMap<NodeID, PointLight3DState>,
    spot_lights: AHashMap<NodeID, SpotLight3DState>,
    ray_lights_sorted_cache: Vec<(NodeID, RayLight3DState)>,
    point_lights_sorted_cache: Vec<(NodeID, PointLight3DState)>,
    spot_lights_sorted_cache: Vec<(NodeID, SpotLight3DState)>,
    retained_draws_sorted_cache: Vec<Draw3DInstance>,
    ray_lights_dirty: bool,
    point_lights_dirty: bool,
    spot_lights_dirty: bool,
    camera: Camera3DState,
    draw_revision: u64,
    last_frame_time: Option<Instant>,
    cloud_time_seconds: f32,
}

impl Renderer3D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_camera(&mut self, camera: Camera3DState) {
        self.camera = camera;
    }

    pub fn queue_draw(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        model: [[f32; 4]; 4],
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            instance_mats: Arc::from([model]),
            skeleton,
            dense_multimesh: None,
            meshlet_override,
        });
    }

    pub fn queue_draw_multi(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        instance_mats: Arc<[[[f32; 4]; 4]]>,
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            instance_mats,
            skeleton,
            dense_multimesh: None,
            meshlet_override,
        });
    }

    pub fn queue_draw_multi_dense(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node_model: [[f32; 4]; 4],
        instance_scale: f32,
        instances: Arc<[DenseInstancePose3D]>,
        meshlet_override: Option<bool>,
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            // Dense path uploads compact pose data directly in GPU prepare.
            // Keep this empty to avoid N x matrix expansion in retained CPU state.
            instance_mats: Arc::from([]),
            skeleton: None,
            dense_multimesh: Some(DenseMultiMeshDraw3D {
                node_model,
                instance_scale,
                instances,
            }),
            meshlet_override,
        });
    }

    pub fn remove_node(&mut self, node: NodeID) {
        if self.retained_draws.remove(&node).is_some() {
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
        self.last_frame_time = Some(now);
        self.cloud_time_seconds = (self.cloud_time_seconds + dt.max(0.0)).rem_euclid(1.0e9);

        for draw in self.queued_draws.drain(..) {
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
            if draw_ready {
                let changed = self.retained_draws.get(&draw.node) != Some(&draw);
                if changed {
                    self.retained_draws.insert(draw.node, draw);
                    draws_changed = true;
                }
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                if let Some(retained) = self.retained_draws.get_mut(&draw.node) {
                    // Keep previous mesh/material bindings until replacements exist,
                    // but continue applying latest transform updates.
                    if retained.instance_mats != draw.instance_mats {
                        retained.instance_mats = draw.instance_mats;
                        draws_changed = true;
                    }
                    if mesh_ready && retained.kind != draw.kind {
                        retained.kind = draw.kind;
                        draws_changed = true;
                    }
                    if material_ready && retained.surfaces != draw.surfaces {
                        retained.surfaces = draw.surfaces;
                        draws_changed = true;
                    }
                    if draw.skeleton.is_some() && retained.skeleton != draw.skeleton {
                        retained.skeleton = draw.skeleton;
                        draws_changed = true;
                    }
                    if draw.dense_multimesh.is_some()
                        && retained.dense_multimesh != draw.dense_multimesh
                    {
                        retained.dense_multimesh = draw.dense_multimesh;
                        draws_changed = true;
                    }
                    if retained.meshlet_override != draw.meshlet_override {
                        retained.meshlet_override = draw.meshlet_override;
                        draws_changed = true;
                    }
                }
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        if draws_changed {
            self.draw_revision = self.draw_revision.wrapping_add(1);
            self.rebuild_sorted_draws_cache();
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

    pub fn retained_draw(&self, node: NodeID) -> Option<Draw3DInstance> {
        self.retained_draws.get(&node).cloned()
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
    }

    pub fn retained_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws.values().cloned()
    }

    pub fn all_draws(&self) -> impl Iterator<Item = Draw3DInstance> + '_ {
        self.retained_draws.values().cloned()
    }

    pub fn retained_draws_sorted(&self) -> &[Draw3DInstance] {
        &self.retained_draws_sorted_cache
    }

    pub fn draw_revision(&self) -> u64 {
        self.draw_revision
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
            self.retained_draws_sorted_cache.reserve(
                self.retained_draws.len() - self.retained_draws_sorted_cache.capacity(),
            );
        }
        self.retained_draws_sorted_cache
            .extend(self.retained_draws.values().cloned());
        self.retained_draws_sorted_cache
            .sort_unstable_by_key(|draw| draw.node.as_u64());
    }
}

impl Default for Renderer3D {
    fn default() -> Self {
        Self {
            queued_draws: Vec::new(),
            retained_draws: AHashMap::new(),
            ambient_lights: AHashMap::new(),
            skies: AHashMap::new(),
            ray_lights: AHashMap::new(),
            point_lights: AHashMap::new(),
            spot_lights: AHashMap::new(),
            ray_lights_sorted_cache: Vec::new(),
            point_lights_sorted_cache: Vec::new(),
            spot_lights_sorted_cache: Vec::new(),
            retained_draws_sorted_cache: Vec::new(),
            ray_lights_dirty: false,
            point_lights_dirty: false,
            spot_lights_dirty: false,
            // Keep a usable fallback view if no Camera3D node is active.
            camera: Camera3DState {
                position: [0.0, 0.0, 6.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                projection: CameraProjectionState::Perspective {
                    fov_y_degrees: 60.0,
                    near: 0.1,
                    far: 1000.0,
                },
                post_processing: Arc::from([]),
            },
            draw_revision: 0,
            last_frame_time: None,
            cloud_time_seconds: 0.0,
        }
    }
}
