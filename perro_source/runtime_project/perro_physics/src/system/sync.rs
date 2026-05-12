use super::*;

impl PhysicsSystem {
    pub fn sync_world_2d(
        &mut self,
        bodies: &[crate::BodyDesc2D],
        mut set_body_handle: impl FnMut(NodeID, Option<u64>),
    ) {
        if bodies.is_empty() {
            if let Some(world) = self.world_2d.take() {
                for id in world.body_map.keys().copied() {
                    set_body_handle(id, None);
                }
            }
            return;
        }

        let mut world = self.world_2d.take().unwrap_or_default();
        let mut alive = AHashSet::default();
        for body in bodies {
            alive.insert(body.id);
            if !world.body_map.contains_key(&body.id) {
                let rb_handle = world.bodies.insert(build_rigid_body_2d(body));
                let opaque = self.alloc_opaque_handle();
                world.body_map.insert(
                    body.id,
                    crate::BodyState2D {
                        handle: rb_handle,
                        colliders: Vec::new(),
                        kind: body.kind,
                        shape_signature: 0,
                        opaque_handle: opaque,
                    },
                );
                set_body_handle(body.id, Some(opaque));
            }

            let Some(state) = world.body_map.get_mut(&body.id) else {
                continue;
            };

            state.kind = body.kind;
            if let Some(rb) = world.bodies.get_mut(state.handle) {
                rb.set_enabled(body.enabled);
                let target_body_type = match body.kind {
                    crate::BodyKind::Static | crate::BodyKind::Area => r2::RigidBodyType::Fixed,
                    crate::BodyKind::Rigid => r2::RigidBodyType::Dynamic,
                };
                if rb.body_type() != target_body_type {
                    rb.set_body_type(target_body_type, true);
                }

                let target_pos = transform_to_iso2(body.global);
                let current_pos = rb.position();
                let pos_changed =
                    !approx_eq_f32(
                        current_pos.translation.vector.x,
                        target_pos.translation.vector.x,
                    ) || !approx_eq_f32(
                        current_pos.translation.vector.y,
                        target_pos.translation.vector.y,
                    ) || !approx_eq_f32(current_pos.rotation.angle(), target_pos.rotation.angle());
                if pos_changed {
                    rb.set_position(target_pos, true);
                }

                if let Some(rigid) = body.rigid {
                    let target_lin =
                        na2::Vector2::new(rigid.linear_velocity.x, rigid.linear_velocity.y);
                    let current_lin = rb.linvel();
                    if !approx_eq_f32(current_lin.x, target_lin.x)
                        || !approx_eq_f32(current_lin.y, target_lin.y)
                    {
                        rb.set_linvel(target_lin, true);
                    }
                    if !approx_eq_f32(rb.angvel(), rigid.angular_velocity) {
                        rb.set_angvel(rigid.angular_velocity, true);
                    }
                    if !approx_eq_f32(rb.gravity_scale(), rigid.gravity_scale) {
                        rb.set_gravity_scale(rigid.gravity_scale, true);
                    }
                    if !approx_eq_f32(rb.linear_damping(), rigid.linear_damping) {
                        rb.set_linear_damping(rigid.linear_damping);
                    }
                    if !approx_eq_f32(rb.angular_damping(), rigid.angular_damping) {
                        rb.set_angular_damping(rigid.angular_damping);
                    }
                    let target_speed_sq = target_lin.norm_squared();
                    let target_ccd = rigid.continuous_collision_detection
                        && target_speed_sq >= CCD_MIN_SPEED_SQ_2D;
                    if rb.is_ccd_enabled() != target_ccd {
                        rb.enable_ccd(target_ccd);
                    }
                } else if rb.is_ccd_enabled() {
                    rb.enable_ccd(false);
                }
            }

            if state.shape_signature != body.shape_signature {
                for handle in state.colliders.drain(..) {
                    world.collider_owners.remove(&handle);
                    let _ =
                        world
                            .colliders
                            .remove(handle, &mut world.islands, &mut world.bodies, true);
                }

                for shape in &body.shapes {
                    let Some(builder) = collider_builder_2d(shape) else {
                        continue;
                    };
                    let handle = world.colliders.insert_with_parent(
                        builder,
                        state.handle,
                        &mut world.bodies,
                    );
                    world.collider_owners.insert(handle, body.id);
                    state.colliders.push(handle);
                }
                state.shape_signature = body.shape_signature;
            }
        }

        let mut stale = std::mem::take(&mut self.stale_ids_2d);
        stale.clear();
        stale.extend(
            world
                .body_map
                .keys()
                .copied()
                .filter(|id| !alive.contains(id)),
        );

        for id in stale.iter().copied() {
            if let Some(state) = world.body_map.remove(&id) {
                for handle in &state.colliders {
                    world.collider_owners.remove(handle);
                }
                let _ = world.bodies.remove(
                    state.handle,
                    &mut world.islands,
                    &mut world.colliders,
                    &mut world.impulse_joints,
                    &mut world.multibody_joints,
                    true,
                );
            }
            set_body_handle(id, None);
        }
        stale.clear();
        self.stale_ids_2d = stale;
        self.world_2d = Some(world);
    }

