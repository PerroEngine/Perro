use super::RuntimePhysicsStepTiming;
use crate::Runtime;
use ahash::{AHashMap, AHashSet};
#[cfg(test)]
use glam::{Mat3, Mat4};
use perro_ids::{NodeID, SignalID};
#[cfg(test)]
use perro_nodes::TileMap2D;
use perro_nodes::{Node2D, Node3D, SceneNodeData, Shape2D, Shape3D, WaterShape};
use perro_physics::*;
use perro_runtime_api::sub_apis::{
    NodeAPI, PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D,
    PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D, SignalAPI,
};
#[cfg(test)]
use perro_structs::BitMask;
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;
use rayon::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[path = "physics/water.rs"]
mod water;

use water::*;
pub(crate) use water::{
    lookup_water_body_sample, water_physics_sample_for_body_cached, water_target_submerged,
};

pub(crate) type PhysicsState = PhysicsSystem;
pub(crate) use perro_physics::{AudioRaycastInput, AudioRaycastResult};

impl Runtime {
    pub fn get_physics_gravity(&self) -> f32 {
        self.physics_gravity_raw()
    }

    pub fn set_physics_gravity(&mut self, gravity: f32) {
        if gravity.is_finite() {
            self.physics_gravity_override = Some(gravity);
        }
    }

    pub fn get_physics_coefficient(&self) -> f32 {
        self.physics_coef()
    }

    pub fn set_physics_coefficient(&mut self, coefficient: f32) {
        if coefficient.is_finite() && coefficient > 0.0 {
            self.physics_coef_override = Some(coefficient);
        }
    }

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
        let total_start = Instant::now();

        let pre_transforms_start = Instant::now();
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let pre_transforms = pre_transforms_start.elapsed();

        let collect_start = Instant::now();
        let bodies_2d = self.collect_body_descs_2d();
        let bodies_3d = self.collect_body_descs_3d();
        let joints_2d = self.collect_joint_descs_2d();
        let joints_3d = self.collect_joint_descs_3d();
        let collect = collect_start.elapsed();

        let sync_world_start = Instant::now();
        self.sync_world_2d(&bodies_2d);
        self.sync_world_3d(&bodies_3d);
        self.sync_joints_parallel(&joints_2d, &joints_3d);
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

        let apply_forces_impulses_start = Instant::now();
        self.reset_water_scan_cache_all();
        self.queue_physics_force_emitters_2d();
        self.queue_physics_force_emitters_3d();
        self.queue_water_forces_2d();
        self.queue_water_forces_3d();
        self.apply_pending_forces_and_impulses_parallel();
        let apply_forces_impulses = apply_forces_impulses_start.elapsed();

        let step_start = Instant::now();
        self.step_worlds_parallel();
        let step = step_start.elapsed();

        let sync_nodes_start = Instant::now();
        let changed_2d = self.sync_world_to_nodes_2d();
        let changed_3d = self.sync_world_to_nodes_3d();
        let sync_nodes = sync_nodes_start.elapsed();

        let post_transforms_start = Instant::now();
        if changed_2d || changed_3d {
            self.propagate_pending_transform_dirty();
            self.refresh_dirty_global_transforms();
        }
        let post_transforms = post_transforms_start.elapsed();

        let signals_start = Instant::now();
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

    fn sync_joints_parallel(&mut self, joints_2d: &[JointDesc2D], joints_3d: &[JointDesc3D]) {
        self.physics.sync_joints_parallel(joints_2d, joints_3d);
    }

    fn step_worlds_parallel(&mut self) {
        self.physics
            .step_worlds_parallel(self.physics_gravity(), self.time.fixed_delta);
    }

