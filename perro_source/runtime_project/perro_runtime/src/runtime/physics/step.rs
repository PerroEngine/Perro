use super::*;

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
        // reuse cached water id lists; skip full node scan + per-call Vec alloc.
        // mark_needs_rerender only touch dirty state, so taken caches stay valid.
        self.cached_water_ids_2d();
        let water_ids_2d = std::mem::take(&mut self.water_ids_2d_cache);
        for &id in water_ids_2d.iter() {
            self.mark_needs_rerender(id);
        }
        self.water_ids_2d_cache = water_ids_2d;
        self.cached_water_ids_3d();
        let water_ids_3d = std::mem::take(&mut self.water_ids_3d_cache);
        for &id in water_ids_3d.iter() {
            self.mark_needs_rerender(id);
        }
        self.water_ids_3d_cache = water_ids_3d;
    }

    pub fn physics_paused(&self) -> bool {
        self.physics.paused()
    }

    pub(crate) fn physics_fixed_step_timed(&mut self) -> RuntimePhysicsStepTiming {
        let total_start = Instant::now();

        let pre_transforms_start = Instant::now();
        // capture external chg b4 propagate clear dirty flags. physics-scoped:
        // non-physics node move (spin coin / ui tween) not force world re-sync.
        // pending roots -> conservative dirty (type unknown til walk).
        let had_physics_dirty_2d = self.dirty.has_physics_transform_dirty_2d();
        let had_physics_dirty_3d = self.dirty.has_physics_transform_dirty_3d();
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let pre_transforms = pre_transforms_start.elapsed();

        if self.can_skip_physics_fixed_step_pre_sync() {
            return RuntimePhysicsStepTiming {
                pre_transforms,
                collect: std::time::Duration::ZERO,
                sync_world: std::time::Duration::ZERO,
                apply_forces_impulses: std::time::Duration::ZERO,
                step: std::time::Duration::ZERO,
                sync_nodes: std::time::Duration::ZERO,
                post_transforms: std::time::Duration::ZERO,
                signals: std::time::Duration::ZERO,
                total: total_start.elapsed(),
            };
        }

        // skip collect+sync when world already mirror nodes:
        // arena revision unchanged since last sync + no transform dirty.
        // physics-driven moves land in nodes via sync_world_to_nodes;
        // revision re-record aft post_transforms so internal write-back not invalidate.
        let node_revision = self.nodes.physics_revision();
        let sync_2d_needed =
            self.physics_synced_node_revision_2d != Some(node_revision) || had_physics_dirty_2d;
        let sync_3d_needed =
            self.physics_synced_node_revision_3d != Some(node_revision) || had_physics_dirty_3d;

        let collect_start = Instant::now();
        let bodies_2d = sync_2d_needed.then(|| self.collect_body_descs_2d());
        let bodies_3d = sync_3d_needed.then(|| self.collect_body_descs_3d());
        let joints_2d = sync_2d_needed.then(|| self.collect_joint_descs_2d());
        let joints_3d = sync_3d_needed.then(|| self.collect_joint_descs_3d());
        let collect = collect_start.elapsed();

        let sync_world_start = Instant::now();
        if let Some(bodies) = bodies_2d {
            self.sync_world_2d(&bodies);
            self.physics_body_descs_2d = bodies;
        }
        if let Some(bodies) = bodies_3d {
            self.sync_world_3d(&bodies);
            self.physics_body_descs_3d = bodies;
        }
        match (joints_2d, joints_3d) {
            (Some(joints_2d), Some(joints_3d)) => {
                self.sync_joints_parallel(&joints_2d, &joints_3d);
                self.physics_joint_descs_2d = joints_2d;
                self.physics_joint_descs_3d = joints_3d;
            }
            (Some(joints_2d), None) => {
                self.physics.sync_joints_2d(&joints_2d);
                self.physics_joint_descs_2d = joints_2d;
            }
            (None, Some(joints_3d)) => {
                self.physics.sync_joints_3d(&joints_3d);
                self.physics_joint_descs_3d = joints_3d;
            }
            (None, None) => {}
        }
        // world fresh vs nodes til next node chg; query path skip re-sync
        let synced_revision = Some(self.nodes.physics_revision());
        self.physics_synced_node_revision_2d = synced_revision;
        self.physics_synced_node_revision_3d = synced_revision;
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
        // Water/rigid-body id caches self-invalidate on nodes.physics_revision()
        // chg (cached_water_ids_2d/3d, cached_rigid_body_ids_2d/3d in runtime.rs)
        // -- no unconditional reset needed; empty-arena scenes now cache too.
        self.queue_physics_force_emitters_2d();
        self.queue_physics_force_emitters_3d();
        self.queue_water_forces_2d();
        self.queue_water_forces_3d();
        self.apply_pending_forces_and_impulses_parallel();
        let apply_forces_impulses = apply_forces_impulses_start.elapsed();

        let (step, sync_nodes, post_transforms) = if self.physics.can_skip_step() {
            (
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
            )
        } else {
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
            (step, sync_nodes, post_transforms)
        };

        // internal write-back (world -> nodes, emitter age) bump arena revision;
        // nodes still mirror world -> re-record so next step / query skip
        let synced_revision = Some(self.nodes.physics_revision());
        self.physics_synced_node_revision_2d = synced_revision;
        self.physics_synced_node_revision_3d = synced_revision;

        self.prune_character_sweep_hits();

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

    pub(super) fn can_skip_physics_fixed_step_pre_sync(&self) -> bool {
        self.schedules.fixed_slots_empty()
            && !self.has_physics_joint_nodes()
            && self.physics_synced_node_revision_2d == Some(self.nodes.physics_revision())
            && self.physics_synced_node_revision_3d == Some(self.nodes.physics_revision())
            && (self.internal_updates.physics_body_nodes_2d.is_empty()
                || self.physics.world_2d.is_some())
            && (self.internal_updates.physics_body_nodes_3d.is_empty()
                || self.physics.world_3d.is_some())
            && !self.dirty.has_transform_dirty_any()
            && self.pending_force_emitters_2d.is_empty()
            && self.pending_force_emitters_3d.is_empty()
            && self.force_water_impacts_2d.is_empty()
            && self.force_water_impacts_3d.is_empty()
            && self.water_samples.is_empty()
            && self.water_sample_times.is_empty()
            && self.water_body_samples.is_empty()
            && self.pending_water_queries_2d.is_empty()
            && self.pending_water_queries_3d.is_empty()
            && self.water_contacts_2d.is_empty()
            && self.water_contacts_3d.is_empty()
            && self.physics.active_area_overlaps_2d.is_empty()
            && self.physics.active_area_overlaps_3d.is_empty()
            && self.physics.can_skip_step()
    }

    pub(super) fn has_physics_joint_nodes(&self) -> bool {
        !self.internal_updates.physics_joint_nodes_2d.is_empty()
            || !self.internal_updates.physics_joint_nodes_3d.is_empty()
    }

    /// world may lag nodes: node data / structure chg outside fixed step.
    /// query path cal this b4 query; skip full collect+sync when world fresh.
    pub(crate) fn ensure_physics_world_synced_2d(&mut self) {
        // physics-scoped gate: only 2d physics node moves (or unpropagated
        // roots) invalidate; non-physics tweens skip the full collect+sync.
        if self.physics_synced_node_revision_2d == Some(self.nodes.physics_revision())
            && !self.dirty.has_physics_transform_dirty_2d()
        {
            return;
        }
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);
        self.physics_body_descs_2d = bodies_2d;
        self.physics_synced_node_revision_2d = Some(self.nodes.physics_revision());
    }

    pub(crate) fn ensure_physics_world_synced_3d(&mut self) {
        // physics-scoped gate: see ensure_physics_world_synced_2d.
        if self.physics_synced_node_revision_3d == Some(self.nodes.physics_revision())
            && !self.dirty.has_physics_transform_dirty_3d()
        {
            return;
        }
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);
        self.physics_body_descs_3d = bodies_3d;
        self.physics_synced_node_revision_3d = Some(self.nodes.physics_revision());
    }

    pub(crate) fn invalidate_physics_query_sync(&mut self) {
        self.physics_synced_node_revision_2d = None;
        self.physics_synced_node_revision_3d = None;
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
        self.character_fall_speed_2d.clear();
        self.character_fall_speed_3d.clear();
        self.character_sweep_hit_2d.clear();
        self.character_sweep_hit_3d.clear();
        self.invalidate_physics_query_sync();
    }
}