    pub fn sync_world_3d(
        &mut self,
        bodies: &[crate::BodyDesc3D],
        assets: PhysicsAssetContext,
        mut set_body_handle: impl FnMut(NodeID, Option<u64>),
    ) {
        if bodies.is_empty() {
            if let Some(world) = self.world_3d.take() {
                for id in world.body_map.keys().copied() {
                    set_body_handle(id, None);
                }
            }
            return;
        }

        let mut world = self.world_3d.take().unwrap_or_default();
        let mut alive = AHashSet::default();
        for body in bodies {
            alive.insert(body.id);
            if !world.body_map.contains_key(&body.id) {
                let rb_handle = world.bodies.insert(build_rigid_body_3d(body));
                let opaque = self.alloc_opaque_handle();
                world.body_map.insert(
                    body.id,
                    crate::BodyState3D {
                        handle: rb_handle,
                        colliders: Vec::new(),
                        kind: body.kind,
                        shape_signature: 0,
                        opaque_handle: opaque,
                    },
                );
                set_body_handle(body.id, Some(opaque));
            }

            let Some(state) = world.body_map.get_mut(&body.id) else {
                continue;
            };

            state.kind = body.kind;
            if let Some(rb) = world.bodies.get_mut(state.handle) {
                rb.set_enabled(body.enabled);
                let target_body_type = match body.kind {
                    crate::BodyKind::Static | crate::BodyKind::Area => r3::RigidBodyType::Fixed,
                    crate::BodyKind::Rigid => r3::RigidBodyType::Dynamic,
                };
                if rb.body_type() != target_body_type {
                    rb.set_body_type(target_body_type, true);
                }

                let target_pos = transform_to_iso3(body.global);
                let current_pos = rb.position();
                let pos_changed =
                    !approx_eq_f32(
                        current_pos.translation.vector.x,
                        target_pos.translation.vector.x,
                    ) || !approx_eq_f32(
                        current_pos.translation.vector.y,
                        target_pos.translation.vector.y,
                    ) || !approx_eq_f32(
                        current_pos.translation.vector.z,
                        target_pos.translation.vector.z,
                    ) || !approx_eq_f32(current_pos.rotation.i, target_pos.rotation.i)
                        || !approx_eq_f32(current_pos.rotation.j, target_pos.rotation.j)
                        || !approx_eq_f32(current_pos.rotation.k, target_pos.rotation.k)
                        || !approx_eq_f32(current_pos.rotation.w, target_pos.rotation.w);
                if pos_changed {
                    rb.set_position(target_pos, true);
                }

                if let Some(rigid) = body.rigid {
                    let target_lin = na3::Vector3::new(
                        rigid.linear_velocity.x,
                        rigid.linear_velocity.y,
                        rigid.linear_velocity.z,
                    );
                    let current_lin = rb.linvel();
                    if !approx_eq_f32(current_lin.x, target_lin.x)
                        || !approx_eq_f32(current_lin.y, target_lin.y)
                        || !approx_eq_f32(current_lin.z, target_lin.z)
                    {
                        rb.set_linvel(target_lin, true);
                    }

                    let target_ang = na3::Vector3::new(
                        rigid.angular_velocity.x,
                        rigid.angular_velocity.y,
                        rigid.angular_velocity.z,
                    );
                    let current_ang = rb.angvel();
                    if !approx_eq_f32(current_ang.x, target_ang.x)
                        || !approx_eq_f32(current_ang.y, target_ang.y)
                        || !approx_eq_f32(current_ang.z, target_ang.z)
                    {
                        rb.set_angvel(target_ang, true);
                    }
                    if !approx_eq_f32(rb.gravity_scale(), rigid.gravity_scale) {
                        rb.set_gravity_scale(rigid.gravity_scale, true);
                    }
                    if !approx_eq_f32(rb.linear_damping(), rigid.linear_damping) {
                        rb.set_linear_damping(rigid.linear_damping);
                    }
                    if !approx_eq_f32(rb.angular_damping(), rigid.angular_damping) {
                        rb.set_angular_damping(rigid.angular_damping);
                    }
                    rb.set_additional_mass(rigid.mass.max(0.0), true);
                    let target_speed_sq = target_lin.norm_squared();
                    let target_ccd = rigid.continuous_collision_detection
                        && target_speed_sq >= CCD_MIN_SPEED_SQ_3D;
                    if rb.is_ccd_enabled() != target_ccd {
                        rb.enable_ccd(target_ccd);
                    }
                } else if rb.is_ccd_enabled() {
                    rb.enable_ccd(false);
                }
            }

            if state.shape_signature != body.shape_signature {
                for handle in state.colliders.drain(..) {
                    world.collider_owners.remove(&handle);
                    let _ =
                        world
                            .colliders
                            .remove(handle, &mut world.islands, &mut world.bodies, true);
                }

                for shape in &body.shapes {
                    let Some(builder) = collider_builder_3d(
                        shape,
                        assets.provider_mode,
                        assets.static_mesh_lookup,
                        assets.static_collision_trimesh_lookup,
                        &mut self.trimesh_cache,
                    ) else {
                        continue;
                    };
                    let handle = world.colliders.insert_with_parent(
                        builder,
                        state.handle,
                        &mut world.bodies,
                    );
                    world.collider_owners.insert(handle, body.id);
                    state.colliders.push(handle);
                }
                state.shape_signature = body.shape_signature;
            }
        }

        let mut stale = std::mem::take(&mut self.stale_ids_3d);
        stale.clear();
        stale.extend(
            world
                .body_map
                .keys()
                .copied()
                .filter(|id| !alive.contains(id)),
        );

        for id in stale.iter().copied() {
            if let Some(state) = world.body_map.remove(&id) {
                for handle in &state.colliders {
                    world.collider_owners.remove(handle);
                }
                let _ = world.bodies.remove(
                    state.handle,
                    &mut world.islands,
                    &mut world.colliders,
                    &mut world.impulse_joints,
                    &mut world.multibody_joints,
                    true,
                );
            }
            set_body_handle(id, None);
        }
        stale.clear();
        self.stale_ids_3d = stale;
        self.world_3d = Some(world);
    }

