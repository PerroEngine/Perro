use super::RuntimePhysicsStepTiming;
use crate::Runtime;
use ahash::{AHashMap, AHashSet};
use glam::{Mat3, Mat4, Vec3};
use perro_ids::{NodeID, SignalID};
#[cfg(test)]
use perro_nodes::TileMap2D;
use perro_nodes::{SceneNodeData, Shape2D, Shape3D, WaterShape, water_physics_sample_or_idle};
use perro_physics::*;
use perro_runtime_api::sub_apis::{
    NodeAPI, PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D,
    PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D, SignalAPI,
};
use perro_structs::{BitMask, Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;
use rayon::prelude::*;

const WATER_FORCE_PAR_BODY_THRESHOLD: usize = 1024;
const WATER_WAVE_FOLLOW_DT: f32 = 1.0 / 60.0;
const WATER_BODY_SAMPLE_TTL: f32 = 0.20;
const WATER_QUERY_LOCAL_EPS: f32 = 0.35;
const WATER_QUERY_MAX_PER_WATER: usize = 128;

pub(crate) type PhysicsState = PhysicsSystem;
pub(crate) use perro_physics::{AudioRaycastInput, AudioRaycastResult};

fn water_force_lod(
    near_distance: f32,
    mid_distance: f32,
    far_distance: f32,
    water_pos: Vector2,
    camera_pos: Vector2,
) -> (f32, f32) {
    let distance = Vector2::distance(water_pos, camera_pos);
    let near = near_distance.max(0.0);
    let mid = mid_distance.max(near);
    let far = far_distance.max(mid);
    if distance <= near {
        return (1.0, 0.0);
    }
    if distance <= mid {
        let t = ((distance - near) / (mid - near).max(0.001)).clamp(0.0, 1.0);
        return (1.0 - t * 0.25, 0.02 * t);
    }
    if distance <= far {
        let t = ((distance - mid) / (far - mid).max(0.001)).clamp(0.0, 1.0);
        return (0.75 - t * 0.35, 0.02 + 0.18 * t);
    }
    (0.25, 0.5)
}

pub(crate) fn water_target_submerged(density: f32) -> f32 {
    (0.08 + density.max(0.0) * 0.08).clamp(0.08, 0.45)
}

fn water_buoyancy_cap(mass: f32, submerged: f32, depth: f32, buoyancy: f32) -> f32 {
    let deep_recovery = (submerged - depth.max(0.0)).max(0.0) * 8.0;
    let lift_recovery = buoyancy.max(0.0) * 12.0;
    let mass = mass.max(0.001);
    mass * mass.sqrt() * (9.81 + lift_recovery + deep_recovery)
}

#[derive(Clone, Copy)]
struct RuntimeWater2D {
    id: NodeID,
    half: Vector2,
    transform: Mat3,
    inv_transform: Mat3,
    normal: Vector2,
    min_x: f32,
    max_x: f32,
    surface: perro_nodes::WaterSurfaceParams,
}

#[derive(Clone, Copy)]
struct RuntimeWater3D {
    id: NodeID,
    half: Vector2,
    transform: Mat4,
    inv_transform: Mat4,
    normal: Vector3,
    min_x: f32,
    max_x: f32,
    surface: perro_nodes::WaterSurfaceParams,
}

struct RuntimeWaterIndex2D {
    waters: Vec<RuntimeWater2D>,
    bins: Vec<Vec<usize>>,
    origin_x: f32,
    inv_cell_width: f32,
}

struct RuntimeWaterIndex3D {
    waters: Vec<RuntimeWater3D>,
    bins: Vec<Vec<usize>>,
    origin_x: f32,
    inv_cell_width: f32,
}

#[derive(Clone, Copy)]
struct RuntimeWaterBody2D {
    id: NodeID,
    pos: Vector2,
    velocity: Vector2,
    mass: f32,
    density: f32,
    float_radius: f32,
    collision_layers: BitMask,
    collision_mask: BitMask,
}

#[derive(Clone, Copy)]
struct RuntimeWaterBody3D {
    id: NodeID,
    pos: Vector3,
    velocity: Vector3,
    mass: f32,
    density: f32,
    float_radius: f32,
    collision_layers: BitMask,
    collision_mask: BitMask,
}

#[derive(Clone, Copy)]
struct WaterCandidate2D {
    water: RuntimeWater2D,
    local: Vector2,
    surface_point: Vector2,
    normal: Vector2,
    wave_dir: Vector2,
    sample: perro_nodes::WaterPhysicsSample,
    weight: f32,
}

#[derive(Clone, Copy)]
struct WaterCandidate3D {
    water: RuntimeWater3D,
    local: Vector3,
    surface_point: Vector3,
    normal: Vector3,
    wave_dir: Vector3,
    sample: perro_nodes::WaterPhysicsSample,
    weight: f32,
}

#[derive(Clone, Copy)]
struct BlendedWaterSample2D {
    pos: Vector2,
    normal: Vector2,
    wave_dir: Vector2,
    submerged: f32,
    surface: perro_nodes::WaterSurfaceParams,
    sample: perro_nodes::WaterPhysicsSample,
    lod_weight: f32,
}

#[derive(Clone, Copy)]
struct BlendedWaterSample3D {
    pos: Vector3,
    normal: Vector3,
    wave_dir: Vector3,
    submerged: f32,
    surface: perro_nodes::WaterSurfaceParams,
    sample: perro_nodes::WaterPhysicsSample,
    lod_weight: f32,
}

impl Runtime {
    pub fn set_physics_paused(&mut self, paused: bool) {
        if self.physics.paused() == paused {
            return;
        }
        self.physics.set_paused(paused);
        let water_nodes = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(
                    node.data,
                    SceneNodeData::WaterBody2D(_) | SceneNodeData::WaterBody3D(_)
                )
                .then_some(id)
            })
            .collect::<Vec<_>>();
        for id in water_nodes {
            self.mark_needs_rerender(id);
        }
    }

    pub fn physics_paused(&self) -> bool {
        self.physics.paused()
    }

    pub(crate) fn physics_fixed_step_timed(&mut self) -> RuntimePhysicsStepTiming {
        let total_start = std::time::Instant::now();

        let pre_transforms_start = std::time::Instant::now();
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let pre_transforms = pre_transforms_start.elapsed();

        let collect_start = std::time::Instant::now();
        let bodies_2d = self.collect_body_descs_2d();
        let bodies_3d = self.collect_body_descs_3d();
        let joints_2d = self.collect_joint_descs_2d();
        let joints_3d = self.collect_joint_descs_3d();
        let collect = collect_start.elapsed();

        let sync_world_start = std::time::Instant::now();
        self.sync_world_2d(&bodies_2d);
        self.sync_world_3d(&bodies_3d);
        self.sync_joints_2d(&joints_2d);
        self.sync_joints_3d(&joints_3d);
        let sync_world = sync_world_start.elapsed();

        if self.physics.paused {
            return RuntimePhysicsStepTiming {
                pre_transforms,
                collect,
                sync_world,
                apply_forces_impulses: std::time::Duration::ZERO,
                step: std::time::Duration::ZERO,
                sync_nodes: std::time::Duration::ZERO,
                post_transforms: std::time::Duration::ZERO,
                signals: std::time::Duration::ZERO,
                total: total_start.elapsed(),
            };
        }

        let apply_forces_impulses_start = std::time::Instant::now();
        self.queue_physics_force_emitters_2d();
        self.queue_physics_force_emitters_3d();
        self.queue_water_forces_2d();
        self.queue_water_forces_3d();
        self.apply_pending_forces_2d();
        self.apply_pending_forces_3d();
        self.apply_pending_impulses_2d();
        self.apply_pending_impulses_3d();
        let apply_forces_impulses = apply_forces_impulses_start.elapsed();

        let step_start = std::time::Instant::now();
        self.step_world_2d();
        self.step_world_3d();
        let step = step_start.elapsed();

        let sync_nodes_start = std::time::Instant::now();
        self.sync_world_to_nodes_2d();
        self.sync_world_to_nodes_3d();
        let sync_nodes = sync_nodes_start.elapsed();

        let post_transforms_start = std::time::Instant::now();
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let post_transforms = post_transforms_start.elapsed();

        let signals_start = std::time::Instant::now();
        self.emit_collision_signals_2d();
        self.emit_collision_signals_3d();
        self.emit_area_signals_2d();
        self.emit_area_signals_3d();
        let signals = signals_start.elapsed();

        RuntimePhysicsStepTiming {
            pre_transforms,
            collect,
            sync_world,
            apply_forces_impulses,
            step,
            sync_nodes,
            post_transforms,
            signals,
            total: total_start.elapsed(),
        }
    }

    pub(crate) fn physics_fixed_step(&mut self) {
        let _ = self.physics_fixed_step_timed();
    }

    pub(crate) fn queue_impulse_2d(&mut self, id: NodeID, impulse: Vector2) {
        self.physics.queue_impulse_2d(id, impulse);
    }

    pub(crate) fn queue_force_2d(&mut self, id: NodeID, force: Vector2) {
        self.physics.queue_force_2d(id, force);
    }

    pub(crate) fn queue_impulse_3d(&mut self, id: NodeID, impulse: Vector3) {
        self.physics.queue_impulse_3d(id, impulse);
    }

    pub(crate) fn queue_force_3d(&mut self, id: NodeID, force: Vector3) {
        self.physics.queue_force_3d(id, force);
    }

    pub(crate) fn emit_force_2d(&mut self, emitter: perro_nodes::PhysicsForceEmitter2D) -> bool {
        self.pending_force_emitters_2d.push(emitter);
        true
    }

    pub(crate) fn emit_force_3d(&mut self, emitter: perro_nodes::PhysicsForceEmitter3D) -> bool {
        self.pending_force_emitters_3d.push(emitter);
        true
    }

    pub(crate) fn clear_physics(&mut self) {
        self.physics.clear();
    }

    pub fn physics_raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D> {
        self.physics_raycast_3d_filtered(
            origin,
            direction,
            max_distance,
            &PhysicsQueryFilter {
                include_areas,
                ..PhysicsQueryFilter::default()
            },
        )
    }

    pub fn physics_raycast_3d_filtered(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit3D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);
        self.physics
            .raycast_3d_filtered(origin, direction, max_distance, filter)
    }

    pub fn physics_raycast_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);
        self.physics
            .raycast_2d(origin, direction, max_distance, filter)
    }

    pub(crate) fn prepare_audio_raycast_2d(&mut self) {
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);
        self.physics.update_query_pipeline_2d();
    }

    pub(crate) fn prepare_audio_raycast_3d(&mut self) {
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);
        self.physics.update_query_pipeline_3d();
    }

    #[allow(dead_code)]
    pub(crate) fn prepared_audio_raycast_2d(
        &self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        self.physics
            .prepared_audio_raycast_2d(origin, direction, max_distance, filter)
    }

    #[allow(dead_code)]
    pub(crate) fn prepared_audio_raycast_3d(
        &self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D> {
        self.physics
            .prepared_audio_raycast_3d(origin, direction, max_distance, include_areas)
    }

    pub(crate) fn cast_prepared_audio_rays(
        &self,
        inputs: &[AudioRaycastInput],
        outputs: &mut [AudioRaycastResult],
        parallel: bool,
    ) {
        self.physics
            .cast_prepared_audio_rays(inputs, outputs, parallel);
    }

    pub fn physics_shape_cast_2d(
        &mut self,
        shape: Shape2D,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);
        self.physics
            .shape_cast_2d(shape, origin, direction, max_distance, filter)
    }

    pub fn physics_shape_cast_3d(
        &mut self,
        shape: Shape3D,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);
        self.physics
            .shape_cast_3d(shape, origin, direction, max_distance, filter)
    }

    pub fn physics_contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);
        self.physics.contacts_2d(body_id)
    }

    pub fn physics_contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);
        self.physics.contacts_3d(body_id)
    }

    fn collect_body_descs_2d(&mut self) -> Vec<BodyDesc2D> {
        let node_count = self.internal_updates.physics_body_nodes_2d.len();
        let mut out = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let id = self.internal_updates.physics_body_nodes_2d[i];
            let (kind, enabled, rigid, material, groups) = {
                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                match &node.data {
                    SceneNodeData::StaticBody2D(body) => (
                        BodyKind::Static,
                        body.enabled,
                        None,
                        (body.friction, body.restitution, body.density),
                        (body.collision_layers, body.collision_mask),
                    ),
                    SceneNodeData::Area2D(body) => (
                        BodyKind::Area,
                        body.enabled,
                        None,
                        (0.7, 0.0, 1.0),
                        (body.collision_layers, body.collision_mask),
                    ),
                    SceneNodeData::WaterBody2D(water) => (
                        BodyKind::Area,
                        water.visible,
                        None,
                        (0.7, 0.0, 1.0),
                        (water.water.collision_layers, water.water.collision_mask),
                    ),
                    SceneNodeData::RigidBody2D(body) => (
                        BodyKind::Rigid,
                        body.enabled,
                        Some(RigidProps2D {
                            enabled: body.enabled,
                            can_sleep: body.can_sleep,
                            lock_rotation: body.lock_rotation,
                            mass: body.mass,
                            density: body.density,
                            continuous_collision_detection: body.continuous_collision_detection,
                            linear_velocity: body.linear_velocity,
                            angular_velocity: body.angular_velocity,
                            gravity_scale: body.gravity_scale,
                            linear_damping: body.linear_damping,
                            angular_damping: body.angular_damping,
                        }),
                        (body.friction, body.restitution, body.density),
                        (body.collision_layers, body.collision_mask),
                    ),
                    SceneNodeData::TileMap2D(tilemap) => (
                        BodyKind::Static,
                        tilemap.collision_enabled,
                        None,
                        (0.7, 0.0, 1.0),
                        (tilemap.collision_layers, tilemap.collision_mask),
                    ),
                    _ => continue,
                }
            };
            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            let tilemap_for_body = self.nodes.get(id).and_then(|node| match &node.data {
                SceneNodeData::TileMap2D(tilemap) => Some(tilemap.clone()),
                _ => None,
            });
            let mut shape_signature = body_signature_seed(kind);
            if let Some(tilemap) = tilemap_for_body.as_ref() {
                shape_signature = hash_tilemap_2d(shape_signature, tilemap);
                if let Some(tileset) =
                    crate::runtime::render_2d::resolve_tileset_2d(self, &tilemap.tileset)
                {
                    for tile in tileset.tiles.iter() {
                        if tile.collision {
                            shape_signature = hash_u64(shape_signature, tile.id as u64);
                            shape_signature = hash_tile_collision_shape_2d(
                                shape_signature,
                                tile.collision_shape.clone(),
                            );
                        }
                    }
                }
            } else if let Some(node) = self.nodes.get(id) {
                if let SceneNodeData::WaterBody2D(water) = &node.data {
                    shape_signature = hash_water_shape(shape_signature, water.water.shape);
                }
                for &child_id in node.children_slice() {
                    let Some(child) = self.nodes.get(child_id) else {
                        continue;
                    };
                    if let SceneNodeData::CollisionShape2D(shape) = &child.data {
                        shape_signature = hash_collision_shape_2d(shape_signature, shape, kind);
                    }
                }
            }
            shape_signature = hash_u32(shape_signature, groups.0.bits());
            shape_signature = hash_u32(shape_signature, groups.1.bits());
            shape_signature = hash_f32(shape_signature, material.2.to_bits());

            let needs_shape_rebuild = self
                .physics
                .world_2d
                .as_ref()
                .and_then(|world| world.body_map.get(&id))
                .map(|state| state.shape_signature != shape_signature)
                .unwrap_or(true);

            let mut shapes = Vec::new();
            if needs_shape_rebuild {
                if let Some(tilemap) = tilemap_for_body.as_ref() {
                    let tileset =
                        crate::runtime::render_2d::resolve_tileset_2d(self, &tilemap.tileset);
                    shapes.extend(tilemap_shape_descs_2d(
                        tilemap,
                        groups.0,
                        groups.1,
                        material.0,
                        material.1,
                        material.2,
                        tileset.as_ref(),
                    ));
                } else if let Some(node) = self.nodes.get(id) {
                    if let SceneNodeData::WaterBody2D(water) = &node.data {
                        let shape = water_shape_2d(water.water.shape);
                        shapes.push(ShapeDesc2D {
                            local: Transform2D::IDENTITY,
                            shape: ShapeKind2D::Primitive(shape),
                            sensor: true,
                            collision_layers: groups.0,
                            collision_mask: groups.1,
                            friction: material.0,
                            restitution: material.1,
                            density: material.2,
                        });
                    }
                    let child_count = node.children_slice().len();
                    if shapes.capacity() < child_count {
                        shapes.reserve(child_count - shapes.capacity());
                    }
                    for &child_id in node.children_slice() {
                        let Some(child) = self.nodes.get(child_id) else {
                            continue;
                        };
                        if let SceneNodeData::CollisionShape2D(shape) = &child.data {
                            let mut desc = shape_desc_2d(shape, material.0, material.1);
                            desc.sensor = kind == BodyKind::Area;
                            desc.collision_layers = groups.0;
                            desc.collision_mask = groups.1;
                            desc.density = material.2;
                            shapes.push(desc);
                        }
                    }
                }
            }

            out.push(BodyDesc2D {
                id,
                kind,
                enabled,
                global,
                rigid,
                shape_signature,
                shapes,
            });
        }
        out
    }

    fn collect_body_descs_3d(&mut self) -> Vec<BodyDesc3D> {
        let node_count = self.internal_updates.physics_body_nodes_3d.len();
        let mut out = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let id = self.internal_updates.physics_body_nodes_3d[i];
            let (kind, enabled, rigid, material, groups) = {
                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                match &node.data {
                    SceneNodeData::StaticBody3D(body) => (
                        BodyKind::Static,
                        body.enabled,
                        None,
                        (body.friction, body.restitution, body.density),
                        (body.collision_layers, body.collision_mask),
                    ),
                    SceneNodeData::Area3D(body) => (
                        BodyKind::Area,
                        body.enabled,
                        None,
                        (0.7, 0.0, 1.0),
                        (body.collision_layers, body.collision_mask),
                    ),
                    SceneNodeData::WaterBody3D(water) => (
                        BodyKind::Area,
                        water.visible,
                        None,
                        (0.7, 0.0, 1.0),
                        (water.water.collision_layers, water.water.collision_mask),
                    ),
                    SceneNodeData::RigidBody3D(body) => (
                        BodyKind::Rigid,
                        body.enabled,
                        Some(RigidProps3D {
                            enabled: body.enabled,
                            can_sleep: body.can_sleep,
                            mass: body.mass,
                            density: body.density,
                            continuous_collision_detection: body.continuous_collision_detection,
                            linear_velocity: body.linear_velocity,
                            angular_velocity: body.angular_velocity,
                            gravity_scale: body.gravity_scale,
                            linear_damping: body.linear_damping,
                            angular_damping: body.angular_damping,
                        }),
                        (body.friction, body.restitution, body.density),
                        (body.collision_layers, body.collision_mask),
                    ),
                    _ => continue,
                }
            };

            let Some(global) = self.get_global_transform_3d(id) else {
                continue;
            };
            let mut shape_signature = body_signature_seed(kind);
            shape_signature = hash_f32(shape_signature, global.scale.x.to_bits());
            shape_signature = hash_f32(shape_signature, global.scale.y.to_bits());
            shape_signature = hash_f32(shape_signature, global.scale.z.to_bits());
            shape_signature = hash_u32(shape_signature, groups.0.bits());
            shape_signature = hash_u32(shape_signature, groups.1.bits());
            shape_signature = hash_f32(shape_signature, material.2.to_bits());

            if let Some(node) = self.nodes.get(id) {
                if let SceneNodeData::WaterBody3D(water) = &node.data {
                    shape_signature = hash_water_shape(shape_signature, water.water.shape);
                    shape_signature = hash_f32(shape_signature, water.water.depth.to_bits());
                }
                for &child_id in node.children_slice() {
                    let Some(child) = self.nodes.get(child_id) else {
                        continue;
                    };
                    if let SceneNodeData::CollisionShape3D(shape) = &child.data {
                        shape_signature =
                            hash_collision_shape_3d(shape_signature, shape, kind, global.scale);
                    }
                }
            }

            let needs_shape_rebuild = self
                .physics
                .world_3d
                .as_ref()
                .and_then(|world| world.body_map.get(&id))
                .map(|state| state.shape_signature != shape_signature)
                .unwrap_or(true);

            let mut shapes = Vec::new();
            if needs_shape_rebuild && let Some(node) = self.nodes.get(id) {
                if let SceneNodeData::WaterBody3D(water) = &node.data {
                    let (shape, center_y) = water_shape_3d(water.water.shape, water.water.depth);
                    shapes.push(ShapeDesc3D {
                        local: Transform3D::new(
                            Vector3::new(0.0, center_y, 0.0),
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        ),
                        shape: ShapeKind3D::Primitive(shape),
                        sensor: true,
                        collision_layers: groups.0,
                        collision_mask: groups.1,
                        friction: material.0,
                        restitution: material.1,
                        density: material.2,
                    });
                }
                let child_count = node.children_slice().len();
                if shapes.capacity() < child_count {
                    shapes.reserve(child_count - shapes.capacity());
                }
                for &child_id in node.children_slice() {
                    let Some(child) = self.nodes.get(child_id) else {
                        continue;
                    };
                    if let SceneNodeData::CollisionShape3D(shape) = &child.data {
                        let mut desc = shape_desc_3d(shape, material.0, material.1);
                        // Physics colliders inherit parent body global scale.
                        desc.local.scale = Vector3::new(
                            desc.local.scale.x * global.scale.x,
                            desc.local.scale.y * global.scale.y,
                            desc.local.scale.z * global.scale.z,
                        );
                        desc.sensor = kind == BodyKind::Area;
                        desc.collision_layers = groups.0;
                        desc.collision_mask = groups.1;
                        desc.density = material.2;
                        shapes.push(desc);
                    }
                }
            }

            out.push(BodyDesc3D {
                id,
                kind,
                enabled,
                global,
                rigid,
                shape_signature,
                shapes,
            });
        }
        out
    }

    fn collect_joint_descs_2d(&self) -> Vec<JointDesc2D> {
        let mut out = Vec::new();
        for i in 0..self.internal_updates.internal_fixed_update_nodes.len() {
            let id = self.internal_updates.internal_fixed_update_nodes[i];
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let (body_a, body_b, anchor_a, anchor_b, enabled, collide_connected, kind) =
                match &node.data {
                    SceneNodeData::PinJoint2D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind2D::Pin,
                    ),
                    SceneNodeData::DistanceJoint2D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind2D::Distance {
                            min: joint.min_distance,
                            max: joint.max_distance,
                        },
                    ),
                    SceneNodeData::FixedJoint2D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind2D::Fixed,
                    ),
                    _ => continue,
                };
            let signature = joint_signature_2d(
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
            );
            out.push(JointDesc2D {
                id,
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
                signature,
            });
        }
        out
    }

    fn collect_joint_descs_3d(&self) -> Vec<JointDesc3D> {
        let mut out = Vec::new();
        for i in 0..self.internal_updates.internal_fixed_update_nodes.len() {
            let id = self.internal_updates.internal_fixed_update_nodes[i];
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let (body_a, body_b, anchor_a, anchor_b, enabled, collide_connected, kind) =
                match &node.data {
                    SceneNodeData::BallJoint3D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind3D::Ball,
                    ),
                    SceneNodeData::HingeJoint3D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind3D::Hinge { axis: joint.axis },
                    ),
                    SceneNodeData::FixedJoint3D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind3D::Fixed,
                    ),
                    _ => continue,
                };
            let signature = joint_signature_3d(
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
            );
            out.push(JointDesc3D {
                id,
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
                signature,
            });
        }
        out
    }

    fn sync_world_2d(&mut self, bodies: &[BodyDesc2D]) {
        let mut handle_updates = Vec::new();
        self.physics
            .sync_world_2d(bodies, |id, handle| handle_updates.push((id, handle)));
        for (id, handle) in handle_updates {
            self.set_body_handle_2d(id, handle);
        }
    }

    fn sync_world_3d(&mut self, bodies: &[BodyDesc3D]) {
        let provider_mode = match self.provider_mode {
            crate::runtime_project::ProviderMode::Dynamic => PhysicsProviderMode::Dynamic,
            crate::runtime_project::ProviderMode::Static => PhysicsProviderMode::Static,
        };
        let assets = PhysicsAssetContext {
            provider_mode,
            static_mesh_lookup: self
                .project()
                .and_then(|project| project.static_mesh_lookup),
            static_collision_trimesh_lookup: self
                .project()
                .and_then(|project| project.static_collision_trimesh_lookup),
        };
        let mut handle_updates = Vec::new();
        self.physics.sync_world_3d(bodies, assets, |id, handle| {
            handle_updates.push((id, handle));
        });
        for (id, handle) in handle_updates {
            self.set_body_handle_3d(id, handle);
        }
    }

    fn sync_joints_2d(&mut self, joints: &[JointDesc2D]) {
        self.physics.sync_joints_2d(joints);
    }

    fn sync_joints_3d(&mut self, joints: &[JointDesc3D]) {
        self.physics.sync_joints_3d(joints);
    }

    fn step_world_2d(&mut self) {
        self.physics
            .step_world_2d(self.physics_gravity(), self.time.fixed_delta);
    }

    fn step_world_3d(&mut self) {
        self.physics
            .step_world_3d(self.physics_gravity(), self.time.fixed_delta);
    }

    fn apply_pending_impulses_2d(&mut self) {
        self.physics.apply_pending_impulses_2d(self.physics_coef());
    }

    fn apply_pending_forces_2d(&mut self) {
        self.physics
            .apply_pending_forces_2d(self.physics_coef(), self.time.fixed_delta);
    }

    fn apply_pending_impulses_3d(&mut self) {
        self.physics.apply_pending_impulses_3d(self.physics_coef());
    }

    fn apply_pending_forces_3d(&mut self) {
        self.physics
            .apply_pending_forces_3d(self.physics_coef(), self.time.fixed_delta);
    }

    fn queue_physics_force_emitters_2d(&mut self) {
        self.force_water_impacts_2d.clear();
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::PhysicsForceEmitter2D(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        let mut emitters = Vec::new();
        for id in ids {
            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            let Some(node) = self.nodes.get_mut(id) else {
                continue;
            };
            let SceneNodeData::PhysicsForceEmitter2D(emitter) = &mut node.data else {
                continue;
            };
            if force_emitter_active(
                emitter.enabled,
                emitter.pulse,
                emitter.duration,
                emitter.age,
            ) {
                emitters.push((global.position, emitter.clone()));
            }
            emitter.age += self.time.fixed_delta.max(0.0);
        }
        emitters.extend(
            self.pending_force_emitters_2d
                .drain(..)
                .map(|emitter| (emitter.transform.position, emitter)),
        );
        for (position, emitter) in emitters {
            self.apply_force_emitter_2d(position, &emitter);
        }
    }

    fn queue_physics_force_emitters_3d(&mut self) {
        self.force_water_impacts_3d.clear();
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::PhysicsForceEmitter3D(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        let mut emitters = Vec::new();
        for id in ids {
            let Some(global) = self.get_global_transform_3d(id) else {
                continue;
            };
            let Some(node) = self.nodes.get_mut(id) else {
                continue;
            };
            let SceneNodeData::PhysicsForceEmitter3D(emitter) = &mut node.data else {
                continue;
            };
            if force_emitter_active(
                emitter.enabled,
                emitter.pulse,
                emitter.duration,
                emitter.age,
            ) {
                emitters.push((global.position, emitter.clone()));
            }
            emitter.age += self.time.fixed_delta.max(0.0);
        }
        emitters.extend(
            self.pending_force_emitters_3d
                .drain(..)
                .map(|emitter| (emitter.transform.position, emitter)),
        );
        for (position, emitter) in emitters {
            self.apply_force_emitter_3d(position, &emitter);
        }
    }

    fn apply_force_emitter_2d(
        &mut self,
        emitter_pos: Vector2,
        emitter: &perro_nodes::PhysicsForceEmitter2D,
    ) {
        if emitter.radius <= 0.0 {
            return;
        }
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::RigidBody2D(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            let Some((layers, mask)) = self.nodes.get(id).and_then(|node| {
                let SceneNodeData::RigidBody2D(body) = &node.data else {
                    return None;
                };
                Some((body.collision_layers, body.collision_mask))
            }) else {
                continue;
            };
            if !emitter.affect_bodies
                || emitter.collision_mask.intersects(layers)
                || mask.intersects(emitter.collision_layers)
            {
                continue;
            }
            let offset = global.position - emitter_pos;
            let dist = offset.length();
            if dist > emitter.radius {
                continue;
            }
            let force = force_emitter_force_2d(emitter, offset, dist);
            if force.length_squared() <= 0.000_001 {
                continue;
            }
            if emitter.pulse || emitter.profile == perro_nodes::PhysicsForceProfile::Explosion {
                self.physics.queue_impulse_2d(id, force);
            } else {
                self.physics.queue_force_2d(id, force);
            }
        }
        if emitter.affect_water {
            self.queue_force_water_impacts_2d(emitter_pos, emitter);
        }
    }

    fn apply_force_emitter_3d(
        &mut self,
        emitter_pos: Vector3,
        emitter: &perro_nodes::PhysicsForceEmitter3D,
    ) {
        if emitter.radius <= 0.0 {
            return;
        }
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::RigidBody3D(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            let Some(global) = self.get_global_transform_3d(id) else {
                continue;
            };
            let Some((layers, mask)) = self.nodes.get(id).and_then(|node| {
                let SceneNodeData::RigidBody3D(body) = &node.data else {
                    return None;
                };
                Some((body.collision_layers, body.collision_mask))
            }) else {
                continue;
            };
            if !emitter.affect_bodies
                || emitter.collision_mask.intersects(layers)
                || mask.intersects(emitter.collision_layers)
            {
                continue;
            }
            let offset = global.position - emitter_pos;
            let dist = offset.length();
            if dist > emitter.radius {
                continue;
            }
            let force = force_emitter_force_3d(emitter, offset, dist);
            if force.length_squared() <= 0.000_001 {
                continue;
            }
            if emitter.pulse || emitter.profile == perro_nodes::PhysicsForceProfile::Explosion {
                self.physics.queue_impulse_3d(id, force);
            } else {
                self.physics.queue_force_3d(id, force);
            }
        }
        if emitter.affect_water {
            self.queue_force_water_impacts_3d(emitter_pos, emitter);
        }
    }

    fn queue_force_water_impacts_2d(
        &mut self,
        emitter_pos: Vector2,
        emitter: &perro_nodes::PhysicsForceEmitter2D,
    ) {
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::WaterBody2D(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            let Some(water) = self.nodes.get(id).and_then(|node| {
                let SceneNodeData::WaterBody2D(water) = &node.data else {
                    return None;
                };
                Some(water.water)
            }) else {
                continue;
            };
            if emitter.collision_mask.intersects(water.collision_layers)
                || water.collision_mask.intersects(emitter.collision_layers)
            {
                continue;
            }
            let local = emitter_pos - global.position;
            let half = water.shape.surface_size() * 0.5;
            if local.x.abs() > half.x + emitter.radius || local.y.abs() > half.y + emitter.radius {
                continue;
            }
            let dist = local.length().min(emitter.radius);
            let force = force_emitter_force_2d(emitter, local, dist);
            let strength = force.length().min(512.0);
            if strength <= 0.0 {
                continue;
            }
            self.force_water_impacts_2d
                .push(crate::runtime::ForceWaterImpact2D {
                    position: emitter_pos,
                    force,
                    strength,
                    radius: emitter.radius.max(0.001),
                    cavitation: if water.shape.contains_surface(local) {
                        (strength / 256.0).clamp(0.0, 1.0)
                    } else {
                        0.0
                    },
                });
            self.mark_needs_rerender(id);
        }
    }

    fn queue_force_water_impacts_3d(
        &mut self,
        emitter_pos: Vector3,
        emitter: &perro_nodes::PhysicsForceEmitter3D,
    ) {
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::WaterBody3D(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        for id in ids {
            let Some(global) = self.get_global_transform_3d(id) else {
                continue;
            };
            let Some(water) = self.nodes.get(id).and_then(|node| {
                let SceneNodeData::WaterBody3D(water) = &node.data else {
                    return None;
                };
                Some(water.water)
            }) else {
                continue;
            };
            if emitter.collision_mask.intersects(water.collision_layers)
                || water.collision_mask.intersects(emitter.collision_layers)
            {
                continue;
            }
            let local = emitter_pos - global.position;
            let half = water.shape.surface_size() * 0.5;
            if local.x.abs() > half.x + emitter.radius
                || local.z.abs() > half.y + emitter.radius
                || emitter_pos.y > global.position.y + emitter.radius
                || emitter_pos.y
                    < global.position.y - water.shape.depth(water.depth) - emitter.radius
            {
                continue;
            }
            let dist = Vector2::new(local.x, local.z).length().min(emitter.radius);
            let force = force_emitter_force_3d(emitter, local, dist);
            let strength = force.length().min(512.0);
            if strength <= 0.0 {
                continue;
            }
            self.force_water_impacts_3d
                .push(crate::runtime::ForceWaterImpact3D {
                    position: emitter_pos,
                    force,
                    strength,
                    radius: emitter.radius.max(0.001),
                    cavitation: if water.shape.contains_surface(Vector2::new(local.x, local.z))
                        && emitter_pos.y <= global.position.y
                        && emitter_pos.y >= global.position.y - water.shape.depth(water.depth)
                    {
                        (strength / 256.0).clamp(0.0, 1.0)
                    } else {
                        0.0
                    },
                });
            self.mark_needs_rerender(id);
        }
    }

    fn queue_water_forces_2d(&mut self) {
        self.pending_water_queries_2d.clear();
        self.water_contacts_2d.clear();
        let water_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, scene_node)| {
                matches!(scene_node.data, SceneNodeData::WaterBody2D(_)).then_some(id)
            })
            .collect();
        let mut waters = Vec::new();
        for &id in water_ids.iter() {
            let Some(transform) = self.get_global_transform_2d(id) else {
                continue;
            };
            let Some(scene_node) = self.nodes.get(id) else {
                continue;
            };
            let SceneNodeData::WaterBody2D(water) = &scene_node.data else {
                continue;
            };
            let transform_mat = transform.to_mat3();
            let inv_transform = transform_mat.inverse();
            let half = water.water.shape.surface_size() * 0.5;
            let (min_x, max_x) = water_world_x_bounds_2d(transform_mat, half);
            waters.push(RuntimeWater2D {
                id,
                half,
                transform: transform_mat,
                inv_transform,
                normal: water_normal_2d(transform_mat),
                min_x,
                max_x,
                surface: water.water,
            });
        }
        if waters.is_empty() {
            return;
        }
        let water_index = RuntimeWaterIndex2D::new(waters);
        let camera_pos = self
            .render_2d
            .last_camera
            .as_ref()
            .map(|camera| Vector2::new(camera.position[0], camera.position[1]))
            .unwrap_or(Vector2::ZERO);

        let body_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, scene_node)| {
                matches!(scene_node.data, SceneNodeData::RigidBody2D(_)).then_some(id)
            })
            .collect();
        let mut bodies = Vec::with_capacity(body_ids.len());
        for body_id in body_ids {
            let Some(body_transform) = self.get_global_transform_2d(body_id) else {
                continue;
            };
            let Some((velocity, mass, density, collision_layers, collision_mask)) =
                self.nodes.get(body_id).and_then(|scene_node| {
                    let SceneNodeData::RigidBody2D(body) = &scene_node.data else {
                        return None;
                    };
                    Some((
                        body.linear_velocity,
                        body.mass,
                        body.density,
                        body.collision_layers,
                        body.collision_mask,
                    ))
                })
            else {
                continue;
            };
            bodies.push(RuntimeWaterBody2D {
                id: body_id,
                pos: body_transform.position,
                velocity,
                mass,
                density,
                float_radius: self.body_float_radius_2d(body_id, body_transform.position),
                collision_layers,
                collision_mask,
            });
        }
        let elapsed = self.time.elapsed;
        let splash_impacts =
            water_body_splashes_2d(&bodies, &water_index, &self.water_body_samples, elapsed);
        self.register_water_queries_2d(&bodies, &water_index);
        self.record_water_contacts_2d(&bodies, &water_index, elapsed);
        let water_samples = &self.water_samples;
        let forces: Vec<_> = if bodies.len() >= WATER_FORCE_PAR_BODY_THRESHOLD {
            bodies
                .par_iter()
                .flat_map_iter(|body| {
                    water_forces_for_body_2d(
                        *body,
                        &water_index,
                        water_samples,
                        &self.water_body_samples,
                        elapsed,
                        camera_pos,
                    )
                })
                .collect()
        } else {
            bodies
                .iter()
                .flat_map(|body| {
                    water_forces_for_body_2d(
                        *body,
                        &water_index,
                        water_samples,
                        &self.water_body_samples,
                        elapsed,
                        camera_pos,
                    )
                })
                .collect()
        };
        for (body, force) in forces {
            self.physics.queue_force_2d(body, force);
            self.apply_water_angular_nudge_2d(body, force.x * 0.04);
        }
        if !splash_impacts.is_empty() {
            self.force_water_impacts_2d.extend(splash_impacts);
            for id in water_ids {
                self.mark_needs_rerender(id);
            }
        }
    }

    fn queue_water_forces_3d(&mut self) {
        self.pending_water_queries_3d.clear();
        self.water_contacts_3d.clear();
        let water_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, scene_node)| {
                matches!(scene_node.data, SceneNodeData::WaterBody3D(_)).then_some(id)
            })
            .collect();
        let mut waters = Vec::new();
        for &id in water_ids.iter() {
            let Some(transform) = self.get_global_transform_3d(id) else {
                continue;
            };
            let Some(scene_node) = self.nodes.get(id) else {
                continue;
            };
            let SceneNodeData::WaterBody3D(water) = &scene_node.data else {
                continue;
            };
            let transform_mat = transform.to_mat4();
            let inv_transform = transform_mat.inverse();
            let half = water.water.shape.surface_size() * 0.5;
            let (min_x, max_x) = water_world_x_bounds_3d(
                transform_mat,
                half,
                water.water.shape.depth(water.water.depth),
            );
            waters.push(RuntimeWater3D {
                id,
                half,
                transform: transform_mat,
                inv_transform,
                normal: water_normal_3d(transform_mat),
                min_x,
                max_x,
                surface: water.water,
            });
        }
        if waters.is_empty() {
            return;
        }
        let water_index = RuntimeWaterIndex3D::new(waters);
        let camera_pos = self
            .render_3d
            .last_camera
            .as_ref()
            .map(|camera| Vector2::new(camera.position[0], camera.position[2]))
            .unwrap_or(Vector2::ZERO);

        let body_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, scene_node)| {
                matches!(scene_node.data, SceneNodeData::RigidBody3D(_)).then_some(id)
            })
            .collect();
        let mut bodies = Vec::with_capacity(body_ids.len());
        for body_id in body_ids {
            let Some(body_transform) = self.get_global_transform_3d(body_id) else {
                continue;
            };
            let Some((velocity, mass, density, collision_layers, collision_mask)) =
                self.nodes.get(body_id).and_then(|scene_node| {
                    let SceneNodeData::RigidBody3D(body) = &scene_node.data else {
                        return None;
                    };
                    Some((
                        body.linear_velocity,
                        body.mass,
                        body.density,
                        body.collision_layers,
                        body.collision_mask,
                    ))
                })
            else {
                continue;
            };
            bodies.push(RuntimeWaterBody3D {
                id: body_id,
                pos: body_transform.position,
                velocity,
                mass,
                density,
                float_radius: self.body_float_radius_3d(body_id, body_transform.position),
                collision_layers,
                collision_mask,
            });
        }
        let elapsed = self.time.elapsed;
        let splash_impacts =
            water_body_splashes_3d(&bodies, &water_index, &self.water_body_samples, elapsed);
        self.register_water_queries_3d(&bodies, &water_index);
        self.record_water_contacts_3d(&bodies, &water_index, elapsed);
        let water_samples = &self.water_samples;
        let forces: Vec<_> = if bodies.len() >= WATER_FORCE_PAR_BODY_THRESHOLD {
            bodies
                .par_iter()
                .flat_map_iter(|body| {
                    water_forces_for_body_3d(
                        *body,
                        &water_index,
                        water_samples,
                        &self.water_body_samples,
                        elapsed,
                        camera_pos,
                    )
                })
                .collect()
        } else {
            bodies
                .iter()
                .flat_map(|body| {
                    water_forces_for_body_3d(
                        *body,
                        &water_index,
                        water_samples,
                        &self.water_body_samples,
                        elapsed,
                        camera_pos,
                    )
                })
                .collect()
        };
        for (body, force) in forces {
            self.physics.queue_force_3d(body, force);
            self.apply_water_angular_nudge_3d(
                body,
                Vector3::new(force.z * 0.025, 0.0, -force.x * 0.025),
            );
        }
        if !splash_impacts.is_empty() {
            self.force_water_impacts_3d.extend(splash_impacts);
            for id in water_ids {
                self.mark_needs_rerender(id);
            }
        }
    }

    fn apply_water_angular_nudge_2d(&mut self, id: NodeID, delta: f32) {
        if delta.abs() <= 0.000_1 {
            return;
        }
        let Some(world) = self.physics.world_2d.as_mut() else {
            return;
        };
        let Some(state) = world.body_map.get(&id) else {
            return;
        };
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            return;
        };
        let target = (rb.angvel() + delta).clamp(-1.75, 1.75);
        rb.set_angvel(target, true);
    }

    fn body_float_radius_2d(&mut self, body: NodeID, body_pos: Vector2) -> f32 {
        let child_ids = self
            .nodes
            .get(body)
            .map(|node| node.children_slice().to_vec())
            .unwrap_or_default();
        let mut radius = 0.0f32;
        for child_id in child_ids {
            let Some(shape) = self.nodes.get(child_id).and_then(|child| {
                let SceneNodeData::CollisionShape2D(shape) = &child.data else {
                    return None;
                };
                Some(shape.shape)
            }) else {
                continue;
            };
            let Some(global) = self.get_global_transform_2d(child_id) else {
                continue;
            };
            let half_y = match shape {
                Shape2D::Quad { height, .. } | Shape2D::Triangle { height, .. } => {
                    height.abs() * global.scale.y.abs() * 0.5
                }
                Shape2D::Circle { radius } => radius.abs() * global.scale.y.abs(),
            };
            radius = radius.max((global.position.y - body_pos.y).abs() + half_y);
        }
        radius
    }

    fn body_float_radius_3d(&mut self, body: NodeID, body_pos: Vector3) -> f32 {
        let child_ids = self
            .nodes
            .get(body)
            .map(|node| node.children_slice().to_vec())
            .unwrap_or_default();
        let mut radius = 0.0f32;
        for child_id in child_ids {
            let Some(shape) = self.nodes.get(child_id).and_then(|child| {
                let SceneNodeData::CollisionShape3D(shape) = &child.data else {
                    return None;
                };
                Some(shape.shape.clone())
            }) else {
                continue;
            };
            let Some(global) = self.get_global_transform_3d(child_id) else {
                continue;
            };
            let half_y = match shape {
                Shape3D::Cube { size }
                | Shape3D::TriPrism { size }
                | Shape3D::TriangularPyramid { size }
                | Shape3D::SquarePyramid { size } => size.y.abs() * global.scale.y.abs() * 0.5,
                Shape3D::Sphere { radius } => radius.abs() * global.scale.y.abs(),
                Shape3D::Capsule {
                    radius,
                    half_height,
                } => (radius.abs() + half_height.abs()) * global.scale.y.abs(),
                Shape3D::Cylinder { half_height, .. } | Shape3D::Cone { half_height, .. } => {
                    half_height.abs() * global.scale.y.abs()
                }
                Shape3D::TriMesh { .. } => 0.0,
            };
            radius = radius.max((global.position.y - body_pos.y).abs() + half_y);
        }
        radius
    }

    fn apply_water_angular_nudge_3d(&mut self, id: NodeID, delta: Vector3) {
        if delta.length_squared() <= 0.000_001 {
            return;
        }
        let Some(world) = self.physics.world_3d.as_mut() else {
            return;
        };
        let Some(state) = world.body_map.get(&id) else {
            return;
        };
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            return;
        };
        let current = rb.angvel();
        let target = na3::Vector3::new(
            (current.x + delta.x).clamp(-1.4, 1.4),
            (current.y + delta.y).clamp(-1.4, 1.4),
            (current.z + delta.z).clamp(-1.4, 1.4),
        );
        rb.set_angvel(target, true);
    }

    fn register_water_queries_2d(
        &mut self,
        bodies: &[RuntimeWaterBody2D],
        water_index: &RuntimeWaterIndex2D,
    ) {
        for body in bodies {
            let radius = body.float_radius.max(0.5);
            for (point, pos) in [
                (0u8, body.pos),
                (1u8, body.pos + Vector2::new(-radius * 0.75, 0.0)),
                (2u8, body.pos + Vector2::new(radius * 0.75, 0.0)),
            ] {
                register_water_query_candidates_2d(
                    &mut self.pending_water_queries_2d,
                    water_index,
                    *body,
                    point,
                    pos,
                );
            }
        }
    }

    fn register_water_queries_3d(
        &mut self,
        bodies: &[RuntimeWaterBody3D],
        water_index: &RuntimeWaterIndex3D,
    ) {
        for body in bodies {
            let radius = body.float_radius.max(0.5);
            for (point, pos) in [
                (0u8, body.pos),
                (1u8, body.pos + Vector3::new(-radius * 0.75, 0.0, 0.0)),
                (2u8, body.pos + Vector3::new(radius * 0.75, 0.0, 0.0)),
            ] {
                register_water_query_candidates_3d(
                    &mut self.pending_water_queries_3d,
                    water_index,
                    *body,
                    point,
                    pos,
                );
            }
        }
    }

    fn record_water_contacts_2d(
        &mut self,
        bodies: &[RuntimeWaterBody2D],
        water_index: &RuntimeWaterIndex2D,
        elapsed: f32,
    ) {
        let empty_samples = AHashMap::new();
        for body in bodies {
            for sample in blended_water_samples_2d(
                body.pos,
                body.collision_layers,
                body.collision_mask,
                water_index,
                &empty_samples,
                &self.water_body_samples,
                body.id,
                0,
                elapsed,
            ) {
                if sample.submerged <= 0.0 {
                    continue;
                }
                if let Some(water_id) = sample_water_id_2d(body.pos, water_index, sample.pos) {
                    self.water_contacts_2d
                        .entry(water_id)
                        .or_default()
                        .push(crate::runtime::WaterBodyContact2D {
                        body: body.id,
                        position: sample.pos,
                        velocity: body.velocity,
                        radius: body.float_radius.max(0.75),
                        foam_amount: (sample.sample.foam + body.velocity.length() * 0.06)
                            .clamp(0.1, 1.0),
                        persist: 0.22,
                    });
                }
            }
        }
    }

    fn record_water_contacts_3d(
        &mut self,
        bodies: &[RuntimeWaterBody3D],
        water_index: &RuntimeWaterIndex3D,
        elapsed: f32,
    ) {
        let empty_samples = AHashMap::new();
        for body in bodies {
            for sample in blended_water_samples_3d(
                body.pos,
                body.collision_layers,
                body.collision_mask,
                water_index,
                &empty_samples,
                &self.water_body_samples,
                body.id,
                0,
                elapsed,
            ) {
                if sample.submerged <= 0.0 {
                    continue;
                }
                if let Some(water_id) = sample_water_id_3d(body.pos, water_index, sample.pos) {
                    self.water_contacts_3d
                        .entry(water_id)
                        .or_default()
                        .push(crate::runtime::WaterBodyContact3D {
                        body: body.id,
                        position: sample.pos,
                        velocity: body.velocity,
                        radius: body.float_radius.max(0.75),
                        foam_amount: (sample.sample.foam
                            + Vector2::new(body.velocity.x, body.velocity.z).length() * 0.05)
                            .clamp(0.1, 1.0),
                        persist: 0.26,
                    });
                }
            }
        }
    }

    fn sync_world_to_nodes_2d(&mut self) {
        let Some(world) = self.physics.world_2d.take() else {
            return;
        };

        for (&id, state) in &world.body_map {
            self.set_body_handle_2d(id, Some(state.opaque_handle));
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(body) = world.bodies.get(state.handle) else {
                continue;
            };
            let position = Vector2::new(body.translation().x, body.translation().y);
            let rotation = body.rotation().angle();
            let lin = Vector2::new(body.linvel().x, body.linvel().y);
            let ang = body.angvel();

            let mut target = self
                .get_global_transform_2d(id)
                .unwrap_or(Transform2D::IDENTITY);
            target.position = position;
            target.rotation = rotation;
            let _ = NodeAPI::set_global_transform_2d(self, id, target);

            if let Some(scene_node) = self.nodes.get_mut(id)
                && let SceneNodeData::RigidBody2D(node) = &mut scene_node.data
            {
                node.linear_velocity = lin;
                node.angular_velocity = ang;
            }
        }

        self.physics.world_2d = Some(world);
    }

    fn sync_world_to_nodes_3d(&mut self) {
        let Some(world) = self.physics.world_3d.take() else {
            return;
        };

        for (&id, state) in &world.body_map {
            self.set_body_handle_3d(id, Some(state.opaque_handle));
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(body) = world.bodies.get(state.handle) else {
                continue;
            };
            let position = Vector3::new(
                body.translation().x,
                body.translation().y,
                body.translation().z,
            );
            let rot = body.rotation();
            let rotation = Quaternion::new(rot.i, rot.j, rot.k, rot.w);
            let lin = Vector3::new(body.linvel().x, body.linvel().y, body.linvel().z);
            let ang = Vector3::new(body.angvel().x, body.angvel().y, body.angvel().z);

            let mut target = self
                .get_global_transform_3d(id)
                .unwrap_or(Transform3D::IDENTITY);
            target.position = position;
            target.rotation = rotation;
            let _ = NodeAPI::set_global_transform_3d(self, id, target);

            if let Some(scene_node) = self.nodes.get_mut(id)
                && let SceneNodeData::RigidBody3D(node) = &mut scene_node.data
            {
                node.linear_velocity = lin;
                node.angular_velocity = ang;
            }
        }

        self.physics.world_3d = Some(world);
    }

    fn set_body_handle_2d(&mut self, id: NodeID, handle: Option<u64>) {
        if let Some(node) = self.nodes.get_mut(id) {
            match &mut node.data {
                SceneNodeData::StaticBody2D(body) => body.physics_handle = handle,
                SceneNodeData::Area2D(body) => body.physics_handle = handle,
                SceneNodeData::RigidBody2D(body) => body.physics_handle = handle,
                _ => {}
            }
        }
    }

    fn set_body_handle_3d(&mut self, id: NodeID, handle: Option<u64>) {
        if let Some(node) = self.nodes.get_mut(id) {
            match &mut node.data {
                SceneNodeData::StaticBody3D(body) => body.physics_handle = handle,
                SceneNodeData::Area3D(body) => body.physics_handle = handle,
                SceneNodeData::RigidBody3D(body) => body.physics_handle = handle,
                _ => {}
            }
        }
    }

    fn physics_gravity(&self) -> f32 {
        self.project()
            .map(|p| p.config.physics_gravity)
            .filter(|v| v.is_finite())
            .unwrap_or(-9.81)
            * self.physics_coef()
    }

    fn physics_coef(&self) -> f32 {
        self.project()
            .map(|p| p.config.physics_coef)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0)
    }

    fn emit_collision_signals_2d(&mut self) {
        let Some(world) = self.physics.world_2d.as_ref() else {
            self.physics.active_collision_pairs_2d.clear();
            return;
        };
        let mut current_pairs = AHashSet::default();
        let mut entered_pairs = Vec::new();

        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            if a == b {
                continue;
            }

            let key = BodyPair::sorted(a, b);
            current_pairs.insert(key);
            if !self.physics.active_collision_pairs_2d.contains(&key) {
                entered_pairs.push(key);
            }
        }

        self.physics.active_collision_pairs_2d = current_pairs;
        self.emit_collision_signals_for_pairs(&entered_pairs);
    }

    fn emit_collision_signals_3d(&mut self) {
        let Some(world) = self.physics.world_3d.as_ref() else {
            self.physics.active_collision_pairs_3d.clear();
            return;
        };
        let mut current_pairs = AHashSet::default();
        let mut entered_pairs = Vec::new();

        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            if a == b {
                continue;
            }

            let key = BodyPair::sorted(a, b);
            current_pairs.insert(key);
            if !self.physics.active_collision_pairs_3d.contains(&key) {
                entered_pairs.push(key);
            }
        }

        self.physics.active_collision_pairs_3d = current_pairs;
        self.emit_collision_signals_for_pairs(&entered_pairs);
    }

    fn emit_collision_signals_for_pairs(&mut self, pairs: &[BodyPair]) {
        for pair in pairs {
            self.emit_collision_signal_for_node(pair.a, pair.b);
            self.emit_collision_signal_for_node(pair.b, pair.a);
        }
    }

    fn emit_collision_signal_for_node(&mut self, source: NodeID, other: NodeID) {
        let signal_id = {
            let Some(node) = self.nodes.get(source) else {
                return;
            };
            if node.name.is_empty() {
                return;
            }
            self.physics.signal_name_scratch.clear();
            self.physics
                .signal_name_scratch
                .push_str(node.name.as_ref());
            self.physics.signal_name_scratch.push_str("_Collided");
            SignalID::from_string(&self.physics.signal_name_scratch)
        };

        let params = [Variant::from(source), Variant::from(other)];
        let _ = SignalAPI::signal_emit(self, signal_id, &params);
    }

    fn emit_area_signals_2d(&mut self) {
        let Some(world) = self.physics.world_2d.as_ref() else {
            self.physics.active_area_overlaps_2d.clear();
            return;
        };
        let mut current = AHashSet::default();

        for (collider_a, collider_b, intersecting) in world.narrow_phase.intersection_pairs() {
            if !intersecting {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&collider_a) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&collider_b) else {
                continue;
            };
            if a == b {
                continue;
            }

            let kind_a = world.body_map.get(&a).map(|state| state.kind);
            let kind_b = world.body_map.get(&b).map(|state| state.kind);

            if kind_a == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: a, other: b });
            }
            if kind_b == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: b, other: a });
            }
        }

        self.emit_area_overlap_signals(current, true);
    }

    fn emit_area_signals_3d(&mut self) {
        let Some(world) = self.physics.world_3d.as_ref() else {
            self.physics.active_area_overlaps_3d.clear();
            return;
        };
        let mut current = AHashSet::default();

        for (collider_a, collider_b, intersecting) in world.narrow_phase.intersection_pairs() {
            if !intersecting {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&collider_a) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&collider_b) else {
                continue;
            };
            if a == b {
                continue;
            }

            let kind_a = world.body_map.get(&a).map(|state| state.kind);
            let kind_b = world.body_map.get(&b).map(|state| state.kind);

            if kind_a == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: a, other: b });
            }
            if kind_b == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: b, other: a });
            }
        }

        self.emit_area_overlap_signals(current, false);
    }

    fn emit_area_overlap_signals(&mut self, current: AHashSet<AreaOverlap>, is_2d: bool) {
        let previous = if is_2d {
            std::mem::take(&mut self.physics.active_area_overlaps_2d)
        } else {
            std::mem::take(&mut self.physics.active_area_overlaps_3d)
        };

        for overlap in current.iter().copied() {
            if !previous.contains(&overlap) {
                self.emit_area_signal(overlap.area, overlap.other, "Entered");
            }
            self.emit_area_signal(overlap.area, overlap.other, "Occupied");
        }

        for overlap in previous.iter().copied() {
            if !current.contains(&overlap) {
                self.emit_area_signal(overlap.area, overlap.other, "Exited");
            }
        }

        if is_2d {
            self.physics.active_area_overlaps_2d = current;
        } else {
            self.physics.active_area_overlaps_3d = current;
        }
    }

    fn emit_area_signal(&mut self, area: NodeID, other: NodeID, action: &str) {
        let signal_id = {
            let Some(node) = self.nodes.get(area) else {
                return;
            };
            if node.name.is_empty() {
                return;
            }
            self.physics.signal_name_scratch.clear();
            self.physics
                .signal_name_scratch
                .push_str(node.name.as_ref());
            self.physics.signal_name_scratch.push('_');
            self.physics.signal_name_scratch.push_str(action);
            SignalID::from_string(&self.physics.signal_name_scratch)
        };

        let params = [Variant::from(area), Variant::from(other)];
        let _ = SignalAPI::signal_emit(self, signal_id, &params);
    }
}

