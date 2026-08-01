use super::*;

impl Runtime {
    pub(super) fn queue_physics_force_emitters_2d(&mut self) {
        let active_world = self.active_physics_world();
        self.force_water_impacts_2d
            .retain(|impact| impact.world != active_world);
        let mut ids = std::mem::take(&mut self.physics_force_emitter_ids_scratch_2d);
        ids.clear();
        super::super::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::PhysicsForceEmitter2D,
            |node| matches!(node.data, SceneNodeData::PhysicsForceEmitter2D(_)),
            &mut ids,
        );
        let mut emitters = std::mem::take(&mut self.physics_force_emitters_scratch_2d);
        emitters.clear();
        for id in ids.drain(..) {
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(global) = self.physics_transform_2d(id) else {
                continue;
            };
            let Some(node) = self.nodes.get_mut_untracked(id) else {
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
                // stage (pos, id) only; emitter data re-read @ apply -> no clone
                emitters.push((global.position, id));
            }
            emitter.age += self.time.fixed_delta.max(0.0);
        }
        self.physics_force_emitter_ids_scratch_2d = ids;
        for (position, id) in emitters.drain(..) {
            // borrow-safe zero-alloc apply: swap emitter out of node slot
            // (Default vectors = no alloc), apply, swap back untouched.
            let Some(staged) = self.nodes.get_mut_untracked(id).and_then(|node| {
                let SceneNodeData::PhysicsForceEmitter2D(emitter) = &mut node.data else {
                    return None;
                };
                Some(std::mem::take(emitter))
            }) else {
                continue;
            };
            self.apply_force_emitter_2d(position, &staged);
            if let Some(node) = self.nodes.get_mut_untracked(id)
                && let SceneNodeData::PhysicsForceEmitter2D(emitter) = &mut node.data
            {
                *emitter = staged;
            }
        }
        self.physics_force_emitters_scratch_2d = emitters;
        // one-shot script emitters: consume active-world entries in place
        // (order kp, no keep-buf alloc).
        let mut pending = std::mem::take(&mut self.pending_force_emitters_2d);
        for (_, emitter) in pending.extract_if(.., |(world, _)| *world == active_world) {
            self.apply_force_emitter_2d(emitter.transform.position, &emitter);
        }
        self.pending_force_emitters_2d = pending;
    }

    pub(super) fn queue_physics_force_emitters_3d(&mut self) {
        let active_world = self.active_physics_world();
        self.force_water_impacts_3d
            .retain(|impact| impact.world != active_world);
        let mut ids = std::mem::take(&mut self.physics_force_emitter_ids_scratch_3d);
        ids.clear();
        super::super::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::PhysicsForceEmitter3D,
            |node| matches!(node.data, SceneNodeData::PhysicsForceEmitter3D(_)),
            &mut ids,
        );
        let mut emitters = std::mem::take(&mut self.physics_force_emitters_scratch_3d);
        emitters.clear();
        for id in ids.drain(..) {
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(global) = self.physics_transform_3d(id) else {
                continue;
            };
            let Some(node) = self.nodes.get_mut_untracked(id) else {
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
                // stage (pos, id) only; emitter data re-read @ apply -> no clone
                emitters.push((global.position, id));
            }
            emitter.age += self.time.fixed_delta.max(0.0);
        }
        self.physics_force_emitter_ids_scratch_3d = ids;
        for (position, id) in emitters.drain(..) {
            // borrow-safe zero-alloc apply: swap emitter out of node slot
            // (Default vectors = no alloc), apply, swap back untouched.
            let Some(staged) = self.nodes.get_mut_untracked(id).and_then(|node| {
                let SceneNodeData::PhysicsForceEmitter3D(emitter) = &mut node.data else {
                    return None;
                };
                Some(std::mem::take(emitter))
            }) else {
                continue;
            };
            self.apply_force_emitter_3d(position, &staged);
            if let Some(node) = self.nodes.get_mut_untracked(id)
                && let SceneNodeData::PhysicsForceEmitter3D(emitter) = &mut node.data
            {
                *emitter = staged;
            }
        }
        self.physics_force_emitters_scratch_3d = emitters;
        // one-shot script emitters: consume active-world entries in place
        // (order kp, no keep-buf alloc).
        let mut pending = std::mem::take(&mut self.pending_force_emitters_3d);
        for (_, emitter) in pending.extract_if(.., |(world, _)| *world == active_world) {
            self.apply_force_emitter_3d(emitter.transform.position, &emitter);
        }
        self.pending_force_emitters_3d = pending;
    }

    pub(super) fn apply_force_emitter_2d(
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
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(global) = self.physics_transform_2d(id) else {
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

    pub(super) fn apply_force_emitter_3d(
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
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(global) = self.physics_transform_3d(id) else {
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

    pub(super) fn queue_force_water_impacts_2d(
        &mut self,
        emitter_pos: Vector2,
        emitter: &perro_nodes::PhysicsForceEmitter2D,
    ) {
        self.cached_water_ids_2d();
        let ids = std::mem::take(&mut self.water_ids_2d_cache);
        for &id in ids.iter() {
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(global) = self.physics_transform_2d(id) else {
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
                    world: self.active_physics_world(),
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
        self.water_ids_2d_cache = ids;
    }

    pub(super) fn queue_force_water_impacts_3d(
        &mut self,
        emitter_pos: Vector3,
        emitter: &perro_nodes::PhysicsForceEmitter3D,
    ) {
        self.cached_water_ids_3d();
        let ids = std::mem::take(&mut self.water_ids_3d_cache);
        for &id in ids.iter() {
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(global) = self.physics_transform_3d(id) else {
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
                    world: self.active_physics_world(),
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
        self.water_ids_3d_cache = ids;
    }

    /// Drop readback-sample cache entries whose water (or sampled body) node
    /// is gone. Nothing else removes from these maps -- they are fed straight
    /// from `RenderEvent::WaterSamples` / `WaterBodySamples` -- so freeing a
    /// water node leaks its entries and, worse, keeps
    /// `can_skip_physics_fixed_step_pre_sync` permanently false. Same
    /// clear-in-place / drop-dead split as the pending-query prune below.
    pub(super) fn prune_dead_water_samples(&mut self) {
        if !self.water_samples.is_empty() {
            let mut samples = std::mem::take(&mut self.water_samples);
            samples.retain(|id, _| self.nodes.get(*id).is_some());
            self.water_samples = samples;
        }
        if !self.water_sample_times.is_empty() {
            let mut times = std::mem::take(&mut self.water_sample_times);
            times.retain(|id, _| self.nodes.get(*id).is_some());
            self.water_sample_times = times;
        }
        if !self.water_body_samples.is_empty() {
            let mut body_samples = std::mem::take(&mut self.water_body_samples);
            body_samples.retain(|key, _| {
                self.nodes.get(key.water).is_some() && self.nodes.get(key.body).is_some()
            });
            self.water_body_samples = body_samples;
        }
    }

    pub(super) fn queue_water_forces_2d(&mut self) {
        let active_world = self.active_physics_world();
        // active-world entries: drop (refilled this tick); other worlds: kp
        // data; dead nodes: drop entry. dropping instead of clear-in-place kp
        // both maps free of drained-empty entries, so the fixed-step skip gate
        // reduces 2 a map is_empty() instead of a per-entry emptiness scan.
        let mut pending_queries = std::mem::take(&mut self.pending_water_queries_2d);
        pending_queries.retain(|id, _| match self.node_world(*id) {
            Some(world) => world != active_world,
            None => false,
        });
        self.pending_water_queries_2d = pending_queries;
        let mut contacts = std::mem::take(&mut self.water_contacts_2d);
        contacts.retain(|id, _| match self.node_world(*id) {
            Some(world) => world != active_world,
            None => false,
        });
        self.water_contacts_2d = contacts;
        self.cached_water_ids_2d();
        let water_ids = std::mem::take(&mut self.water_ids_2d_cache);
        let mut waters = std::mem::take(&mut self.physics_waters_scratch_2d);
        waters.clear();
        for &id in water_ids.iter() {
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(transform) = self.physics_transform_2d(id) else {
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
            self.physics_waters_scratch_2d = waters;
            self.water_ids_2d_cache = water_ids;
            return;
        }
        let bins = std::mem::take(&mut self.physics_water_bins_scratch_2d);
        let water_index = RuntimeWaterIndex2D::new_with_bins(waters, bins);
        let camera_pos = self
            .render_2d
            .last_camera
            .as_ref()
            .map(|camera| Vector2::new(camera.position[0], camera.position[1]))
            .unwrap_or(Vector2::ZERO);

        self.cached_rigid_body_ids_2d();
        let body_ids = std::mem::take(&mut self.water_rigid_body_ids_2d_cache);
        let mut bodies = std::mem::take(&mut self.physics_water_bodies_scratch_2d);
        bodies.clear();
        if bodies.capacity() < body_ids.len() {
            bodies.reserve(body_ids.len() - bodies.capacity());
        }
        for &body_id in body_ids.iter() {
            if self.node_world(body_id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(body_transform) = self.physics_transform_2d(body_id) else {
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
        let mut splash_impacts =
            water_body_splashes_2d(&bodies, &water_index, &self.water_body_samples, elapsed);
        for impact in splash_impacts.iter_mut() {
            impact.world = active_world;
        }
        self.register_water_queries_2d(&bodies, &water_index);
        self.record_water_contacts_2d(&bodies, &water_index, elapsed);
        let water_samples = &self.water_samples;
        let mut forces = std::mem::take(&mut self.physics_water_forces_scratch_2d);
        forces.clear();
        if bodies.len() >= WATER_FORCE_PAR_BODY_THRESHOLD {
            forces.par_extend(bodies.par_iter().flat_map_iter(|body| {
                water_forces_for_body_2d(
                    *body,
                    &water_index,
                    water_samples,
                    &self.water_body_samples,
                    elapsed,
                    camera_pos,
                )
            }));
        } else {
            forces.extend(bodies.iter().flat_map(|body| {
                water_forces_for_body_2d(
                    *body,
                    &water_index,
                    water_samples,
                    &self.water_body_samples,
                    elapsed,
                    camera_pos,
                )
            }));
        }
        bodies.clear();
        self.physics_water_bodies_scratch_2d = bodies;
        let mut waters = water_index.waters;
        waters.clear();
        self.physics_waters_scratch_2d = waters;
        self.physics_water_bins_scratch_2d = water_index.bins;
        self.water_rigid_body_ids_2d_cache = body_ids;
        for effect in forces.drain(..) {
            self.physics.queue_force_2d(effect.id, effect.force);
            if effect.impulse.length_squared() > 0.000_001 {
                self.physics.queue_impulse_2d(effect.id, effect.impulse);
            }
            self.apply_water_angular_nudge_2d(effect.id, effect.force.x * 0.04);
        }
        self.physics_water_forces_scratch_2d = forces;
        if !splash_impacts.is_empty() {
            self.force_water_impacts_2d.extend(splash_impacts);
        }
        // waves animate on the water's sim clock carried in render state, so
        // re-extract every tick or the surface freezes while the camera rests
        for &id in water_ids.iter() {
            self.mark_needs_rerender(id);
        }
        self.water_ids_2d_cache = water_ids;
    }

    pub(super) fn queue_water_forces_3d(&mut self) {
        let active_world = self.active_physics_world();
        // see queue_water_forces_2d: drop active-world / kp other / drop dead.
        let mut pending_queries = std::mem::take(&mut self.pending_water_queries_3d);
        pending_queries.retain(|id, _| match self.node_world(*id) {
            Some(world) => world != active_world,
            None => false,
        });
        self.pending_water_queries_3d = pending_queries;
        let mut contacts = std::mem::take(&mut self.water_contacts_3d);
        contacts.retain(|id, _| match self.node_world(*id) {
            Some(world) => world != active_world,
            None => false,
        });
        self.water_contacts_3d = contacts;
        self.cached_water_ids_3d();
        let water_ids = std::mem::take(&mut self.water_ids_3d_cache);
        let mut waters = std::mem::take(&mut self.physics_waters_scratch_3d);
        waters.clear();
        for &id in water_ids.iter() {
            if self.node_world(id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(transform) = self.physics_transform_3d(id) else {
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
            self.physics_waters_scratch_3d = waters;
            self.water_ids_3d_cache = water_ids;
            return;
        }
        let bins = std::mem::take(&mut self.physics_water_bins_scratch_3d);
        let water_index = RuntimeWaterIndex3D::new_with_bins(waters, bins);
        let camera_pos = self
            .render_3d
            .last_camera
            .as_ref()
            .map(|camera| Vector2::new(camera.position[0], camera.position[2]))
            .unwrap_or(Vector2::ZERO);

        self.cached_rigid_body_ids_3d();
        let body_ids = std::mem::take(&mut self.water_rigid_body_ids_3d_cache);
        let mut bodies = std::mem::take(&mut self.physics_water_bodies_scratch_3d);
        bodies.clear();
        if bodies.capacity() < body_ids.len() {
            bodies.reserve(body_ids.len() - bodies.capacity());
        }
        for &body_id in body_ids.iter() {
            if self.node_world(body_id) != Some(self.active_physics_world()) {
                continue;
            }
            let Some(body_transform) = self.physics_transform_3d(body_id) else {
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
        let mut splash_impacts = water_body_splashes_3d(
            &bodies,
            &water_index,
            &self.water_body_samples,
            elapsed,
            &mut self.water_entry_states_3d,
        );
        for impact in splash_impacts.iter_mut() {
            impact.world = active_world;
        }
        let mut entry_states = std::mem::take(&mut self.water_entry_states_3d);
        entry_states.retain(|body, _| {
            self.node_world(*body) != Some(active_world)
                || bodies.iter().any(|candidate| candidate.id == *body)
        });
        self.water_entry_states_3d = entry_states;
        self.register_water_queries_3d(&bodies, &water_index);
        self.record_water_contacts_3d(&bodies, &water_index, elapsed);
        let water_samples = &self.water_samples;
        let mut forces = std::mem::take(&mut self.physics_water_forces_scratch_3d);
        forces.clear();
        if bodies.len() >= WATER_FORCE_PAR_BODY_THRESHOLD {
            forces.par_extend(bodies.par_iter().flat_map_iter(|body| {
                water_forces_for_body_3d(
                    *body,
                    &water_index,
                    water_samples,
                    &self.water_body_samples,
                    elapsed,
                    camera_pos,
                )
            }));
        } else {
            forces.extend(bodies.iter().flat_map(|body| {
                water_forces_for_body_3d(
                    *body,
                    &water_index,
                    water_samples,
                    &self.water_body_samples,
                    elapsed,
                    camera_pos,
                )
            }));
        }
        bodies.clear();
        self.physics_water_bodies_scratch_3d = bodies;
        let mut waters = water_index.waters;
        waters.clear();
        self.physics_waters_scratch_3d = waters;
        self.physics_water_bins_scratch_3d = water_index.bins;
        self.water_rigid_body_ids_3d_cache = body_ids;
        for effect in forces.drain(..) {
            self.physics.queue_force_3d(effect.id, effect.force);
            if effect.impulse.length_squared() > 0.000_001 {
                self.physics.queue_impulse_3d(effect.id, effect.impulse);
            }
            self.apply_water_angular_nudge_3d(
                effect.id,
                Vector3::new(effect.force.z * 0.025, 0.0, -effect.force.x * 0.025),
            );
        }
        self.physics_water_forces_scratch_3d = forces;
        if !splash_impacts.is_empty() {
            self.force_water_impacts_3d.extend(splash_impacts);
        }
        // waves animate on the water's sim clock carried in render state, so
        // re-extract every tick or the surface freezes while the camera rests
        for &id in water_ids.iter() {
            self.mark_needs_rerender(id);
        }
        self.water_ids_3d_cache = water_ids;
    }

    pub(super) fn apply_water_angular_nudge_2d(&mut self, id: NodeID, delta: f32) {
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

    pub(super) fn body_float_radius_2d(&mut self, body: NodeID, body_pos: Vector2) -> f32 {
        let child_count = self.nodes.children(body).map(<[NodeID]>::len).unwrap_or(0);
        let mut radius = 0.0f32;
        for i in 0..child_count {
            let Some(child_id) = self
                .nodes
                .children(body)
                .and_then(|children| children.get(i).copied())
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

    pub(super) fn body_float_radius_3d(&mut self, body: NodeID, body_pos: Vector3) -> f32 {
        let child_count = self.nodes.children(body).map(<[NodeID]>::len).unwrap_or(0);
        let mut radius = 0.0f32;
        for i in 0..child_count {
            let Some(child_id) = self
                .nodes
                .children(body)
                .and_then(|children| children.get(i).copied())
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

    pub(super) fn apply_water_angular_nudge_3d(&mut self, id: NodeID, delta: Vector3) {
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
        let target = r3::Vector::new(
            (current.x + delta.x).clamp(-1.4, 1.4),
            (current.y + delta.y).clamp(-1.4, 1.4),
            (current.z + delta.z).clamp(-1.4, 1.4),
        );
        rb.set_angvel(target, true);
    }

    pub(super) fn register_water_queries_2d(
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

    pub(super) fn register_water_queries_3d(
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

    pub(super) fn record_water_contacts_2d(
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

    pub(super) fn record_water_contacts_3d(
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
                            // keep rings >= ~2 sim cells wide or they alias on the grid
                            radius: (body.float_radius * 0.9).max(1.3),
                            foam_amount: (sample.sample.foam
                                + Vector2::new(body.velocity.x, body.velocity.z).length() * 0.05
                                + body.velocity.y.abs() * 0.08)
                                .clamp(0.16, 1.0),
                        },
                    );
                }
            }
        }
    }
}