    pub fn sync_joints_2d(&mut self, joints: &[crate::JointDesc2D]) {
        let mut stale = std::mem::take(&mut self.stale_joint_ids_2d);
        stale.clear();
        let next_epoch = self.joint_sync_epoch_2d.wrapping_add(1);
        let reset_epochs = next_epoch == 0;
        self.joint_sync_epoch_2d = if reset_epochs { 1 } else { next_epoch };
        let sync_epoch = self.joint_sync_epoch_2d;

        let Some(world) = self.world_2d.as_mut() else {
            self.stale_joint_ids_2d = stale;
            return;
        };
        if reset_epochs {
            for state in world.joint_map.values_mut() {
                state.sync_epoch = 0;
            }
        }
        for joint in joints {
            if !joint.enabled || joint.body_a.is_nil() || joint.body_b.is_nil() {
                remove_joint_2d(world, joint.id);
                continue;
            }
            let Some(body_a) = world.body_map.get(&joint.body_a).map(|state| state.handle) else {
                remove_joint_2d(world, joint.id);
                continue;
            };
            let Some(body_b) = world.body_map.get(&joint.body_b).map(|state| state.handle) else {
                remove_joint_2d(world, joint.id);
                continue;
            };
            if let Some(state) = world.joint_map.get_mut(&joint.id)
                && state.signature == joint.signature
            {
                state.sync_epoch = sync_epoch;
                continue;
            }
            remove_joint_2d(world, joint.id);
            let data = build_joint_2d(joint);
            let handle = world.impulse_joints.insert(body_a, body_b, data, true);
            world.joint_map.insert(
                joint.id,
                crate::JointState2D {
                    handle,
                    signature: joint.signature,
                    sync_epoch,
                },
            );
        }

        stale.extend(world.joint_map.keys().copied().filter(|id| {
            world
                .joint_map
                .get(id)
                .is_some_and(|state| state.sync_epoch != sync_epoch)
        }));
        for id in stale.iter().copied() {
            remove_joint_2d(world, id);
        }
        stale.clear();
        self.stale_joint_ids_2d = stale;
    }

