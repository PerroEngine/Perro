use super::*;

impl PhysicsSystem {
    pub fn step_world_2d(&mut self, gravity_y: f32, fixed_delta: f32) {
        let Some(world) = self.world_2d.as_mut() else {
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

    pub fn step_world_3d(&mut self, gravity_y: f32, fixed_delta: f32) {
        let Some(world) = self.world_3d.as_mut() else {
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

    pub fn apply_pending_impulses_2d(&mut self, coef: f32) {
        let mut pending = std::mem::take(&mut self.pending_impulses_2d);
        let Some(world) = self.world_2d.as_mut() else {
            return;
        };
        for impulse in pending.drain(..) {
            let Some(state) = world.body_map.get(&impulse.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq =
                impulse.impulse.x * impulse.impulse.x + impulse.impulse.y * impulse.impulse.y;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na2::Vector2::new(impulse.impulse.x * coef, impulse.impulse.y * coef),
                true,
            );
            clamp_rb_speed_2d(rb, MAX_RIGID_SPEED_2D);
        }
        self.pending_impulses_2d = pending;
    }

    pub fn apply_pending_forces_2d(&mut self, coef: f32, fixed_delta: f32) {
        let mut pending = std::mem::take(&mut self.pending_forces_2d);
        let Some(world) = self.world_2d.as_mut() else {
            return;
        };
        let dt = fixed_delta.max(0.000_1);
        for force in pending.drain(..) {
            let Some(state) = world.body_map.get(&force.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq = force.force.x * force.force.x + force.force.y * force.force.y;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na2::Vector2::new(force.force.x * dt * coef, force.force.y * dt * coef),
                true,
            );
            clamp_rb_speed_2d(rb, MAX_RIGID_SPEED_2D);
        }
        self.pending_forces_2d = pending;
    }

    pub fn apply_pending_impulses_3d(&mut self, coef: f32) {
        let mut pending = std::mem::take(&mut self.pending_impulses_3d);
        let Some(world) = self.world_3d.as_mut() else {
            return;
        };
        for impulse in pending.drain(..) {
            let Some(state) = world.body_map.get(&impulse.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq = impulse.impulse.x * impulse.impulse.x
                + impulse.impulse.y * impulse.impulse.y
                + impulse.impulse.z * impulse.impulse.z;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na3::Vector3::new(
                    impulse.impulse.x * coef,
                    impulse.impulse.y * coef,
                    impulse.impulse.z * coef,
                ),
                true,
            );
            clamp_rb_speed_3d(rb, MAX_RIGID_SPEED_3D);
        }
        self.pending_impulses_3d = pending;
    }

    pub fn apply_pending_forces_3d(&mut self, coef: f32, fixed_delta: f32) {
        let mut pending = std::mem::take(&mut self.pending_forces_3d);
        let Some(world) = self.world_3d.as_mut() else {
            return;
        };
        let dt = fixed_delta.max(0.000_1);
        for force in pending.drain(..) {
            let Some(state) = world.body_map.get(&force.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq = force.force.x * force.force.x
                + force.force.y * force.force.y
                + force.force.z * force.force.z;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na3::Vector3::new(
                    force.force.x * dt * coef,
                    force.force.y * dt * coef,
                    force.force.z * dt * coef,
                ),
                true,
            );
            clamp_rb_speed_3d(rb, MAX_RIGID_SPEED_3D);
        }
        self.pending_forces_3d = pending;
    }
}