    fn apply_pending_forces_and_impulses_parallel(&mut self) {
        self.physics
            .apply_pending_forces_and_impulses_parallel(self.physics_coef(), self.time.fixed_delta);
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
        let radius_sq = emitter.radius * emitter.radius;
        let body_count = self.internal_updates.physics_body_nodes_2d.len();
        for i in 0..body_count {
            let id = self.internal_updates.physics_body_nodes_2d[i];
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
            let dist_sq = offset.length_squared();
            if dist_sq > radius_sq {
                continue;
            }
            let dist = dist_sq.sqrt();
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
        let radius_sq = emitter.radius * emitter.radius;
        let body_count = self.internal_updates.physics_body_nodes_3d.len();
        for i in 0..body_count {
            let id = self.internal_updates.physics_body_nodes_3d[i];
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
            let dist_sq = offset.length_squared();
            if dist_sq > radius_sq {
                continue;
            }
            let dist = dist_sq.sqrt();
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
        let ids = self.cached_water_ids_2d().to_vec();
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
        let ids = self.cached_water_ids_3d().to_vec();
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
        let water_ids = self.cached_water_ids_2d().to_vec();
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

        let body_ids = self.cached_rigid_body_ids_2d().to_vec();
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
            let sleeping = self
                .physics
                .world_2d
                .as_ref()
                .and_then(|world| {
                    world
                        .body_map
                        .get(&body_id)
                        .and_then(|state| world.bodies.get(state.handle))
                })
                .map(|body| body.is_sleeping())
                .unwrap_or(false);
            bodies.push(RuntimeWaterBody2D {
                id: body_id,
                pos: body_transform.position,
                velocity,
                mass,
                density,
                float_radius: self.body_float_radius_2d(body_id, body_transform.position),
                sleeping,
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
        for effect in forces {
            self.physics.queue_force_2d(effect.id, effect.force);
            if effect.impulse.length_squared() > 0.000_001 {
                self.physics.queue_impulse_2d(effect.id, effect.impulse);
            }
            self.apply_water_angular_nudge_2d(effect.id, effect.force.x * 0.04);
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
        let water_ids = self.cached_water_ids_3d().to_vec();
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

        let body_ids = self.cached_rigid_body_ids_3d().to_vec();
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
            let sleeping = self
                .physics
                .world_3d
                .as_ref()
                .and_then(|world| {
                    world
                        .body_map
                        .get(&body_id)
                        .and_then(|state| world.bodies.get(state.handle))
                })
                .map(|body| body.is_sleeping())
                .unwrap_or(false);
            bodies.push(RuntimeWaterBody3D {
                id: body_id,
                pos: body_transform.position,
                velocity,
                mass,
                density,
                float_radius: self.body_float_radius_3d(body_id, body_transform.position),
                sleeping,
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
        for effect in forces {
            self.physics.queue_force_3d(effect.id, effect.force);
            if effect.impulse.length_squared() > 0.000_001 {
                self.physics.queue_impulse_3d(effect.id, effect.impulse);
            }
            self.apply_water_angular_nudge_3d(
                effect.id,
                Vector3::new(effect.force.z * 0.025, 0.0, -effect.force.x * 0.025),
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
        let child_count = self
            .nodes
            .get(body)
            .map(|node| node.children_slice().len())
            .unwrap_or(0);
        let mut radius = 0.0f32;
        for i in 0..child_count {
            let Some(child_id) = self
                .nodes
                .get(body)
                .and_then(|node| node.children_slice().get(i).copied())
            else {
                continue;
            };
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
        let child_count = self
            .nodes
            .get(body)
            .map(|node| node.children_slice().len())
            .unwrap_or(0);
        let mut radius = 0.0f32;
        for i in 0..child_count {
            let Some(child_id) = self
                .nodes
                .get(body)
                .and_then(|node| node.children_slice().get(i).copied())
            else {
                continue;
            };
            let Some(shape_y) = self.nodes.get(child_id).and_then(|child| {
                let SceneNodeData::CollisionShape3D(shape) = &child.data else {
                    return None;
                };
                Some(match &shape.shape {
                    Shape3D::Cube { size }
                    | Shape3D::TriPrism { size }
                    | Shape3D::TriangularPyramid { size }
                    | Shape3D::SquarePyramid { size } => size.y.abs() * 0.5,
                    Shape3D::Sphere { radius } => radius.abs(),
                    Shape3D::Capsule {
                        radius,
                        half_height,
                    } => radius.abs() + half_height.abs(),
                    Shape3D::Cylinder { half_height, .. } | Shape3D::Cone { half_height, .. } => {
                        half_height.abs()
                    }
                    Shape3D::TriMesh { .. } => 0.0,
                })
            }) else {
                continue;
            };
            let Some(global) = self.get_global_transform_3d(child_id) else {
                continue;
            };
            let half_y = shape_y * global.scale.y.abs();
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
            let sample_points = [
                (0u8, body.pos),
                (1u8, body.pos + Vector2::new(-radius * 0.75, 0.0)),
                (2u8, body.pos + Vector2::new(radius * 0.75, 0.0)),
            ];
            let sample_count = if body.sleeping {
                1
            } else {
                sample_points.len()
            };
            for (point, pos) in sample_points.into_iter().take(sample_count) {
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
            let sample_points = [
                (0u8, body.pos),
                (1u8, body.pos + Vector3::new(-radius * 0.75, 0.0, 0.0)),
                (2u8, body.pos + Vector3::new(radius * 0.75, 0.0, 0.0)),
                (3u8, body.pos + Vector3::new(0.0, 0.0, -radius * 0.75)),
                (4u8, body.pos + Vector3::new(0.0, 0.0, radius * 0.75)),
            ];
            let sample_count = if body.sleeping {
                1
            } else {
                sample_points.len()
            };
            for (point, pos) in sample_points.into_iter().take(sample_count) {
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
            for sample in blended_water_samples_2d(WaterBlendQuery2D {
                point: body.pos,
                body_layers: body.collision_layers,
                body_mask: body.collision_mask,
                water_index,
                water_samples: &empty_samples,
                water_body_samples: &self.water_body_samples,
                body_id: body.id,
                point_id: 0,
                elapsed,
            }) {
                if sample.submerged <= 0.0 {
                    continue;
                }
                if let Some(water_id) = sample_water_id_2d(body.pos, water_index, sample.pos) {
                    self.water_contacts_2d.entry(water_id).or_default().push(
                        crate::runtime::WaterBodyContact2D {
                            position: sample.pos,
                            velocity: body.velocity,
                            radius: body.float_radius.max(0.75) * 0.5,
                            foam_amount: (sample.sample.foam + body.velocity.length() * 0.06)
                                .clamp(0.1, 1.0),
                        },
                    );
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
            for sample in blended_water_samples_3d(WaterBlendQuery3D {
                point: body.pos,
                body_layers: body.collision_layers,
                body_mask: body.collision_mask,
                water_index,
                water_samples: &empty_samples,
                water_body_samples: &self.water_body_samples,
                body_id: body.id,
                point_id: 0,
                elapsed,
            }) {
                if sample.submerged <= 0.0 {
                    continue;
                }
                if let Some(water_id) = sample_water_id_3d(body.pos, water_index, sample.pos) {
                    self.water_contacts_3d.entry(water_id).or_default().push(
                        crate::runtime::WaterBodyContact3D {
                            position: sample.pos,
                            velocity: body.velocity,
                            radius: body.float_radius.max(0.75) * 0.5,
                            foam_amount: (sample.sample.foam
                                + Vector2::new(body.velocity.x, body.velocity.z).length() * 0.05)
                                .clamp(0.1, 1.0),
                        },
                    );
                }
            }
        }
    }

    fn sync_world_to_nodes_2d(&mut self) -> bool {
        let Some(mut world) = self.physics.world_2d.take() else {
            return false;
        };
        let mut changed = false;

        for (&id, state) in &mut world.body_map {
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
            let sleeping = body.is_sleeping();
            let same_as_last_sync = body_sync_same_2d(state, position, rotation, lin, ang);
            if sleeping && same_as_last_sync && state.idle_sync_frames >= 1 {
                continue;
            }
            let parent = self
                .nodes
                .get(id)
                .map(|node| node.parent)
                .unwrap_or(NodeID::nil());
            let before = self
                .get_global_transform_2d(id)
                .unwrap_or(Transform2D::IDENTITY);
            let curr = Transform2D {
                position,
                rotation,
                scale: before.scale,
            };

            self.record_physics_pose_2d(id, parent, before, curr);
            self.set_physics_body_transform_2d(id, position, rotation);
            changed = true;

            if let Some(scene_node) = self.nodes.get_mut(id)
                && let SceneNodeData::RigidBody2D(node) = &mut scene_node.data
            {
                node.linear_velocity = lin;
                node.angular_velocity = ang;
            }
            update_body_sync_state_2d(
                state,
                position,
                rotation,
                lin,
                ang,
                sleeping,
                same_as_last_sync,
            );
        }

        self.physics.world_2d = Some(world);
        changed
    }

    fn sync_world_to_nodes_3d(&mut self) -> bool {
        let Some(mut world) = self.physics.world_3d.take() else {
            return false;
        };
        let mut changed = false;

        for (&id, state) in &mut world.body_map {
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
            let sleeping = body.is_sleeping();
            let same_as_last_sync = body_sync_same_3d(state, position, rotation, lin, ang);
            if sleeping && same_as_last_sync && state.idle_sync_frames >= 1 {
                continue;
            }
            let parent = self
                .nodes
                .get(id)
                .map(|node| node.parent)
                .unwrap_or(NodeID::nil());
            let before = self
                .get_global_transform_3d(id)
                .unwrap_or(Transform3D::IDENTITY);
            let curr = Transform3D {
                position,
                rotation,
                scale: before.scale,
            };

            self.record_physics_pose_3d(id, parent, before, curr);
            self.set_physics_body_transform_3d(id, position, rotation);
            changed = true;

            if let Some(scene_node) = self.nodes.get_mut(id)
                && let SceneNodeData::RigidBody3D(node) = &mut scene_node.data
            {
                node.linear_velocity = lin;
                node.angular_velocity = ang;
            }
            update_body_sync_state_3d(
                state,
                position,
                rotation,
                lin,
                ang,
                sleeping,
                same_as_last_sync,
            );
        }

        self.physics.world_3d = Some(world);
        changed
    }

    fn set_physics_body_transform_2d(&mut self, id: NodeID, position: Vector2, rotation: f32) {
        let Some(parent) = self.nodes.get(id).map(|node| node.parent) else {
            return;
        };
        if parent.is_nil() {
            let _ = self.with_base_node_mut::<Node2D, _, _>(id, |node| {
                node.transform.position = position;
                node.transform.rotation = rotation;
            });
            return;
        }

        let mut target = self
            .get_global_transform_2d(id)
            .unwrap_or(Transform2D::IDENTITY);
        target.position = position;
        target.rotation = rotation;
        let _ = NodeAPI::set_global_transform_2d(self, id, target);
    }

    fn set_physics_body_transform_3d(
        &mut self,
        id: NodeID,
        position: Vector3,
        rotation: Quaternion,
    ) {
        let Some(parent) = self.nodes.get(id).map(|node| node.parent) else {
            return;
        };
        if parent.is_nil() {
            let _ = self.with_base_node_mut::<Node3D, _, _>(id, |node| {
                node.transform.position = position;
                node.transform.rotation = rotation;
            });
            return;
        }

        let mut target = self
            .get_global_transform_3d(id)
            .unwrap_or(Transform3D::IDENTITY);
        target.position = position;
        target.rotation = rotation;
        let _ = NodeAPI::set_global_transform_3d(self, id, target);
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
        self.physics_gravity_raw() * self.physics_coef()
    }

    fn physics_gravity_raw(&self) -> f32 {
        self.physics_gravity_override
            .or_else(|| self.project().map(|p| p.config.physics_gravity))
            .filter(|v| v.is_finite())
            .unwrap_or(-9.81)
    }

    fn physics_coef(&self) -> f32 {
        self.physics_coef_override
            .or_else(|| self.project().map(|p| p.config.physics_coef))
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

fn body_sync_same_2d(
    state: &BodyState2D,
    position: Vector2,
    rotation: f32,
    linear_velocity: Vector2,
    angular_velocity: f32,
) -> bool {
    approx_eq_f32(state.last_translation[0], position.x)
        && approx_eq_f32(state.last_translation[1], position.y)
        && approx_eq_f32(state.last_rotation, rotation)
        && approx_eq_f32(state.last_linear_velocity[0], linear_velocity.x)
        && approx_eq_f32(state.last_linear_velocity[1], linear_velocity.y)
        && approx_eq_f32(state.last_angular_velocity, angular_velocity)
}

fn update_body_sync_state_2d(
    state: &mut BodyState2D,
    position: Vector2,
    rotation: f32,
    linear_velocity: Vector2,
    angular_velocity: f32,
    sleeping: bool,
    same_as_last_sync: bool,
) {
    state.last_translation = [position.x, position.y];
    state.last_rotation = rotation;
    state.last_linear_velocity = [linear_velocity.x, linear_velocity.y];
    state.last_angular_velocity = angular_velocity;
    state.idle_sync_frames = if sleeping && same_as_last_sync {
        state.idle_sync_frames.saturating_add(1)
    } else {
        0
    };
}

fn body_sync_same_3d(
    state: &BodyState3D,
    position: Vector3,
    rotation: Quaternion,
    linear_velocity: Vector3,
    angular_velocity: Vector3,
) -> bool {
    approx_eq_f32(state.last_translation[0], position.x)
        && approx_eq_f32(state.last_translation[1], position.y)
        && approx_eq_f32(state.last_translation[2], position.z)
        && approx_eq_f32(state.last_rotation[0], rotation.x)
        && approx_eq_f32(state.last_rotation[1], rotation.y)
        && approx_eq_f32(state.last_rotation[2], rotation.z)
        && approx_eq_f32(state.last_rotation[3], rotation.w)
        && approx_eq_f32(state.last_linear_velocity[0], linear_velocity.x)
        && approx_eq_f32(state.last_linear_velocity[1], linear_velocity.y)
        && approx_eq_f32(state.last_linear_velocity[2], linear_velocity.z)
        && approx_eq_f32(state.last_angular_velocity[0], angular_velocity.x)
        && approx_eq_f32(state.last_angular_velocity[1], angular_velocity.y)
        && approx_eq_f32(state.last_angular_velocity[2], angular_velocity.z)
}

fn update_body_sync_state_3d(
    state: &mut BodyState3D,
    position: Vector3,
    rotation: Quaternion,
    linear_velocity: Vector3,
    angular_velocity: Vector3,
    sleeping: bool,
    same_as_last_sync: bool,
) {
    state.last_translation = [position.x, position.y, position.z];
    state.last_rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
    state.last_linear_velocity = [linear_velocity.x, linear_velocity.y, linear_velocity.z];
    state.last_angular_velocity = [angular_velocity.x, angular_velocity.y, angular_velocity.z];
    state.idle_sync_frames = if sleeping && same_as_last_sync {
        state.idle_sync_frames.saturating_add(1)
    } else {
        0
    };
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
    if falloff <= 0.0 {
        1.0
    } else if (falloff - 1.0).abs() <= f32::EPSILON {
        t
    } else {
        t.powf(falloff)
    }
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