fn water_shape_2d(shape: WaterShape) -> Shape2D {
    match shape {
        WaterShape::Circle { radius } | WaterShape::Cylinder { radius, .. } => {
            Shape2D::Circle { radius }
        }
        WaterShape::Rect { size } => Shape2D::Quad {
            width: size.x,
            height: size.y,
        },
        WaterShape::Box { size } => Shape2D::Quad {
            width: size.x,
            height: size.z,
        },
    }
}

fn blend_water_candidates_2d(candidates: Vec<WaterCandidate2D>) -> Vec<BlendedWaterSample2D> {
    if candidates.len() <= 1 {
        return candidates
            .into_iter()
            .map(blended_water_sample_2d)
            .collect();
    }
    let mut used = vec![false; candidates.len()];
    let mut out = Vec::new();
    for start in 0..candidates.len() {
        if used[start] {
            continue;
        }
        let mut group = vec![start];
        used[start] = true;
        let mut cursor = 0;
        while cursor < group.len() {
            let a = group[cursor];
            for b in 0..candidates.len() {
                if used[b] {
                    continue;
                }
                if water_linked_2d(candidates[a].water, candidates[b].water) {
                    used[b] = true;
                    group.push(b);
                }
            }
            cursor += 1;
        }
        out.push(blend_water_group_2d(&candidates, &group));
    }
    out
}

