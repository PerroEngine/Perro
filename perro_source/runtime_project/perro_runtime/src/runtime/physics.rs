use super::RuntimePhysicsStepTiming;
use crate::Runtime;
use ahash::AHashSet;
use perro_ids::{NodeID, SignalID};
#[cfg(test)]
use perro_nodes::TileMap2D;
use perro_nodes::{SceneNodeData, Shape2D, Shape3D, water_physics_sample_or_idle};
use perro_physics::*;
use perro_runtime_api::sub_apis::{
    NodeAPI, PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D,
    PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D, SignalAPI,
};
#[cfg(test)]
use perro_structs::BitMask;
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;

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

impl Runtime {
    pub fn set_physics_paused(&mut self, paused: bool) {
        self.physics.set_paused(paused);
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
                    shape_signature = hash_f32(shape_signature, water.water.size.x.to_bits());
                    shape_signature = hash_f32(shape_signature, water.water.size.y.to_bits());
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
                        shapes.push(ShapeDesc2D {
                            local: Transform2D::IDENTITY,
                            shape: ShapeKind2D::Primitive(Shape2D::Quad {
                                width: water.water.size.x,
                                height: water.water.size.y,
                            }),
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
                    shape_signature = hash_f32(shape_signature, water.water.size.x.to_bits());
                    shape_signature = hash_f32(shape_signature, water.water.size.y.to_bits());
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
                    shapes.push(ShapeDesc3D {
                        local: Transform3D::new(
                            Vector3::new(0.0, -water.water.depth * 0.5, 0.0),
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        ),
                        shape: ShapeKind3D::Primitive(Shape3D::Cube {
                            size: Vector3::new(
                                water.water.size.x,
                                water.water.depth.max(0.001),
                                water.water.size.y,
                            ),
                        }),
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

    fn queue_water_forces_2d(&mut self) {
        let water_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, scene_node)| {
                matches!(scene_node.data, SceneNodeData::WaterBody2D(_)).then_some(id)
            })
            .collect();
        let mut waters = Vec::new();
        for id in water_ids {
            let Some(transform) = self.get_global_transform_2d(id) else {
                continue;
            };
            let Some(scene_node) = self.nodes.get(id) else {
                continue;
            };
            let SceneNodeData::WaterBody2D(water) = &scene_node.data else {
                continue;
            };
            waters.push((id, transform.position, water.water));
        }
        if waters.is_empty() {
            return;
        }
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
        let mut forces = Vec::new();
        for body_id in body_ids {
            let Some(body_transform) = self.get_global_transform_2d(body_id) else {
                continue;
            };
            let Some(scene_node) = self.nodes.get(body_id) else {
                continue;
            };
            let SceneNodeData::RigidBody2D(body) = &scene_node.data else {
                continue;
            };
            for (water_id, water_pos, surface) in &waters {
                if surface.collision_mask.intersects(body.collision_layers)
                    || body.collision_mask.intersects(surface.collision_layers)
                {
                    continue;
                }
                let half = surface.size * 0.5;
                let local = body_transform.position - *water_pos;
                if local.x.abs() > half.x || local.y.abs() > half.y {
                    continue;
                }
                let sample = water_physics_sample_or_idle(
                    surface,
                    local,
                    self.time.elapsed,
                    self.water_samples.get(water_id).copied(),
                );
                let surface_y = water_pos.y + sample.height;
                let submerged = (surface_y - body_transform.position.y).max(0.0);
                if submerged <= 0.0 {
                    continue;
                }
                let buoyancy = submerged * surface.physics.buoyancy * body.density.max(0.0);
                let drag = -body.linear_velocity.y * surface.physics.drag;
                let (scale, deadzone) = water_force_lod(
                    surface.lod.near_distance,
                    surface.lod.mid_distance,
                    surface.lod.far_distance,
                    *water_pos,
                    camera_pos,
                );
                let force_y = (buoyancy + drag) * scale;
                if force_y.abs() >= deadzone {
                    forces.push((body_id, Vector2::new(0.0, force_y)));
                }
            }
        }
        for (body, force) in forces {
            self.physics.queue_force_2d(body, force);
        }
    }

    fn queue_water_forces_3d(&mut self) {
        let water_ids: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(id, scene_node)| {
                matches!(scene_node.data, SceneNodeData::WaterBody3D(_)).then_some(id)
            })
            .collect();
        let mut waters = Vec::new();
        for id in water_ids {
            let Some(transform) = self.get_global_transform_3d(id) else {
                continue;
            };
            let Some(scene_node) = self.nodes.get(id) else {
                continue;
            };
            let SceneNodeData::WaterBody3D(water) = &scene_node.data else {
                continue;
            };
            waters.push((id, transform.position, water.water));
        }
        if waters.is_empty() {
            return;
        }
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
        let mut forces = Vec::new();
        for body_id in body_ids {
            let Some(body_transform) = self.get_global_transform_3d(body_id) else {
                continue;
            };
            let Some(scene_node) = self.nodes.get(body_id) else {
                continue;
            };
            let SceneNodeData::RigidBody3D(body) = &scene_node.data else {
                continue;
            };
            for (water_id, water_pos, surface) in &waters {
                if surface.collision_mask.intersects(body.collision_layers)
                    || body.collision_mask.intersects(surface.collision_layers)
                {
                    continue;
                }
                let half = surface.size * 0.5;
                let local = Vector2::new(
                    body_transform.position.x - water_pos.x,
                    body_transform.position.z - water_pos.z,
                );
                if local.x.abs() > half.x || local.y.abs() > half.y {
                    continue;
                }
                let sample = water_physics_sample_or_idle(
                    surface,
                    local,
                    self.time.elapsed,
                    self.water_samples.get(water_id).copied(),
                );
                let surface_y = water_pos.y + sample.height;
                let submerged = (surface_y - body_transform.position.y).max(0.0);
                if submerged <= 0.0 {
                    continue;
                }
                let buoyancy = submerged * surface.physics.buoyancy * body.density.max(0.0);
                let drag = -body.linear_velocity.y * surface.physics.drag;
                let water_pos_2d = Vector2::new(water_pos.x, water_pos.z);
                let (scale, deadzone) = water_force_lod(
                    surface.lod.near_distance,
                    surface.lod.mid_distance,
                    surface.lod.far_distance,
                    water_pos_2d,
                    camera_pos,
                );
                let force_y = (buoyancy + drag) * scale;
                if force_y.abs() >= deadzone {
                    forces.push((body_id, Vector3::new(0.0, force_y, 0.0)));
                }
            }
        }
        for (body, force) in forces {
            self.physics.queue_force_3d(body, force);
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

#[cfg(test)]
mod tests;
