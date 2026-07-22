use super::*;

impl Runtime {
    pub(super) fn collect_body_descs_2d(&mut self) -> Vec<BodyDesc2D> {
        #[cfg(any(test, feature = "bench"))]
        self.physics_collect_calls_2d
            .set(self.physics_collect_calls_2d.get() + 1);
        let node_count = self.internal_updates.physics_body_nodes_2d.len();
        let mut out = std::mem::take(&mut self.physics_body_descs_2d);
        out.clear();
        if out.capacity() < node_count {
            out.reserve(node_count - out.capacity());
        }
        for i in 0..node_count {
            let id = self.internal_updates.physics_body_nodes_2d[i];
            let suspended = self.is_suspended_by_sub_view(id);
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
                    SceneNodeData::CharacterBody2D(body) => (
                        BodyKind::Character,
                        body.enabled,
                        None,
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
            let enabled = enabled && !suspended;
            let rigid = rigid.map(|mut rigid| {
                rigid.enabled = enabled;
                rigid
            });
            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            // resolve tileset b4 node borrow; avoid full tilemap clone / step
            let tileset_source = self.nodes.get(id).and_then(|node| match &node.data {
                SceneNodeData::TileMap2D(tilemap) => Some(tilemap.tileset.clone()),
                _ => None,
            });
            let tileset = tileset_source
                .as_ref()
                .and_then(|source| crate::runtime::render_2d::resolve_tileset_2d(self, source));
            let is_tilemap = tileset_source.is_some();
            let mut shape_signature = body_signature_seed(kind);
            if let Some(node) = self.nodes.get(id) {
                if let SceneNodeData::TileMap2D(tilemap) = &node.data {
                    shape_signature = hash_tilemap_2d(shape_signature, tilemap);
                    if let Some(tileset) = tileset.as_deref() {
                        for tile in tileset.tiles.iter() {
                            if tile.collision {
                                shape_signature = hash_u64(shape_signature, tile.id as u64);
                                shape_signature = hash_tile_collision_shape_2d(
                                    shape_signature,
                                    &tile.collision_shape,
                                );
                            }
                        }
                    }
                } else {
                    if let SceneNodeData::WaterBody2D(water) = &node.data {
                        shape_signature = hash_water_shape(shape_signature, water.water.shape);
                    }
                    for &child_id in self.nodes.children(id).unwrap_or_default() {
                        let Some(child) = self.nodes.get(child_id) else {
                            continue;
                        };
                        if let SceneNodeData::CollisionShape2D(shape) = &child.data {
                            shape_signature = hash_collision_shape_2d(shape_signature, shape, kind);
                        }
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
                if is_tilemap {
                    if let Some(node) = self.nodes.get(id)
                        && let SceneNodeData::TileMap2D(tilemap) = &node.data
                    {
                        shapes.extend(tilemap_shape_descs_2d(
                            tilemap,
                            groups.0,
                            groups.1,
                            material.0,
                            material.1,
                            material.2,
                            tileset.as_deref(),
                        ));
                    }
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
                    let children = self.nodes.children(id).unwrap_or_default();
                    let child_count = children.len();
                    if shapes.capacity() < child_count {
                        shapes.reserve(child_count - shapes.capacity());
                    }
                    for &child_id in children {
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
                sync_signature: body_sync_signature_2d_if_useful(kind, enabled, global, rigid),
                shape_signature,
                shapes,
            });
        }
        out
    }

    pub(super) fn collect_body_descs_3d(&mut self) -> Vec<BodyDesc3D> {
        #[cfg(any(test, feature = "bench"))]
        self.physics_collect_calls_3d
            .set(self.physics_collect_calls_3d.get() + 1);
        let node_count = self.internal_updates.physics_body_nodes_3d.len();
        let mut out = std::mem::take(&mut self.physics_body_descs_3d);
        out.clear();
        if out.capacity() < node_count {
            out.reserve(node_count - out.capacity());
        }
        for i in 0..node_count {
            let id = self.internal_updates.physics_body_nodes_3d[i];
            let suspended = self.is_suspended_by_sub_view(id);
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
                    SceneNodeData::CharacterBody3D(body) => (
                        BodyKind::Character,
                        body.enabled,
                        None,
                        (body.friction, body.restitution, body.density),
                        (body.collision_layers, body.collision_mask),
                    ),
                    _ => continue,
                }
            };
            let enabled = enabled && !suspended;
            let rigid = rigid.map(|mut rigid| {
                rigid.enabled = enabled;
                rigid
            });

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
                for &child_id in self.nodes.children(id).unwrap_or_default() {
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
                let children = self.nodes.children(id).unwrap_or_default();
                let child_count = children.len();
                if shapes.capacity() < child_count {
                    shapes.reserve(child_count - shapes.capacity());
                }
                for &child_id in children {
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
                sync_signature: body_sync_signature_3d_if_useful(kind, enabled, global, rigid),
                shape_signature,
                shapes,
            });
        }
        out
    }

    pub(super) fn collect_joint_descs_2d(&mut self) -> Vec<JointDesc2D> {
        let mut out = std::mem::take(&mut self.physics_joint_descs_2d);
        out.clear();
        let node_count = self.internal_updates.physics_joint_nodes_2d.len();
        if out.capacity() < node_count {
            out.reserve(node_count - out.capacity());
        }
        for i in 0..self.internal_updates.physics_joint_nodes_2d.len() {
            let id = self.internal_updates.physics_joint_nodes_2d[i];
            let suspended = self.is_suspended_by_sub_view(id);
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
            let enabled = enabled && !suspended;
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

    pub(super) fn collect_joint_descs_3d(&mut self) -> Vec<JointDesc3D> {
        let mut out = std::mem::take(&mut self.physics_joint_descs_3d);
        out.clear();
        let node_count = self.internal_updates.physics_joint_nodes_3d.len();
        if out.capacity() < node_count {
            out.reserve(node_count - out.capacity());
        }
        for i in 0..self.internal_updates.physics_joint_nodes_3d.len() {
            let id = self.internal_updates.physics_joint_nodes_3d[i];
            let suspended = self.is_suspended_by_sub_view(id);
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
            let enabled = enabled && !suspended;
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

    pub(super) fn sync_world_2d(&mut self, bodies: &[BodyDesc2D]) {
        let mut handle_updates = std::mem::take(&mut self.physics_handle_updates_scratch_2d);
        handle_updates.clear();
        self.physics
            .sync_world_2d(bodies, |id, handle| handle_updates.push((id, handle)));
        for &(id, handle) in &handle_updates {
            self.set_body_handle_2d(id, handle);
        }
        handle_updates.clear();
        self.physics_handle_updates_scratch_2d = handle_updates;
    }

    pub(super) fn sync_world_3d(&mut self, bodies: &[BodyDesc3D]) {
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
        let mut handle_updates = std::mem::take(&mut self.physics_handle_updates_scratch_3d);
        handle_updates.clear();
        self.physics.sync_world_3d(bodies, assets, |id, handle| {
            handle_updates.push((id, handle));
        });
        for &(id, handle) in &handle_updates {
            self.set_body_handle_3d(id, handle);
        }
        handle_updates.clear();
        self.physics_handle_updates_scratch_3d = handle_updates;
    }

    pub(super) fn sync_joints_parallel(
        &mut self,
        joints_2d: &[JointDesc2D],
        joints_3d: &[JointDesc3D],
    ) {
        self.physics.sync_joints_parallel(joints_2d, joints_3d);
    }

    pub(super) fn step_worlds_parallel(&mut self) {
        self.physics
            .step_worlds_parallel(self.physics_gravity(), self.time.fixed_delta);
    }

    pub(super) fn apply_pending_forces_and_impulses_parallel(&mut self) {
        self.physics
            .apply_pending_forces_and_impulses_parallel(self.physics_coef(), self.time.fixed_delta);
    }

    pub(super) fn sync_world_to_nodes_2d(&mut self) -> bool {
        let Some(mut world) = self.physics.world_2d.take() else {
            return false;
        };

        // SoA writeback: stage awake rigid poses frm rapier, sort by slot,
        // then 1 fused fat-slot write / body (parent + b4 + pose + vel).
        // handle write drop: set @ create/rm via sync_world callback.
        let mut staged = std::mem::take(&mut self.physics_writeback_scratch_2d);
        staged.clear();
        for (&id, state) in &mut world.body_map {
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
            update_body_sync_state_2d(
                state,
                position,
                rotation,
                lin,
                ang,
                sleeping,
                same_as_last_sync,
            );
            staged.push(StagedBodyPose2D {
                id,
                position,
                rotation,
                lin,
                ang,
            });
        }
        self.physics.world_2d = Some(world);
        // slot order -> arena writes sequential-ish, not hashmap order
        staged.sort_unstable_by_key(|pose| pose.id.index());

        let mut changed = false;
        for pose in &staged {
            // 1 fat-slot touch: parent read + b4 capture + pose/vel write fused
            let Some((parent, before_local, moved)) = self
                .nodes
                .get_mut_untracked(pose.id)
                .and_then(|scene_node| {
                    let parent = scene_node.parent;
                    let SceneNodeData::RigidBody2D(node) = &mut scene_node.data else {
                        return None;
                    };
                    let before_local = node.transform;
                    node.linear_velocity = pose.lin;
                    node.angular_velocity = pose.ang;
                    if parent.is_nil() {
                        let moved = before_local.position != pose.position
                            || before_local.rotation != pose.rotation;
                        node.transform.position = pose.position;
                        node.transform.rotation = pose.rotation;
                        Some((parent, before_local, moved))
                    } else {
                        Some((parent, before_local, true))
                    }
                })
            else {
                continue;
            };
            changed = true;
            if parent.is_nil() {
                // root body: global = local; skip parent walk
                let curr = Transform2D {
                    position: pose.position,
                    rotation: pose.rotation,
                    scale: before_local.scale,
                };
                self.record_physics_pose_2d(pose.id, parent, before_local, curr);
                if moved {
                    self.mark_transform_dirty_recursive(pose.id);
                }
            } else {
                // nested body: kp global-space slow path
                let before = self
                    .get_global_transform_2d(pose.id)
                    .unwrap_or(Transform2D::IDENTITY);
                let mut curr = before;
                curr.position = pose.position;
                curr.rotation = pose.rotation;
                self.record_physics_pose_2d(pose.id, parent, before, curr);
                let _ = NodeAPI::set_global_transform_2d(self, pose.id, curr);
            }
        }

        staged.clear();
        self.physics_writeback_scratch_2d = staged;
        changed
    }

    pub(super) fn sync_world_to_nodes_3d(&mut self) -> bool {
        let Some(mut world) = self.physics.world_3d.take() else {
            return false;
        };

        // SoA writeback: stage awake rigid poses frm rapier, sort by slot,
        // then 1 fused fat-slot write / body (parent + b4 + pose + vel).
        // handle write drop: set @ create/rm via sync_world callback.
        let mut staged = std::mem::take(&mut self.physics_writeback_scratch_3d);
        staged.clear();
        for (&id, state) in &mut world.body_map {
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
            update_body_sync_state_3d(
                state,
                position,
                rotation,
                lin,
                ang,
                sleeping,
                same_as_last_sync,
            );
            staged.push(StagedBodyPose3D {
                id,
                position,
                rotation,
                lin,
                ang,
            });
        }
        self.physics.world_3d = Some(world);
        // slot order -> arena writes sequential-ish, not hashmap order
        staged.sort_unstable_by_key(|pose| pose.id.index());

        let mut changed = false;
        for pose in &staged {
            // 1 fat-slot touch: parent read + b4 capture + pose/vel write fused
            let Some((parent, before_local, moved)) = self
                .nodes
                .get_mut_untracked(pose.id)
                .and_then(|scene_node| {
                    let parent = scene_node.parent;
                    let SceneNodeData::RigidBody3D(node) = &mut scene_node.data else {
                        return None;
                    };
                    let before_local = node.transform;
                    node.linear_velocity = pose.lin;
                    node.angular_velocity = pose.ang;
                    if parent.is_nil() {
                        let moved = before_local.position != pose.position
                            || before_local.rotation != pose.rotation;
                        node.transform.position = pose.position;
                        node.transform.rotation = pose.rotation;
                        Some((parent, before_local, moved))
                    } else {
                        Some((parent, before_local, true))
                    }
                })
            else {
                continue;
            };
            changed = true;
            if parent.is_nil() {
                // root body: global = local; skip parent walk
                let curr = Transform3D {
                    position: pose.position,
                    rotation: pose.rotation,
                    scale: before_local.scale,
                };
                self.record_physics_pose_3d(pose.id, parent, before_local, curr);
                if moved {
                    self.mark_transform_dirty_recursive(pose.id);
                }
            } else {
                // nested body: kp global-space slow path
                let before = self
                    .get_global_transform_3d(pose.id)
                    .unwrap_or(Transform3D::IDENTITY);
                let mut curr = before;
                curr.position = pose.position;
                curr.rotation = pose.rotation;
                self.record_physics_pose_3d(pose.id, parent, before, curr);
                let _ = NodeAPI::set_global_transform_3d(self, pose.id, curr);
            }
        }

        staged.clear();
        self.physics_writeback_scratch_3d = staged;
        changed
    }

    // bench-only isolators 4 SoA phase-5 gate.
    // collect wrapper store Vec back -> mirror real scratch reuse, no fake realloc.
    #[cfg(feature = "bench")]
    pub fn bench_collect_body_descs_2d(&mut self) -> usize {
        let bodies = self.collect_body_descs_2d();
        let len = bodies.len();
        self.physics_body_descs_2d = bodies;
        len
    }

    #[cfg(feature = "bench")]
    pub fn bench_collect_body_descs_3d(&mut self) -> usize {
        let bodies = self.collect_body_descs_3d();
        let len = bodies.len();
        self.physics_body_descs_3d = bodies;
        len
    }

    #[cfg(feature = "bench")]
    pub fn bench_sync_world_to_nodes_2d(&mut self) -> bool {
        self.sync_world_to_nodes_2d()
    }

    #[cfg(feature = "bench")]
    pub fn bench_sync_world_to_nodes_3d(&mut self) -> bool {
        self.sync_world_to_nodes_3d()
    }

    pub(super) fn set_body_handle_2d(&mut self, id: NodeID, handle: Option<u64>) {
        if let Some(node) = self.nodes.get_mut_untracked(id) {
            match &mut node.data {
                SceneNodeData::StaticBody2D(body) => body.physics_handle = handle,
                SceneNodeData::Area2D(body) => body.physics_handle = handle,
                SceneNodeData::RigidBody2D(body) => body.physics_handle = handle,
                SceneNodeData::CharacterBody2D(body) => body.physics_handle = handle,
                _ => {}
            }
        }
    }

    pub(super) fn set_body_handle_3d(&mut self, id: NodeID, handle: Option<u64>) {
        if let Some(node) = self.nodes.get_mut_untracked(id) {
            match &mut node.data {
                SceneNodeData::StaticBody3D(body) => body.physics_handle = handle,
                SceneNodeData::Area3D(body) => body.physics_handle = handle,
                SceneNodeData::RigidBody3D(body) => body.physics_handle = handle,
                SceneNodeData::CharacterBody3D(body) => body.physics_handle = handle,
                _ => {}
            }
        }
    }

    pub(super) fn physics_gravity(&self) -> f32 {
        self.physics_gravity_raw() * self.physics_coef()
    }

    pub(super) fn physics_gravity_raw(&self) -> f32 {
        self.physics_gravity_override
            .or_else(|| self.project().map(|p| p.config.physics_gravity))
            .filter(|v| v.is_finite())
            .unwrap_or(-9.81)
    }

    pub(super) fn physics_coef(&self) -> f32 {
        self.physics_coef_override
            .or_else(|| self.project().map(|p| p.config.physics_coef))
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0)
    }
}