fn blend_water_candidates_3d(candidates: Vec<WaterCandidate3D>) -> Vec<BlendedWaterSample3D> {
    if candidates.len() <= 1 {
        return candidates
            .into_iter()
            .map(blended_water_sample_3d)
            .collect();
    }
    let mut used = vec![false; candidates.len()];
    let mut out = Vec::new();
    for start in 0..candidates.len() {
        if used[start] {
            continue;
        }
        let mut group = vec![start];
        used[start] = true;
        let mut cursor = 0;
        while cursor < group.len() {
            let a = group[cursor];
            for b in 0..candidates.len() {
                if used[b] {
                    continue;
                }
                if water_linked_3d(candidates[a].water, candidates[b].water) {
                    used[b] = true;
                    group.push(b);
                }
            }
            cursor += 1;
        }
        out.push(blend_water_group_3d(&candidates, &group));
    }
    out
}

fn water_forces_for_body_2d(
    body: RuntimeWaterBody2D,
    water_index: &RuntimeWaterIndex2D,
    water_samples: &AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    elapsed: f32,
    camera_pos: Vector2,
) -> Vec<(NodeID, Vector2)> {
    let samples = blended_water_samples_2d(
        body.pos,
        body.collision_layers,
        body.collision_mask,
        water_index,
        water_samples,
        water_body_samples,
        body.id,
        0,
        elapsed,
    );
    let mut forces = Vec::with_capacity(samples.len());
    for blend in samples {
        let float_radius = body.float_radius.max(0.0);
        let submerged = (blend.submerged + float_radius).max(0.0);
        if submerged <= 0.0 {
            continue;
        }
        let mass = body.mass.max(0.001);
        let target_submerged = (float_radius * body.density.clamp(0.05, 1.2))
            .max(water_target_submerged(body.density));
        let contact = (submerged / target_submerged.max(0.001)).clamp(0.0, 1.5);
        let support = mass * 9.81 * (submerged / target_submerged.max(0.001)).clamp(0.0, 1.25);
        let spring = (submerged - target_submerged) * blend.surface.physics.buoyancy * mass * 8.0;
        let rel_y = body.velocity.dot(blend.normal) - blend.sample.velocity.y;
        let drag = -rel_y * blend.surface.physics.drag * mass * 4.0;
        let wave_follow = (blend.sample.velocity.y - body.velocity.dot(blend.normal))
            * mass
            * blend.surface.physics.buoyancy.max(0.0)
            * (1.2 + blend.surface.physics.wake_strength.max(0.0) * 0.25)
            * contact;
        let current_speed = blend.sample.velocity.x;
        let wave_speed = (current_speed + blend.sample.velocity.y.abs() * 0.012)
            * blend.surface.physics.wake_strength.max(0.0)
            * contact;
        let target_wave_speed = wave_speed.clamp(-1.5, 1.5);
        let body_wave_speed = body.velocity.dot(blend.wave_dir);
        let wave_drive = blend.wave_dir
            * ((target_wave_speed - body_wave_speed).clamp(-2.0, 2.0)
                * mass
                * contact
                * blend.surface.physics.drag.max(0.05)
                * 5.0);
        let (scale, deadzone) = water_force_lod(
            blend.surface.lod.near_distance,
            blend.surface.lod.mid_distance,
            blend.surface.lod.far_distance,
            blend.pos,
            camera_pos,
        );
        let cap = water_buoyancy_cap(
            mass,
            submerged,
            blend.surface.shape.depth(blend.surface.depth),
            blend.surface.physics.buoyancy,
        );
        let force_y =
            (support + spring + drag + wave_follow).clamp(-cap, cap) * scale * blend.lod_weight;
        let mass_scale = (1.0 / mass.max(1.0).sqrt()).clamp(0.35, 1.0);
        let force = blend.normal * force_y + wave_drive * scale * blend.lod_weight * mass_scale;
        if force.length() >= deadzone {
            forces.push((body.id, force));
        }
    }
    forces
}