    pub fn sync_joints_3d(&mut self, joints: &[crate::JointDesc3D]) {
        let mut stale = std::mem::take(&mut self.stale_joint_ids_3d);
        stale.clear();
        let next_epoch = self.joint_sync_epoch_3d.wrapping_add(1);
        let reset_epochs = next_epoch == 0;
        self.joint_sync_epoch_3d = if reset_epochs { 1 } else { next_epoch };
        let sync_epoch = self.joint_sync_epoch_3d;

        let Some(world) = self.world_3d.as_mut() else {
            self.stale_joint_ids_3d = stale;
            return;
        };
        if reset_epochs {
            for state in world.joint_map.values_mut() {
                state.sync_epoch = 0;
            }
        }
        for joint in joints {
            if !joint.enabled || joint.body_a.is_nil() || joint.body_b.is_nil() {
                remove_joint_3d(world, joint.id);
                continue;
            }
            let Some(body_a) = world.body_map.get(&joint.body_a).map(|state| state.handle) else {
                remove_joint_3d(world, joint.id);
                continue;
            };
            let Some(body_b) = world.body_map.get(&joint.body_b).map(|state| state.handle) else {
                remove_joint_3d(world, joint.id);
                continue;
            };
            if let Some(state) = world.joint_map.get_mut(&joint.id)
                && state.signature == joint.signature
            {
                state.sync_epoch = sync_epoch;
                continue;
            }
            remove_joint_3d(world, joint.id);
            let data = build_joint_3d(joint);
            let handle = world.impulse_joints.insert(body_a, body_b, data, true);
            world.joint_map.insert(
                joint.id,
                crate::JointState3D {
                    handle,
                    signature: joint.signature,
                    sync_epoch,
                },
            );
        }

        stale.extend(world.joint_map.keys().copied().filter(|id| {
            world
                .joint_map
                .get(id)
                .is_some_and(|state| state.sync_epoch != sync_epoch)
        }));
        for id in stale.iter().copied() {
            remove_joint_3d(world, id);
        }
        stale.clear();
        self.stale_joint_ids_3d = stale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

    fn body_2d(id: u32) -> crate::BodyDesc2D {
        crate::BodyDesc2D {
            id: NodeID::new(id),
            kind: crate::BodyKind::Rigid,
            enabled: true,
            global: Transform2D::IDENTITY,
            rigid: None,
            shape_signature: 0,
            shapes: Vec::new(),
        }
    }

    fn body_3d(id: u32) -> crate::BodyDesc3D {
        crate::BodyDesc3D {
            id: NodeID::new(id),
            kind: crate::BodyKind::Rigid,
            enabled: true,
            global: Transform3D::new(Vector3::ZERO, Quaternion::IDENTITY, Vector3::ONE),
            rigid: None,
            shape_signature: 0,
            shapes: Vec::new(),
        }
    }

    fn asset_context() -> PhysicsAssetContext {
        PhysicsAssetContext {
            provider_mode: PhysicsProviderMode::Dynamic,
            static_mesh_lookup: None,
            static_collision_trimesh_lookup: None,
        }
    }

    fn joint_2d(id: u32, body_a: u32, body_b: u32) -> crate::JointDesc2D {
        let id = NodeID::new(id);
        let body_a = NodeID::new(body_a);
        let body_b = NodeID::new(body_b);
        let anchor_a = Vector2::ZERO;
        let anchor_b = Vector2::ZERO;
        let kind = crate::JointKind2D::Fixed;
        crate::JointDesc2D {
            id,
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            enabled: true,
            collide_connected: false,
            kind,
            signature: joint_signature_2d(body_a, body_b, anchor_a, anchor_b, true, false, kind),
        }
    }

    fn joint_3d(id: u32, body_a: u32, body_b: u32) -> crate::JointDesc3D {
        let id = NodeID::new(id);
        let body_a = NodeID::new(body_a);
        let body_b = NodeID::new(body_b);
        let anchor_a = Vector3::ZERO;
        let anchor_b = Vector3::ZERO;
        let kind = crate::JointKind3D::Fixed;
        crate::JointDesc3D {
            id,
            body_a,
            body_b,
            anchor_a,
            anchor_b,
            enabled: true,
            collide_connected: false,
            kind,
            signature: joint_signature_3d(body_a, body_b, anchor_a, anchor_b, true, false, kind),
        }
    }

    #[test]
    fn joint_sync_reuses_stale_scratch_after_stale_remove() {
        let mut system = PhysicsSystem::new();
        let bodies = [body_2d(1), body_2d(2), body_2d(3)];
        system.sync_world_2d(&bodies, |_, _| {});

        let joints = [joint_2d(10, 1, 2), joint_2d(11, 2, 3)];
        system.sync_joints_2d(&joints);
        let stale_capacity = system.stale_joint_ids_2d.capacity();

        system.sync_joints_2d(&joints[..1]);

        assert_eq!(
            system.world_2d.as_ref().map(|world| world.joint_map.len()),
            Some(1)
        );
        assert!(system.stale_joint_ids_2d.capacity() >= stale_capacity);
    }

    #[test]
    fn joint_sync_3d_reuses_stale_scratch_after_stale_remove() {
        let mut system = PhysicsSystem::new();
        let bodies = [body_3d(1), body_3d(2), body_3d(3)];
        system.sync_world_3d(&bodies, asset_context(), |_, _| {});

        let joints = [joint_3d(10, 1, 2), joint_3d(11, 2, 3)];
        system.sync_joints_3d(&joints);
        let stale_capacity = system.stale_joint_ids_3d.capacity();

        system.sync_joints_3d(&joints[..1]);

        assert_eq!(
            system.world_3d.as_ref().map(|world| world.joint_map.len()),
            Some(1)
        );
        assert!(system.stale_joint_ids_3d.capacity() >= stale_capacity);
    }
}
