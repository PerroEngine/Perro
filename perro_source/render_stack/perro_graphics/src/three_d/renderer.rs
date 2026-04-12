use crate::resources::ResourceStore;
use ahash::AHashMap;
use glam::{Mat4, Quat, Vec3};
use perro_ids::{MeshID, NodeID};
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, CameraProjectionState, MeshSurfaceBinding3D,
    PointLight3DState, RayLight3DState, SkeletonPalette, Sky3DState, SpotLight3DState,
};
use std::sync::Arc;
use std::time::Instant;

const SKY_DAY_SECONDS: f32 = 1580.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw3DKind {
    Mesh(MeshID),
    DebugPointCube,
    DebugEdgeCylinder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Draw3DInstance {
    pub node: NodeID,
    pub kind: Draw3DKind,
    pub surfaces: Arc<[MeshSurfaceBinding3D]>,
    pub model: [[f32; 4]; 4],
    pub skeleton: Option<SkeletonPalette>,
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
    ) {
        self.queued_draws.push(Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh),
            surfaces,
            model,
            skeleton,
        });
    }

    pub fn queue_debug_point(&mut self, node: NodeID, position: [f32; 3], size: f32) {
        let next = Draw3DInstance {
            node,
            kind: Draw3DKind::DebugPointCube,
            surfaces: Arc::from([]),
            model: debug_point_model(position, size).to_cols_array_2d(),
            skeleton: None,
        };
        let changed = self.retained_draws.get(&node) != Some(&next);
        if changed {
            self.retained_draws.insert(node, next);
            self.draw_revision = self.draw_revision.wrapping_add(1);
        }
    }

    pub fn queue_debug_line(
        &mut self,
        node: NodeID,
        start: [f32; 3],
        end: [f32; 3],
        thickness: f32,
    ) {
        if let Some(model) = debug_line_model(start, end, thickness) {
            let next = Draw3DInstance {
                node,
                kind: Draw3DKind::DebugEdgeCylinder,
                surfaces: Arc::from([]),
                model: model.to_cols_array_2d(),
                skeleton: None,
            };
            let changed = self.retained_draws.get(&node) != Some(&next);
            if changed {
                self.retained_draws.insert(node, next);
                self.draw_revision = self.draw_revision.wrapping_add(1);
            }
        } else if self.retained_draws.remove(&node).is_some() {
            self.draw_revision = self.draw_revision.wrapping_add(1);
        }
    }

    pub fn remove_node(&mut self, node: NodeID) {
        if self.retained_draws.remove(&node).is_some() {
            self.draw_revision = self.draw_revision.wrapping_add(1);
        }
        self.ambient_lights.remove(&node);
        self.skies.remove(&node);
        self.ray_lights.remove(&node);
        self.point_lights.remove(&node);
        self.spot_lights.remove(&node);
    }

    pub fn set_ambient_light(&mut self, node: NodeID, light: AmbientLight3DState) {
        self.ambient_lights.insert(node, light);
    }

    pub fn set_sky(&mut self, node: NodeID, sky: Sky3DState) {
        self.skies.insert(node, sky);
    }

    pub fn set_ray_light(&mut self, node: NodeID, light: RayLight3DState) {
        self.ray_lights.insert(node, light);
    }

    pub fn set_point_light(&mut self, node: NodeID, light: PointLight3DState) {
        self.point_lights.insert(node, light);
    }

    pub fn set_spot_light(&mut self, node: NodeID, light: SpotLight3DState) {
        self.spot_lights.insert(node, light);
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
                    if retained.model != draw.model {
                        retained.model = draw.model;
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
                }
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        if draws_changed {
            self.draw_revision = self.draw_revision.wrapping_add(1);
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
        let mut ray_lights_sorted: Vec<(NodeID, RayLight3DState)> =
            self.ray_lights.iter().map(|(n, l)| (*n, *l)).collect();
        ray_lights_sorted.sort_unstable_by_key(|(node, _)| node.as_u64());
        for (slot, (_, light)) in lighting.ray_lights.iter_mut().zip(ray_lights_sorted.iter()) {
            *slot = Some(*light);
        }

        let mut point_lights_sorted: Vec<(NodeID, PointLight3DState)> =
            self.point_lights.iter().map(|(n, l)| (*n, *l)).collect();
        point_lights_sorted.sort_unstable_by_key(|(node, _)| node.as_u64());
        for (slot, (_, light)) in lighting
            .point_lights
            .iter_mut()
            .zip(point_lights_sorted.iter())
        {
            *slot = Some(*light);
        }

        let mut spot_lights_sorted: Vec<(NodeID, SpotLight3DState)> =
            self.spot_lights.iter().map(|(n, l)| (*n, *l)).collect();
        spot_lights_sorted.sort_unstable_by_key(|(node, _)| node.as_u64());
        for (slot, (_, light)) in lighting
            .spot_lights
            .iter_mut()
            .zip(spot_lights_sorted.iter())
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

    pub fn draw_revision(&self) -> u64 {
        self.draw_revision
    }

    pub fn camera(&self) -> Camera3DState {
        self.camera.clone()
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

fn debug_point_model(position: [f32; 3], size: f32) -> Mat4 {
    let scale = Vec3::splat(size.max(0.001));
    Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, Vec3::from_array(position))
}

fn debug_line_model(start: [f32; 3], end: [f32; 3], thickness: f32) -> Option<Mat4> {
    let a = Vec3::from_array(start);
    let b = Vec3::from_array(end);
    let delta = b - a;
    let len = delta.length();
    if !len.is_finite() || len <= 1.0e-5 {
        return None;
    }
    let dir = delta / len;
    let up = Vec3::Y;
    let rot = Quat::from_rotation_arc(up, dir);
    let center = (a + b) * 0.5;
    let radius = thickness.max(0.001);
    let scale = Vec3::new(radius, len, radius);
    Some(Mat4::from_scale_rotation_translation(scale, rot, center))
}