fn water_forces_for_body_3d(
    body: RuntimeWaterBody3D,
    water_index: &RuntimeWaterIndex3D,
    water_samples: &AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    elapsed: f32,
    camera_pos: Vector2,
) -> Vec<(NodeID, Vector3)> {
    let samples = blended_water_samples_3d(
        body.pos,
        body.collision_layers,
        body.collision_mask,
        water_index,
        water_samples,
        water_body_samples,
        body.id,
        0,
        elapsed,
    );
    let mut forces = Vec::with_capacity(samples.len());
    for blend in samples {
        let float_radius = body.float_radius.max(0.0);
        let submerged = (blend.submerged + float_radius).max(0.0);
        if submerged <= 0.0 {
            continue;
        }
        let mass = body.mass.max(0.001);
        let target_submerged = (float_radius * body.density.clamp(0.05, 1.2))
            .max(water_target_submerged(body.density));
        let contact = (submerged / target_submerged.max(0.001)).clamp(0.0, 1.5);
        let support = mass * 9.81 * (submerged / target_submerged.max(0.001)).clamp(0.0, 1.25);
        let spring = (submerged - target_submerged) * blend.surface.physics.buoyancy * mass * 8.0;
        let rel_y = body.velocity.dot(blend.normal) - blend.sample.velocity.y;
        let drag = -rel_y * blend.surface.physics.drag * mass * 4.0;
        let wave_follow = (blend.sample.velocity.y - body.velocity.dot(blend.normal))
            * mass
            * blend.surface.physics.buoyancy.max(0.0)
            * (1.2 + blend.surface.physics.wake_strength.max(0.0) * 0.25)
            * contact;
        let current_speed = blend.sample.velocity.x;
        let wave_speed = (current_speed + blend.sample.velocity.y.abs() * 0.012)
            * blend.surface.physics.wake_strength.max(0.0)
            * contact;
        let target_wave_speed = wave_speed.clamp(-1.5, 1.5);
        let body_wave_speed = body.velocity.dot(blend.wave_dir);
        let wave_drive = blend.wave_dir
            * ((target_wave_speed - body_wave_speed).clamp(-2.0, 2.0)
                * mass
                * contact
                * blend.surface.physics.drag.max(0.05)
                * 5.0);
        let water_pos_2d = Vector2::new(blend.pos.x, blend.pos.z);
        let (scale, deadzone) = water_force_lod(
            blend.surface.lod.near_distance,
            blend.surface.lod.mid_distance,
            blend.surface.lod.far_distance,
            water_pos_2d,
            camera_pos,
        );
        let cap = water_buoyancy_cap(
            mass,
            submerged,
            blend.surface.shape.depth(blend.surface.depth),
            blend.surface.physics.buoyancy,
        );
        let force_y =
            (support + spring + drag + wave_follow).clamp(-cap, cap) * scale * blend.lod_weight;
        let mass_scale = (1.0 / mass.max(1.0).sqrt()).clamp(0.35, 1.0);
        let force = blend.normal * force_y + wave_drive * scale * blend.lod_weight * mass_scale;
        if force.length() >= deadzone {
            forces.push((body.id, force));
        }
    }
    forces
}

