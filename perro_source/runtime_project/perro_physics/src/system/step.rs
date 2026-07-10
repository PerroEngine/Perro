use super::*;

impl PhysicsSystem {
    pub fn step_world_2d(&mut self, gravity_y: f32, fixed_delta: f32) {
        if self.world_2d.is_some() {
            self.query_pipeline_dirty_2d = true;
        }
        step_world_2d_slot(&mut self.world_2d, gravity_y, fixed_delta);
        self.refresh_world_2d_idle_cache();
    }

    pub fn step_world_3d(&mut self, gravity_y: f32, fixed_delta: f32) {
        if self.world_3d.is_some() {
            self.query_pipeline_dirty_3d = true;
        }
        step_world_3d_slot(&mut self.world_3d, gravity_y, fixed_delta);
        self.refresh_world_3d_idle_cache();
    }

    pub fn step_worlds_parallel(&mut self, gravity_y: f32, fixed_delta: f32) {
        if self.world_2d.is_none() || self.world_3d.is_none() {
            self.step_world_2d(gravity_y, fixed_delta);
            self.step_world_3d(gravity_y, fixed_delta);
            return;
        }
        self.query_pipeline_dirty_2d = true;
        self.query_pipeline_dirty_3d = true;
        let world_2d = &mut self.world_2d;
        let world_3d = &mut self.world_3d;
        rayon::join(
            || step_world_2d_slot(world_2d, gravity_y, fixed_delta),
            || step_world_3d_slot(world_3d, gravity_y, fixed_delta),
        );
        self.refresh_world_idle_cache();
    }

    pub fn apply_pending_impulses_2d(&mut self, coef: f32) {
        apply_pending_impulses_2d_parts(&mut self.world_2d, &mut self.pending_impulses_2d, coef);
    }

    pub fn apply_pending_forces_2d(&mut self, coef: f32, fixed_delta: f32) {
        apply_pending_forces_2d_parts(
            &mut self.world_2d,
            &mut self.pending_forces_2d,
            coef,
            fixed_delta,
        );
    }

    pub fn apply_pending_impulses_3d(&mut self, coef: f32) {
        apply_pending_impulses_3d_parts(&mut self.world_3d, &mut self.pending_impulses_3d, coef);
    }

    pub fn apply_pending_forces_3d(&mut self, coef: f32, fixed_delta: f32) {
        apply_pending_forces_3d_parts(
            &mut self.world_3d,
            &mut self.pending_forces_3d,
            coef,
            fixed_delta,
        );
    }

    pub fn apply_pending_forces_and_impulses_parallel(&mut self, coef: f32, fixed_delta: f32) {
        if self.world_2d.is_none() || self.world_3d.is_none() {
            self.apply_pending_forces_2d(coef, fixed_delta);
            self.apply_pending_forces_3d(coef, fixed_delta);
            self.apply_pending_impulses_2d(coef);
            self.apply_pending_impulses_3d(coef);
            return;
        }
        let world_2d = &mut self.world_2d;
        let pending_forces_2d = &mut self.pending_forces_2d;
        let pending_impulses_2d = &mut self.pending_impulses_2d;
        let world_3d = &mut self.world_3d;
        let pending_forces_3d = &mut self.pending_forces_3d;
        let pending_impulses_3d = &mut self.pending_impulses_3d;
        rayon::join(
            || {
                apply_pending_forces_2d_parts(world_2d, pending_forces_2d, coef, fixed_delta);
                apply_pending_impulses_2d_parts(world_2d, pending_impulses_2d, coef);
            },
            || {
                apply_pending_forces_3d_parts(world_3d, pending_forces_3d, coef, fixed_delta);
                apply_pending_impulses_3d_parts(world_3d, pending_impulses_3d, coef);
            },
        );
    }
}

fn step_world_2d_slot(world: &mut Option<PhysicsWorld2D>, gravity_y: f32, fixed_delta: f32) {
    let Some(world) = world.as_mut() else {
        return;
    };
    world.gravity.y = gravity_y;
    world.integration_parameters.dt = fixed_delta.max(0.000_1);
    world.pipeline.step(
        &world.gravity,
        &world.integration_parameters,
        &mut world.islands,
        &mut world.broad_phase,
        &mut world.narrow_phase,
        &mut world.bodies,
        &mut world.colliders,
        &mut world.impulse_joints,
        &mut world.multibody_joints,
        &mut world.ccd_solver,
        None,
        &(),
        &(),
    );
}