fn water_body_splashes_2d(
    bodies: &[RuntimeWaterBody2D],
    water_index: &RuntimeWaterIndex2D,
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    elapsed: f32,
) -> Vec<crate::runtime::ForceWaterImpact2D> {
    let mut impacts = Vec::new();
    let empty_samples = AHashMap::new();
    for body in bodies {
        for sample in blended_water_samples_2d(
            body.pos,
            body.collision_layers,
                body.collision_mask,
                water_index,
                &empty_samples,
                water_body_samples,
                body.id,
                0,
                elapsed,
            ) {
            let target = water_target_submerged(body.density);
            if sample.submerged <= 0.0 || sample.submerged > target * 2.25 {
                continue;
            }
            let rel_down = sample.sample.velocity.y - body.velocity.dot(sample.normal);
            if rel_down <= 0.35 {
                continue;
            }
            let strength = perro_nodes::water_impact_strength(
                body.mass.max(body.density),
                sample.normal * rel_down,
                sample.surface.physics.wake_strength,
            );
            if strength <= 0.0 {
                continue;
            }
            impacts.push(crate::runtime::ForceWaterImpact2D {
                position: sample.pos,
                force: -sample.normal * rel_down * body.mass.max(0.001),
                strength: strength.min(512.0),
                radius: body.mass.max(body.density).sqrt().clamp(0.65, 4.0),
                cavitation: (strength / 128.0).clamp(0.0, 1.0),
            });
        }
    }
    impacts
}

fn water_body_splashes_3d(
    bodies: &[RuntimeWaterBody3D],
    water_index: &RuntimeWaterIndex3D,
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    elapsed: f32,
) -> Vec<crate::runtime::ForceWaterImpact3D> {
    let mut impacts = Vec::new();
    let empty_samples = AHashMap::new();
    for body in bodies {
        for sample in blended_water_samples_3d(
            body.pos,
            body.collision_layers,
                body.collision_mask,
                water_index,
                &empty_samples,
                water_body_samples,
                body.id,
                0,
                elapsed,
            ) {
            let target = water_target_submerged(body.density);
            if sample.submerged <= 0.0 || sample.submerged > target * 2.25 {
                continue;
            }
            let rel_down = sample.sample.velocity.y - body.velocity.dot(sample.normal);
            if rel_down <= 0.35 {
                continue;
            }
            let strength = perro_nodes::water_impact_strength(
                body.mass.max(body.density),
                Vector2::new(0.0, rel_down),
                sample.surface.physics.wake_strength,
            );
            if strength <= 0.0 {
                continue;
            }
            impacts.push(crate::runtime::ForceWaterImpact3D {
                position: sample.pos,
                force: -sample.normal * rel_down * body.mass.max(0.001),
                strength: strength.min(512.0),
                radius: body.mass.max(body.density).sqrt().clamp(0.65, 4.0),
                cavitation: (strength / 128.0).clamp(0.0, 1.0),
            });
        }
    }
    impacts
}

fn blended_water_samples_2d(
    point: Vector2,
    body_layers: BitMask,
    body_mask: BitMask,
    water_index: &RuntimeWaterIndex2D,
    water_samples: &AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    body_id: NodeID,
    point_id: u8,
    elapsed: f32,
) -> Vec<BlendedWaterSample2D> {
    let mut first = None;
    let mut candidates: Vec<WaterCandidate2D> = Vec::new();
    let Some(bin) = water_index.bin(point.x) else {
        return Vec::new();
    };
    for &idx in bin {
        let water = water_index.waters[idx];
        if water.surface.collision_mask.intersects(body_layers)
            || body_mask.intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local = water_local_point_2d(water.inv_transform, point);
        if local.x.abs() > water.half.x || local.y.abs() > water.half.y {
            continue;
        }
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let sample = water_physics_sample_for_body_cached(
            &water.surface,
            local,
            elapsed,
            lookup_water_body_sample(
                water_body_samples,
                water.id,
                body_id,
                point_id,
                local,
                elapsed,
            ),
            water_samples.get(&water.id).copied(),
        );
        let surface_point =
            water_global_point_2d(water.transform, Vector2::new(local.x, sample.height));
        let candidate = WaterCandidate2D {
            water,
            local,
            surface_point,
            normal: water.normal,
            wave_dir: water_wave_dir_2d(water.transform, water.surface),
            sample,
            weight: water_blend_weight(water.surface.shape, local),
        };
        if let Some(existing) = first {
            if candidates.is_empty() {
                candidates.push(existing);
            }
            candidates.push(candidate);
        } else {
            first = Some(candidate);
        }
    }
    if candidates.is_empty() {
        return first.map(blended_water_sample_2d).into_iter().collect();
    }
    blend_water_candidates_2d(candidates)
}

fn blended_water_samples_3d(
    point: Vector3,
    body_layers: BitMask,
    body_mask: BitMask,
    water_index: &RuntimeWaterIndex3D,
    water_samples: &AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    body_id: NodeID,
    point_id: u8,
    elapsed: f32,
) -> Vec<BlendedWaterSample3D> {
    let mut first = None;
    let mut candidates: Vec<WaterCandidate3D> = Vec::new();
    let Some(bin) = water_index.bin(point.x) else {
        return Vec::new();
    };
    for &idx in bin {
        let water = water_index.waters[idx];
        if water.surface.collision_mask.intersects(body_layers)
            || body_mask.intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local3 = water_local_point_3d(water.inv_transform, point);
        let local = Vector2::new(local3.x, local3.z);
        if local.x.abs() > water.half.x || local.y.abs() > water.half.y {
            continue;
        }
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let sample = water_physics_sample_for_body_cached(
            &water.surface,
            local,
            elapsed,
            lookup_water_body_sample(
                water_body_samples,
                water.id,
                body_id,
                point_id,
                local,
                elapsed,
            ),
            water_samples.get(&water.id).copied(),
        );
        let surface_point = water_global_point_3d(
            water.transform,
            Vector3::new(local3.x, sample.height, local3.z),
        );
        let candidate = WaterCandidate3D {
            water,
            local: local3,
            surface_point,
            normal: water.normal,
            wave_dir: water_wave_dir_3d(water.transform, water.surface),
            sample,
            weight: water_blend_weight(water.surface.shape, local),
        };
        if let Some(existing) = first {
            if candidates.is_empty() {
                candidates.push(existing);
            }
            candidates.push(candidate);
        } else {
            first = Some(candidate);
        }
    }
    if candidates.is_empty() {
        return first.map(blended_water_sample_3d).into_iter().collect();
    }
    blend_water_candidates_3d(candidates)
}