fn step_world_3d_slot(world: &mut Option<PhysicsWorld3D>, gravity_y: f32, fixed_delta: f32) {
    let Some(world) = world.as_mut() else {
        return;
    };
    world.gravity.y = gravity_y;
    world.integration_parameters.dt = fixed_delta.max(0.000_1);
    world.pipeline.step(
        &world.gravity,
        &world.integration_parameters,
        &mut world.islands,
        &mut world.broad_phase,
        &mut world.narrow_phase,
        &mut world.bodies,
        &mut world.colliders,
        &mut world.impulse_joints,
        &mut world.multibody_joints,
        &mut world.ccd_solver,
        None,
        &(),
        &(),
    );
}

fn apply_pending_impulses_2d_parts(
    world: &mut Option<PhysicsWorld2D>,
    pending: &mut Vec<PendingImpulse2D>,
    coef: f32,
) {
    let mut pending_taken = std::mem::take(pending);
    let Some(world) = world.as_mut() else {
        *pending = pending_taken;
        return;
    };
    for impulse in pending_taken.drain(..) {
        let Some(state) = world.body_map.get(&impulse.id) else {
            continue;
        };
        if state.kind != crate::BodyKind::Rigid {
            continue;
        }
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            continue;
        };
        let x = impulse.impulse.x * coef;
        let y = impulse.impulse.y * coef;
        let len_sq = x * x + y * y;
        if !len_sq.is_finite() || len_sq <= 0.000_001 {
            continue;
        }
        rb.apply_impulse(na2::Vector2::new(x, y), true);
        clamp_rb_speed_2d(rb, MAX_RIGID_SPEED_2D);
    }
    *pending = pending_taken;
}

fn apply_pending_forces_2d_parts(
    world: &mut Option<PhysicsWorld2D>,
    pending: &mut Vec<PendingForce2D>,
    coef: f32,
    fixed_delta: f32,
) {
    let mut pending_taken = std::mem::take(pending);
    let Some(world) = world.as_mut() else {
        *pending = pending_taken;
        return;
    };
    let dt = fixed_delta.max(0.000_1);
    for force in pending_taken.drain(..) {
        let Some(state) = world.body_map.get(&force.id) else {
            continue;
        };
        if state.kind != crate::BodyKind::Rigid {
            continue;
        }
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            continue;
        };
        let x = force.force.x * dt * coef;
        let y = force.force.y * dt * coef;
        let len_sq = x * x + y * y;
        if !len_sq.is_finite() || len_sq <= 0.000_001 {
            continue;
        }
        rb.apply_impulse(na2::Vector2::new(x, y), true);
        clamp_rb_speed_2d(rb, MAX_RIGID_SPEED_2D);
    }
    *pending = pending_taken;
}

fn apply_pending_impulses_3d_parts(
    world: &mut Option<PhysicsWorld3D>,
    pending: &mut Vec<PendingImpulse3D>,
    coef: f32,
) {
    let mut pending_taken = std::mem::take(pending);
    let Some(world) = world.as_mut() else {
        *pending = pending_taken;
        return;
    };
    for impulse in pending_taken.drain(..) {
        let Some(state) = world.body_map.get(&impulse.id) else {
            continue;
        };
        if state.kind != crate::BodyKind::Rigid {
            continue;
        }
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            continue;
        };
        let x = impulse.impulse.x * coef;
        let y = impulse.impulse.y * coef;
        let z = impulse.impulse.z * coef;
        let len_sq = x * x + y * y + z * z;
        if !len_sq.is_finite() || len_sq <= 0.000_001 {
            continue;
        }
        rb.apply_impulse(na3::Vector3::new(x, y, z), true);
        clamp_rb_speed_3d(rb, MAX_RIGID_SPEED_3D);
    }
    *pending = pending_taken;
}

fn apply_pending_forces_3d_parts(
    world: &mut Option<PhysicsWorld3D>,
    pending: &mut Vec<PendingForce3D>,
    coef: f32,
    fixed_delta: f32,
) {
    let mut pending_taken = std::mem::take(pending);
    let Some(world) = world.as_mut() else {
        *pending = pending_taken;
        return;
    };
    let dt = fixed_delta.max(0.000_1);
    for force in pending_taken.drain(..) {
        let Some(state) = world.body_map.get(&force.id) else {
            continue;
        };
        if state.kind != crate::BodyKind::Rigid {
            continue;
        }
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            continue;
        };
        let x = force.force.x * dt * coef;
        let y = force.force.y * dt * coef;
        let z = force.force.z * dt * coef;
        let len_sq = x * x + y * y + z * z;
        if !len_sq.is_finite() || len_sq <= 0.000_001 {
            continue;
        }
        rb.apply_impulse(na3::Vector3::new(x, y, z), true);
        clamp_rb_speed_3d(rb, MAX_RIGID_SPEED_3D);
    }
    *pending = pending_taken;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BodyDesc2D, BodyDesc3D, BodyKind, PhysicsAssetContext, PhysicsProviderMode, RigidProps2D,
        RigidProps3D, ShapeDesc2D, ShapeDesc3D, ShapeKind2D, ShapeKind3D,
    };
    use perro_nodes::{Shape2D, Shape3D};
    use perro_structs::{BitMask, Quaternion, Transform2D, Transform3D};

    fn asset_context() -> PhysicsAssetContext {
        PhysicsAssetContext {
            provider_mode: PhysicsProviderMode::Dynamic,
            static_mesh_lookup: None,
            static_collision_trimesh_lookup: None,
        }
    }

    fn shape_2d() -> ShapeDesc2D {
        ShapeDesc2D {
            local: Transform2D::IDENTITY,
            shape: ShapeKind2D::Primitive(Shape2D::Circle { radius: 0.35 }),
            sensor: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            friction: 0.7,
            restitution: 0.1,
            density: 1.0,
        }
    }

    fn shape_3d() -> ShapeDesc3D {
        ShapeDesc3D {
            local: Transform3D::IDENTITY,
            shape: ShapeKind3D::Primitive(Shape3D::Sphere { radius: 0.35 }),
            sensor: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            friction: 0.7,
            restitution: 0.1,
            density: 1.0,
        }
    }

    fn body_2d(id: NodeID, x: f32) -> BodyDesc2D {
        BodyDesc2D {
            id,
            kind: BodyKind::Rigid,
            enabled: true,
            global: Transform2D::new(Vector2::new(x, 2.0), 0.0, Vector2::ONE),
            rigid: Some(RigidProps2D {
                enabled: true,
                can_sleep: false,
                lock_rotation: false,
                mass: 1.0,
                density: 1.0,
                continuous_collision_detection: false,
                linear_velocity: Vector2::new(0.1, 0.0),
                angular_velocity: 0.0,
                gravity_scale: 1.0,
                linear_damping: 0.01,
                angular_damping: 0.01,
            }),
            sync_signature: id.as_u64(),
            shape_signature: 2,
            shapes: vec![shape_2d()],
        }
    }

    fn body_3d(id: NodeID, x: f32) -> BodyDesc3D {
        BodyDesc3D {
            id,
            kind: BodyKind::Rigid,
            enabled: true,
            global: Transform3D::new(Vector3::new(x, 2.0, x), Quaternion::IDENTITY, Vector3::ONE),
            rigid: Some(RigidProps3D {
                enabled: true,
                can_sleep: false,
                mass: 1.0,
                density: 1.0,
                continuous_collision_detection: false,
                linear_velocity: Vector3::new(0.1, 0.0, -0.1),
                angular_velocity: Vector3::ZERO,
                gravity_scale: 1.0,
                linear_damping: 0.01,
                angular_damping: 0.01,
            }),
            sync_signature: id.as_u64(),
            shape_signature: 2,
            shapes: vec![shape_3d()],
        }
    }

    fn mixed_system() -> PhysicsSystem {
        let mut system = PhysicsSystem::new();
        let bodies_2d = vec![
            BodyDesc2D {
                id: NodeID::new(1),
                kind: BodyKind::Static,
                enabled: true,
                global: Transform2D::new(Vector2::new(0.0, -8.0), 0.0, Vector2::ONE),
                rigid: None,
                sync_signature: 1,
                shape_signature: 1,
                shapes: vec![ShapeDesc2D {
                    shape: ShapeKind2D::Primitive(Shape2D::Quad {
                        width: 16.0,
                        height: 1.0,
                    }),
                    ..shape_2d()
                }],
            },
            body_2d(NodeID::new(2), -1.0),
            body_2d(NodeID::new(3), 1.0),
        ];
        let bodies_3d = vec![
            BodyDesc3D {
                id: NodeID::new(10),
                kind: BodyKind::Static,
                enabled: true,
                global: Transform3D::new(
                    Vector3::new(0.0, -8.0, 0.0),
                    Quaternion::IDENTITY,
                    Vector3::ONE,
                ),
                rigid: None,
                sync_signature: 10,
                shape_signature: 1,
                shapes: vec![ShapeDesc3D {
                    shape: ShapeKind3D::Primitive(Shape3D::Cube {
                        size: Vector3::new(16.0, 1.0, 16.0),
                    }),
                    ..shape_3d()
                }],
            },
            body_3d(NodeID::new(11), -1.0),
            body_3d(NodeID::new(12), 1.0),
        ];
        system.sync_world_2d(&bodies_2d, |_, _| {});
        system.sync_world_3d(&bodies_3d, asset_context(), |_, _| {});
        system
    }

    fn queue_test_inputs(system: &mut PhysicsSystem) {
        for id in [NodeID::new(2), NodeID::new(3)] {
            system.queue_force_2d(id, Vector2::new(0.4, 0.1));
            system.queue_impulse_2d(id, Vector2::new(0.02, 0.01));
        }
        for id in [NodeID::new(11), NodeID::new(12)] {
            system.queue_force_3d(id, Vector3::new(0.4, 0.1, -0.2));
            system.queue_impulse_3d(id, Vector3::new(0.02, 0.01, 0.03));
        }
    }

    #[test]
    fn force_queue_and_step_reject_nonfinite_values() {
        let mut system = mixed_system();
        let id_2d = NodeID::new(2);
        let id_3d = NodeID::new(11);

        system.queue_force_2d(id_2d, Vector2::new(f32::NAN, 1.0));
        system.queue_impulse_2d(id_2d, Vector2::new(1.0, f32::INFINITY));
        system.queue_force_3d(id_3d, Vector3::new(1.0, f32::NAN, 1.0));
        system.queue_impulse_3d(id_3d, Vector3::new(f32::NEG_INFINITY, 1.0, 1.0));
        assert!(system.pending_forces_2d.is_empty());
        assert!(system.pending_impulses_2d.is_empty());
        assert!(system.pending_forces_3d.is_empty());
        assert!(system.pending_impulses_3d.is_empty());

        system.queue_force_2d(id_2d, Vector2::new(1.0, 1.0));
        system.queue_force_3d(id_3d, Vector3::new(1.0, 1.0, 1.0));
        system.apply_pending_forces_2d(f32::NAN, 1.0 / 60.0);
        system.apply_pending_forces_3d(f32::NAN, 1.0 / 60.0);
        let body_2d = system.world_2d.as_ref().unwrap();
        let body_2d = body_2d.bodies.get(body_2d.body_map[&id_2d].handle).unwrap();
        let body_3d = system.world_3d.as_ref().unwrap();
        let body_3d = body_3d.bodies.get(body_3d.body_map[&id_3d].handle).unwrap();
        assert!(body_2d.linvel().iter().all(|value| value.is_finite()));
        assert!(body_3d.linvel().iter().all(|value| value.is_finite()));
    }

    fn pose_2d(system: &PhysicsSystem, id: NodeID) -> [f32; 2] {
        let world = system.world_2d.as_ref().unwrap();
        let state = world.body_map.get(&id).unwrap();
        let body = world.bodies.get(state.handle).unwrap();
        [body.translation().x, body.translation().y]
    }

    fn pose_3d(system: &PhysicsSystem, id: NodeID) -> [f32; 3] {
        let world = system.world_3d.as_ref().unwrap();
        let state = world.body_map.get(&id).unwrap();
        let body = world.bodies.get(state.handle).unwrap();
        [
            body.translation().x,
            body.translation().y,
            body.translation().z,
        ]
    }

    #[test]
    fn parallel_mixed_step_matches_serial() {
        let mut serial = mixed_system();
        let mut parallel = mixed_system();
        queue_test_inputs(&mut serial);
        queue_test_inputs(&mut parallel);

        serial.apply_pending_forces_2d(1.0, 1.0 / 60.0);
        serial.apply_pending_forces_3d(1.0, 1.0 / 60.0);
        serial.apply_pending_impulses_2d(1.0);
        serial.apply_pending_impulses_3d(1.0);
        serial.step_world_2d(-9.81, 1.0 / 60.0);
        serial.step_world_3d(-9.81, 1.0 / 60.0);

        parallel.apply_pending_forces_and_impulses_parallel(1.0, 1.0 / 60.0);
        parallel.step_worlds_parallel(-9.81, 1.0 / 60.0);

        for id in [NodeID::new(2), NodeID::new(3)] {
            assert_eq!(pose_2d(&parallel, id), pose_2d(&serial, id));
        }
        for id in [NodeID::new(11), NodeID::new(12)] {
            assert_eq!(pose_3d(&parallel, id), pose_3d(&serial, id));
        }
    }
}