fn blended_water_sample_2d(candidate: WaterCandidate2D) -> BlendedWaterSample2D {
    BlendedWaterSample2D {
        pos: candidate.surface_point,
        normal: candidate.normal,
        wave_dir: candidate.wave_dir,
        submerged: candidate.sample.height - candidate.local.y,
        surface: candidate.water.surface,
        sample: candidate.sample,
        lod_weight: 1.0,
    }
}

fn blended_water_sample_3d(candidate: WaterCandidate3D) -> BlendedWaterSample3D {
    BlendedWaterSample3D {
        pos: candidate.surface_point,
        normal: candidate.normal,
        wave_dir: candidate.wave_dir,
        submerged: candidate.sample.height - candidate.local.y,
        surface: candidate.water.surface,
        sample: candidate.sample,
        lod_weight: 1.0,
    }
}

impl RuntimeWaterIndex2D {
    fn new(waters: Vec<RuntimeWater2D>) -> Self {
        let (bins, origin_x, inv_cell_width) =
            build_water_bins(waters.iter().map(|water| (water.min_x, water.max_x)));
        Self {
            waters,
            bins,
            origin_x,
            inv_cell_width,
        }
    }

    fn bin(&self, point_x: f32) -> Option<&[usize]> {
        water_bin(&self.bins, self.origin_x, self.inv_cell_width, point_x)
    }
}

impl RuntimeWaterIndex3D {
    fn new(waters: Vec<RuntimeWater3D>) -> Self {
        let (bins, origin_x, inv_cell_width) =
            build_water_bins(waters.iter().map(|water| (water.min_x, water.max_x)));
        Self {
            waters,
            bins,
            origin_x,
            inv_cell_width,
        }
    }

    fn bin(&self, point_x: f32) -> Option<&[usize]> {
        water_bin(&self.bins, self.origin_x, self.inv_cell_width, point_x)
    }
}

fn build_water_bins(
    waters: impl Iterator<Item = (f32, f32)> + Clone,
) -> (Vec<Vec<usize>>, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_width = 0.0f32;
    let mut count = 0usize;
    for (water_min_x, water_max_x) in waters.clone() {
        min_x = min_x.min(water_min_x);
        max_x = max_x.max(water_max_x);
        max_width = max_width.max(water_max_x - water_min_x);
        count += 1;
    }
    if count == 0 || !min_x.is_finite() || !max_x.is_finite() {
        return (Vec::new(), 0.0, 1.0);
    }
    let cell_width = (max_width * 0.5).max(1.0);
    let inv_cell_width = 1.0 / cell_width;
    let bin_count = (((max_x - min_x) * inv_cell_width).ceil() as usize)
        .saturating_add(1)
        .max(1);
    let mut bins = vec![Vec::new(); bin_count];
    for (idx, (water_min_x, water_max_x)) in waters.enumerate() {
        let first = (((water_min_x - min_x) * inv_cell_width).floor() as isize)
            .clamp(0, bin_count.saturating_sub(1) as isize) as usize;
        let last = (((water_max_x - min_x) * inv_cell_width).floor() as isize)
            .clamp(0, bin_count.saturating_sub(1) as isize) as usize;
        for bin in &mut bins[first..=last] {
            bin.push(idx);
        }
    }
    (bins, min_x, inv_cell_width)
}

fn water_bin(
    bins: &[Vec<usize>],
    origin_x: f32,
    inv_cell_width: f32,
    point_x: f32,
) -> Option<&[usize]> {
    if bins.is_empty() {
        return None;
    }
    let idx = ((point_x - origin_x) * inv_cell_width).floor() as isize;
    if idx < 0 || idx as usize >= bins.len() {
        return None;
    }
    Some(&bins[idx as usize])
}

fn water_local_point_2d(inv_transform: Mat3, point: Vector2) -> Vector2 {
    let p = inv_transform * glam::Vec3::new(point.x, point.y, 1.0);
    Vector2::new(p.x, p.y)
}

fn water_global_point_2d(transform: Mat3, point: Vector2) -> Vector2 {
    let p = transform * glam::Vec3::new(point.x, point.y, 1.0);
    Vector2::new(p.x, p.y)
}

fn water_local_point_3d(inv_transform: Mat4, point: Vector3) -> Vector3 {
    inv_transform.transform_point3(point.into()).into()
}

fn water_global_point_3d(transform: Mat4, point: Vector3) -> Vector3 {
    transform.transform_point3(point.into()).into()
}

pub(crate) fn water_physics_sample_for_body(
    surface: &perro_nodes::WaterSurfaceParams,
    local: Vector2,
    elapsed: f32,
) -> perro_nodes::WaterPhysicsSample {
    water_physics_sample_for_body_cached(surface, local, elapsed, None, None)
}

pub(crate) fn water_physics_sample_for_body_cached(
    surface: &perro_nodes::WaterSurfaceParams,
    local: Vector2,
    elapsed: f32,
    body_cached: Option<crate::runtime::WaterBodySampleCache>,
    cached: Option<perro_nodes::WaterPhysicsSample>,
) -> perro_nodes::WaterPhysicsSample {
    if let Some(body_cached) = body_cached {
        return perro_nodes::WaterPhysicsSample {
            height: body_cached.height,
            velocity: body_cached.velocity,
            foam: body_cached.foam,
        };
    }
    let mut sample = water_physics_sample_or_idle(surface, local, elapsed, None);
    let height_offset = cached.map_or(0.0, |cached| {
        let center_now = water_physics_sample_or_idle(surface, Vector2::ZERO, elapsed, None).height;
        cached.height - center_now
    });
    sample.height += height_offset;
    if let Some(cached) = cached {
        sample.foam = cached.foam;
    }
    let prev_height =
        water_physics_sample_or_idle(surface, local, elapsed - WATER_WAVE_FOLLOW_DT, None).height
            + height_offset;
    sample.velocity.y = (sample.height - prev_height) / WATER_WAVE_FOLLOW_DT;
    sample.velocity.x = surface.flow.dot(water_wave_local_dir(*surface));
    sample
}

fn lookup_water_body_sample(
    water_body_samples: &AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    water: NodeID,
    body: NodeID,
    point: u8,
    local: Vector2,
    elapsed: f32,
) -> Option<crate::runtime::WaterBodySampleCache> {
    let key = crate::runtime::WaterBodySampleKey { water, body, point };
    let sample = water_body_samples.get(&key).copied()?;
    if elapsed - sample.sample_time > WATER_BODY_SAMPLE_TTL {
        return None;
    }
    if (sample.local - local).length() > WATER_QUERY_LOCAL_EPS {
        return None;
    }
    Some(sample)
}

fn water_wave_local_dir(surface: perro_nodes::WaterSurfaceParams) -> Vector2 {
    let dir = if surface.flow.length_squared() > 1.0e-6 {
        surface.flow
    } else {
        surface.wind
    };
    water_normalize_2d(dir)
}

fn water_wave_dir_2d(transform: Mat3, surface: perro_nodes::WaterSurfaceParams) -> Vector2 {
    let dir = water_wave_local_dir(surface);
    let v = transform * glam::Vec3::new(dir.x, dir.y, 0.0);
    water_normalize_2d(Vector2::new(v.x, v.y))
}

fn water_wave_dir_3d(transform: Mat4, surface: perro_nodes::WaterSurfaceParams) -> Vector3 {
    let dir = water_wave_local_dir(surface);
    let v = transform.transform_vector3(Vec3::new(dir.x, 0.0, dir.y));
    water_normalize_3d(Vector3::new(v.x, 0.0, v.z))
}

fn water_normal_2d(transform: Mat3) -> Vector2 {
    let up = transform * glam::Vec3::new(0.0, 1.0, 0.0);
    water_normalize_2d(Vector2::new(up.x, up.y))
}

fn water_normal_3d(transform: Mat4) -> Vector3 {
    water_normalize_3d(transform.transform_vector3(Vec3::Y).into())
}

fn water_normalize_2d(v: Vector2) -> Vector2 {
    let len = (v.x * v.x + v.y * v.y).sqrt();
    if len > 1.0e-6 {
        v / len
    } else {
        Vector2::new(0.0, 1.0)
    }
}

fn water_normalize_3d(v: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len > 1.0e-6 {
        v / len
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    }
}

fn water_world_x_bounds_2d(transform: Mat3, half: Vector2) -> (f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    for point in [
        Vector2::new(-half.x, -half.y),
        Vector2::new(half.x, -half.y),
        Vector2::new(-half.x, half.y),
        Vector2::new(half.x, half.y),
    ] {
        let p = water_global_point_2d(transform, point);
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
    }
    (min_x, max_x)
}

fn water_world_x_bounds_3d(transform: Mat4, half: Vector2, depth: f32) -> (f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    for y in [0.0, -depth] {
        for point in [
            Vector3::new(-half.x, y, -half.y),
            Vector3::new(half.x, y, -half.y),
            Vector3::new(-half.x, y, half.y),
            Vector3::new(half.x, y, half.y),
        ] {
            let p = water_global_point_3d(transform, point);
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
        }
    }
    (min_x, max_x)
}

fn register_water_query_candidates_2d(
    out: &mut AHashMap<NodeID, Vec<crate::runtime::PendingWaterQuery>>,
    water_index: &RuntimeWaterIndex2D,
    body: RuntimeWaterBody2D,
    point: u8,
    pos: Vector2,
) {
    let Some(bin) = water_index.bin(pos.x) else {
        return;
    };
    for &idx in bin {
        let water = water_index.waters[idx];
        if water.surface.collision_mask.intersects(body.collision_layers)
            || body.collision_mask.intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local = water_local_point_2d(water.inv_transform, pos);
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let list = out.entry(water.id).or_default();
        if list.len() >= WATER_QUERY_MAX_PER_WATER
            || list.iter().any(|query| query.body == body.id && query.point == point)
        {
            continue;
        }
        list.push(crate::runtime::PendingWaterQuery {
            body: body.id,
            point,
            local,
        });
    }
}

fn register_water_query_candidates_3d(
    out: &mut AHashMap<NodeID, Vec<crate::runtime::PendingWaterQuery>>,
    water_index: &RuntimeWaterIndex3D,
    body: RuntimeWaterBody3D,
    point: u8,
    pos: Vector3,
) {
    let Some(bin) = water_index.bin(pos.x) else {
        return;
    };
    for &idx in bin {
        let water = water_index.waters[idx];
        if water.surface.collision_mask.intersects(body.collision_layers)
            || body.collision_mask.intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local = water_local_point_3d(water.inv_transform, pos);
        let local_xz = Vector2::new(local.x, local.z);
        if !water.surface.shape.contains_surface(local_xz) {
            continue;
        }
        let list = out.entry(water.id).or_default();
        if list.len() >= WATER_QUERY_MAX_PER_WATER
            || list.iter().any(|query| query.body == body.id && query.point == point)
        {
            continue;
        }
        list.push(crate::runtime::PendingWaterQuery {
            body: body.id,
            point,
            local: local_xz,
        });
    }
}

fn sample_water_id_2d(
    point: Vector2,
    water_index: &RuntimeWaterIndex2D,
    surface_point: Vector2,
) -> Option<NodeID> {
    let bin = water_index.bin(point.x)?;
    let mut best = None;
    let mut best_dist = f32::INFINITY;
    for &idx in bin {
        let water = water_index.waters[idx];
        let local = water_local_point_2d(water.inv_transform, point);
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let dist = (surface_point - water_global_point_2d(water.transform, Vector2::new(local.x, surface_point.y))).length();
        if dist < best_dist {
            best = Some(water.id);
            best_dist = dist;
        }
    }
    best
}

fn sample_water_id_3d(
    point: Vector3,
    water_index: &RuntimeWaterIndex3D,
    surface_point: Vector3,
) -> Option<NodeID> {
    let bin = water_index.bin(point.x)?;
    let mut best = None;
    let mut best_dist = f32::INFINITY;
    for &idx in bin {
        let water = water_index.waters[idx];
        let local = water_local_point_3d(water.inv_transform, point);
        if !water
            .surface
            .shape
            .contains_surface(Vector2::new(local.x, local.z))
        {
            continue;
        }
        let dist = (surface_point
            - water_global_point_3d(water.transform, Vector3::new(local.x, surface_point.y, local.z)))
        .length();
        if dist < best_dist {
            best = Some(water.id);
            best_dist = dist;
        }
    }
    best
}

fn blend_water_group_2d(candidates: &[WaterCandidate2D], group: &[usize]) -> BlendedWaterSample2D {
    if group.len() == 1 {
        let candidate = candidates[group[0]];
        return blended_water_sample_2d(candidate);
    }
    let mut total = 0.0;
    let mut pos = Vector2::ZERO;
    let mut normal = Vector2::ZERO;
    let mut wave_dir = Vector2::ZERO;
    let mut submerged = 0.0;
    let mut sample = perro_nodes::WaterPhysicsSample::default();
    let mut surface = candidates[group[0]].water.surface;
    let mut buoyancy = 0.0;
    let mut drag = 0.0;
    for &idx in group {
        let candidate = candidates[idx];
        let w = candidate.weight.max(0.001);
        total += w;
        pos += candidate.surface_point * w;
        normal += candidate.normal * w;
        wave_dir += candidate.wave_dir * w;
        submerged += (candidate.sample.height - candidate.local.y) * w;
        sample.height += candidate.surface_point.y * w;
        sample.velocity +=
            candidate.sample.velocity * w * candidate.water.surface.link.flow_transfer;
        sample.foam += candidate.sample.foam * w * candidate.water.surface.link.wave_transfer;
        buoyancy += candidate.water.surface.physics.buoyancy * w;
        drag += candidate.water.surface.physics.drag * w;
    }
    let inv = 1.0 / total.max(0.001);
    pos *= inv;
    normal = water_normalize_2d(normal);
    wave_dir = water_normalize_2d(wave_dir);
    submerged *= inv;
    sample.height *= inv;
    sample.velocity *= inv;
    sample.foam *= inv;
    surface.physics.buoyancy = buoyancy * inv;
    surface.physics.drag = drag * inv;
    BlendedWaterSample2D {
        pos,
        normal,
        wave_dir,
        submerged,
        surface,
        sample,
        lod_weight: 1.0,
    }
}

fn blend_water_group_3d(candidates: &[WaterCandidate3D], group: &[usize]) -> BlendedWaterSample3D {
    if group.len() == 1 {
        let candidate = candidates[group[0]];
        return blended_water_sample_3d(candidate);
    }
    let mut total = 0.0;
    let mut pos = Vector3::ZERO;
    let mut normal = Vector3::ZERO;
    let mut wave_dir = Vector3::ZERO;
    let mut submerged = 0.0;
    let mut sample = perro_nodes::WaterPhysicsSample::default();
    let mut surface = candidates[group[0]].water.surface;
    let mut buoyancy = 0.0;
    let mut drag = 0.0;
    for &idx in group {
        let candidate = candidates[idx];
        let w = candidate.weight.max(0.001);
        total += w;
        pos += candidate.surface_point * w;
        normal += candidate.normal * w;
        wave_dir += candidate.wave_dir * w;
        submerged += (candidate.sample.height - candidate.local.y) * w;
        sample.height += candidate.surface_point.y * w;
        sample.velocity +=
            candidate.sample.velocity * w * candidate.water.surface.link.flow_transfer;
        sample.foam += candidate.sample.foam * w * candidate.water.surface.link.wave_transfer;
        buoyancy += candidate.water.surface.physics.buoyancy * w;
        drag += candidate.water.surface.physics.drag * w;
    }
    let inv = 1.0 / total.max(0.001);
    pos *= inv;
    normal = water_normalize_3d(normal);
    wave_dir = water_normalize_3d(wave_dir);
    submerged *= inv;
    sample.height *= inv;
    sample.velocity *= inv;
    sample.foam *= inv;
    surface.physics.buoyancy = buoyancy * inv;
    surface.physics.drag = drag * inv;
    BlendedWaterSample3D {
        pos,
        normal,
        wave_dir,
        submerged,
        surface,
        sample,
        lod_weight: 1.0,
    }
}

fn water_link_allowed(
    a: perro_nodes::WaterSurfaceParams,
    b: perro_nodes::WaterSurfaceParams,
) -> bool {
    !a.link.link_mask.intersects(b.link.link_layers)
        && !b.link.link_mask.intersects(a.link.link_layers)
}

fn water_linked_2d(a: RuntimeWater2D, b: RuntimeWater2D) -> bool {
    water_link_allowed(a.surface, b.surface)
}

fn water_linked_3d(a: RuntimeWater3D, b: RuntimeWater3D) -> bool {
    water_link_allowed(a.surface, b.surface)
}

fn water_blend_weight(shape: WaterShape, local: Vector2) -> f32 {
    let t = match shape {
        WaterShape::Rect { size } => {
            let half = size * 0.5;
            let edge = (half.x - local.x.abs()).min(half.y - local.y.abs());
            let width = (half.x.min(half.y) * 0.25).max(0.5);
            (edge / width).clamp(0.0, 1.0)
        }
        WaterShape::Box { size } => {
            let half = Vector2::new(size.x, size.z) * 0.5;
            let edge = (half.x - local.x.abs()).min(half.y - local.y.abs());
            let width = (half.x.min(half.y) * 0.25).max(0.5);
            (edge / width).clamp(0.0, 1.0)
        }
        WaterShape::Circle { radius } | WaterShape::Cylinder { radius, .. } => {
            let edge = radius - local.length();
            let width = (radius * 0.25).max(0.5);
            (edge / width).clamp(0.0, 1.0)
        }
    };
    smoothstep(t)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn water_shape_3d(shape: WaterShape, fallback_depth: f32) -> (Shape3D, f32) {
    match shape {
        WaterShape::Cylinder {
            radius,
            half_height,
        } => (
            Shape3D::Cylinder {
                radius,
                half_height,
            },
            -half_height,
        ),
        WaterShape::Circle { radius } => {
            let half_height = fallback_depth.max(0.001) * 0.5;
            (
                Shape3D::Cylinder {
                    radius,
                    half_height,
                },
                -half_height,
            )
        }
        WaterShape::Box { size } => (
            Shape3D::Cube {
                size: Vector3::new(size.x, size.y.max(0.001), size.z),
            },
            -size.y.max(0.001) * 0.5,
        ),
        WaterShape::Rect { size } => {
            let depth = fallback_depth.max(0.001);
            (
                Shape3D::Cube {
                    size: Vector3::new(size.x, depth, size.y),
                },
                -depth * 0.5,
            )
        }
    }
}

fn hash_water_shape(state: u64, shape: WaterShape) -> u64 {
    match shape {
        WaterShape::Rect { .. } | WaterShape::Circle { .. } => {
            hash_shape_2d(state, water_shape_2d(shape))
        }
        WaterShape::Box { .. } | WaterShape::Cylinder { .. } => {
            let (shape, _) = water_shape_3d(shape, 0.001);
            hash_shape_3d(state, &shape)
        }
    }
}

fn force_emitter_active(enabled: bool, pulse: bool, duration: f32, age: f32) -> bool {
    enabled && !(pulse && age > 0.0) && (duration <= 0.0 || age <= duration)
}

fn falloff_scale(dist: f32, radius: f32, falloff: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let t = (1.0 - dist / radius).clamp(0.0, 1.0);
    if falloff <= 0.0 { 1.0 } else { t.powf(falloff) }
}

fn force_emitter_force_2d(
    emitter: &perro_nodes::PhysicsForceEmitter2D,
    offset: Vector2,
    dist: f32,
) -> Vector2 {
    let scale = emitter.strength * falloff_scale(dist, emitter.radius, emitter.falloff);
    match emitter.profile {
        perro_nodes::PhysicsForceProfile::Lift => Vector2::new(0.0, 1.0) * scale,
        perro_nodes::PhysicsForceProfile::Explosion => {
            if dist <= 0.000_1 {
                Vector2::new(0.0, 1.0) * scale
            } else {
                offset.normalized() * scale
            }
        }
        perro_nodes::PhysicsForceProfile::Current => {
            emitter
                .vectors
                .first()
                .copied()
                .unwrap_or(Vector2::new(1.0, 0.0))
                * scale
        }
        perro_nodes::PhysicsForceProfile::Vortex => {
            let dir = if dist <= 0.000_1 {
                Vector2::new(1.0, 0.0)
            } else {
                offset.normalized()
            };
            Vector2::new(-dir.y, dir.x) * scale + dir * (-0.35 * scale)
        }
        perro_nodes::PhysicsForceProfile::Custom => {
            sample_force_vectors_2d(
                &emitter.vectors,
                if emitter.radius > 0.0 {
                    dist / emitter.radius
                } else {
                    0.0
                },
            ) * emitter.strength
        }
    }
}

fn force_emitter_force_3d(
    emitter: &perro_nodes::PhysicsForceEmitter3D,
    offset: Vector3,
    dist: f32,
) -> Vector3 {
    let scale = emitter.strength * falloff_scale(dist, emitter.radius, emitter.falloff);
    match emitter.profile {
        perro_nodes::PhysicsForceProfile::Lift => Vector3::new(0.0, 1.0, 0.0) * scale,
        perro_nodes::PhysicsForceProfile::Explosion => {
            if offset.length_squared() <= 0.000_1 {
                Vector3::new(0.0, 1.0, 0.0) * scale
            } else {
                offset.normalized() * scale
            }
        }
        perro_nodes::PhysicsForceProfile::Current => {
            emitter
                .vectors
                .first()
                .copied()
                .unwrap_or(Vector3::new(1.0, 0.0, 0.0))
                * scale
        }
        perro_nodes::PhysicsForceProfile::Vortex => {
            let flat = Vector2::new(offset.x, offset.z);
            let dir = if flat.length_squared() <= 0.000_1 {
                Vector2::new(1.0, 0.0)
            } else {
                flat.normalized()
            };
            Vector3::new(-dir.y * scale, 0.0, dir.x * scale)
                + Vector3::new(dir.x, 0.0, dir.y) * (-0.35 * scale)
        }
        perro_nodes::PhysicsForceProfile::Custom => {
            sample_force_vectors_3d(
                &emitter.vectors,
                if emitter.radius > 0.0 {
                    dist / emitter.radius
                } else {
                    0.0
                },
            ) * emitter.strength
        }
    }
}

fn sample_force_vectors_2d(vectors: &[Vector2], t: f32) -> Vector2 {
    if vectors.is_empty() {
        return Vector2::ZERO;
    }
    if vectors.len() == 1 {
        return vectors[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (vectors.len() - 1) as f32;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(vectors.len() - 1);
    let frac = scaled - idx as f32;
    vectors[idx] * (1.0 - frac) + vectors[next] * frac
}

fn sample_force_vectors_3d(vectors: &[Vector3], t: f32) -> Vector3 {
    if vectors.is_empty() {
        return Vector3::ZERO;
    }
    if vectors.len() == 1 {
        return vectors[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (vectors.len() - 1) as f32;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(vectors.len() - 1);
    let frac = scaled - idx as f32;
    vectors[idx] * (1.0 - frac) + vectors[next] * frac
}

#[cfg(test)]
mod tests;
