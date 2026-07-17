use super::*;

impl Runtime {
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
        self.ensure_physics_world_synced_3d();
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
        self.ensure_physics_world_synced_2d();
        self.physics
            .raycast_2d(origin, direction, max_distance, filter)
    }

    pub(crate) fn prepare_audio_raycast_2d(&mut self) {
        self.ensure_physics_world_synced_2d();
        self.physics.update_query_pipeline_2d();
    }

    pub(crate) fn prepare_audio_raycast_3d(&mut self) {
        self.ensure_physics_world_synced_3d();
        self.physics.update_query_pipeline_3d();
    }

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
        self.ensure_physics_world_synced_2d();
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
        self.ensure_physics_world_synced_3d();
        self.physics
            .shape_cast_3d(shape, origin, direction, max_distance, filter)
    }

    pub fn physics_move_body_2d(
        &mut self,
        body_id: NodeID,
        target: Vector2,
        margin: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        self.ensure_physics_world_synced_2d();
        // world in-sync now (ensure just ran); safe 2 re-record if fast path
        // reproduce next full sync 4 this one body.
        let was_synced =
            self.physics_synced_node_revision_2d == Some(self.nodes.physics_revision());
        let result = self.physics.move_body_2d(body_id, target, margin, filter)?;
        let mut transform = self.get_global_transform_2d(body_id)?;
        transform.position = result.position;
        if !<Runtime as NodeAPI>::set_global_transform_2d(self, body_id, transform) {
            return None;
        }
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        // fast path: write resolved pose straight -> rapier + re-record sync ver
        // instead of full O(bodies) collect+sync next op. only when world was
        // in-sync b4 move (else other stale chg must still trigger full sync).
        if !was_synced || !self.commit_moved_body_2d_fast(body_id) {
            // node mv aft sync -> world stale 4 next query
            self.physics_synced_node_revision_2d = None;
        }
        self.record_character_sweep_hit_2d(body_id, &result);
        Some(result)
    }

    /// reproduce next full sync_world_2d 4 the just-moved `body_id`: push
    /// re-read global + fresh sig into rapier, then re-record synced ver so the
    /// next op skip the O(bodies) walk. ret false -> caller full-invalidate.
    /// re-read global == collect input; sig computed same as collect => next
    /// legit full sync see no mismatch 4 this body.
    pub(super) fn commit_moved_body_2d_fast(&mut self, body_id: NodeID) -> bool {
        // other node chg since ensure (unlikely w/in move, but guard) => bail.
        let node_revision = self.nodes.physics_revision();
        let Some((kind, enabled, rigid)) = self.physics_body_sync_props_2d(body_id) else {
            return false;
        };
        let Some(global) = self.get_global_transform_2d(body_id) else {
            return false;
        };
        let signature = body_sync_signature_2d_if_useful(kind, enabled, global, rigid);
        if !self
            .physics
            .commit_moved_body_2d(body_id, global, signature)
        {
            return false;
        }
        self.physics_synced_node_revision_2d = Some(node_revision);
        true
    }

    /// body kind/enabled/rigid props as collect_body_descs_2d see them.
    /// only bodies move_body target (char, rigid, static, area); ret None else.
    pub(super) fn physics_body_sync_props_2d(
        &self,
        id: NodeID,
    ) -> Option<(BodyKind, bool, Option<RigidProps2D>)> {
        let node = self.nodes.get(id)?;
        match &node.data {
            SceneNodeData::StaticBody2D(body) => Some((BodyKind::Static, body.enabled, None)),
            SceneNodeData::Area2D(body) => Some((BodyKind::Area, body.enabled, None)),
            SceneNodeData::CharacterBody2D(body) => Some((BodyKind::Character, body.enabled, None)),
            SceneNodeData::RigidBody2D(body) => Some((
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
            )),
            _ => None,
        }
    }

    pub fn physics_move_body_3d(
        &mut self,
        body_id: NodeID,
        target: Vector3,
        margin: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        self.ensure_physics_world_synced_3d();
        let was_synced =
            self.physics_synced_node_revision_3d == Some(self.nodes.physics_revision());
        let result = self.physics.move_body_3d(body_id, target, margin, filter)?;
        let mut transform = self.get_global_transform_3d(body_id)?;
        transform.position = result.position;
        if !<Runtime as NodeAPI>::set_global_transform_3d(self, body_id, transform) {
            return None;
        }
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        // fast path: see physics_move_body_2d.
        if !was_synced || !self.commit_moved_body_3d_fast(body_id) {
            // node mv aft sync -> world stale 4 next query
            self.physics_synced_node_revision_3d = None;
        }
        self.record_character_sweep_hit_3d(body_id, &result);
        Some(result)
    }

    /// 3d twin of [`Self::commit_moved_body_2d_fast`].
    pub(super) fn commit_moved_body_3d_fast(&mut self, body_id: NodeID) -> bool {
        let node_revision = self.nodes.physics_revision();
        let Some((kind, enabled, rigid)) = self.physics_body_sync_props_3d(body_id) else {
            return false;
        };
        let Some(global) = self.get_global_transform_3d(body_id) else {
            return false;
        };
        let signature = body_sync_signature_3d_if_useful(kind, enabled, global, rigid);
        if !self
            .physics
            .commit_moved_body_3d(body_id, global, signature)
        {
            return false;
        }
        self.physics_synced_node_revision_3d = Some(node_revision);
        true
    }

    /// body kind/enabled/rigid props as collect_body_descs_3d see them.
    pub(super) fn physics_body_sync_props_3d(
        &self,
        id: NodeID,
    ) -> Option<(BodyKind, bool, Option<RigidProps3D>)> {
        let node = self.nodes.get(id)?;
        match &node.data {
            SceneNodeData::StaticBody3D(body) => Some((BodyKind::Static, body.enabled, None)),
            SceneNodeData::Area3D(body) => Some((BodyKind::Area, body.enabled, None)),
            SceneNodeData::CharacterBody3D(body) => Some((BodyKind::Character, body.enabled, None)),
            SceneNodeData::RigidBody3D(body) => Some((
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
            )),
            _ => None,
        }
    }

    /// sweep along `motion`; on hit, project remainder onto hit plane +
    /// re-sweep, up to MAX_SLIDE_ITERATIONS. body only mv here, never by solver.
    pub fn physics_move_and_slide_2d(
        &mut self,
        body_id: NodeID,
        motion: Vector2,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsSlideResult2D> {
        let mut position = self.get_global_transform_2d(body_id)?.position;
        let mut remaining = motion;
        // hits move into the returned (API-owned) result -> no scratch reuse;
        // Vec::new stay alloc-free til a real hit, bounded by MAX_SLIDE_ITERATIONS.
        let mut hits = Vec::new();
        for _ in 0..MAX_SLIDE_ITERATIONS {
            if remaining.length_squared() <= 1.0e-12 {
                remaining = Vector2::ZERO;
                break;
            }
            let target = position + remaining;
            let result =
                self.physics_move_body_2d(body_id, target, CHARACTER_MOVE_MARGIN, filter)?;
            position = result.position;
            let Some(hit) = result.hit else {
                remaining = Vector2::ZERO;
                break;
            };
            hits.push(hit);
            let unconsumed = target - position;
            remaining = unconsumed - hit.normal * unconsumed.dot(hit.normal);
        }
        Some(PhysicsSlideResult2D {
            position,
            remainder: remaining,
            hits,
        })
    }

    /// sweep along `motion`; on hit, project remainder onto hit plane +
    /// re-sweep, up to MAX_SLIDE_ITERATIONS. body only mv here, never by solver.
    pub fn physics_move_and_slide_3d(
        &mut self,
        body_id: NodeID,
        motion: Vector3,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsSlideResult3D> {
        let mut position = self.get_global_transform_3d(body_id)?.position;
        let mut remaining = motion;
        // hits move into the returned (API-owned) result -> no scratch reuse;
        // Vec::new stay alloc-free til a real hit, bounded by MAX_SLIDE_ITERATIONS.
        let mut hits = Vec::new();
        for _ in 0..MAX_SLIDE_ITERATIONS {
            if remaining.length_squared() <= 1.0e-12 {
                remaining = Vector3::ZERO;
                break;
            }
            let target = position + remaining;
            let result =
                self.physics_move_body_3d(body_id, target, CHARACTER_MOVE_MARGIN, filter)?;
            position = result.position;
            let Some(hit) = result.hit else {
                remaining = Vector3::ZERO;
                break;
            };
            hits.push(hit);
            let unconsumed = target - position;
            remaining = unconsumed - hit.normal * unconsumed.dot(hit.normal);
        }
        Some(PhysicsSlideResult3D {
            position,
            remainder: remaining,
            hits,
        })
    }

    /// script-invoked engine gravity 4 char bodies. integrate internal fall
    /// speed frm world gravity, sweep down, reset on ground hit. separate frm
    /// move_and_slide: cal each step when engine gravity wanted, skip 4 custom.
    pub fn physics_apply_gravity_2d(
        &mut self,
        body_id: NodeID,
        dt: f32,
        max_fall_speed: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        let is_char = matches!(
            self.nodes.get(body_id).map(|node| &node.data),
            Some(SceneNodeData::CharacterBody2D(_))
        );
        if !is_char || !dt.is_finite() || dt <= 0.0 {
            return None;
        }
        let gravity = self.physics_gravity();
        let fall = self.character_fall_speed_2d.entry(body_id).or_insert(0.0);
        let limit = max_fall_speed.abs().max(0.001);
        *fall = (*fall + gravity * dt).clamp(-limit, limit);
        let drop = *fall * dt;
        let global = self.get_global_transform_2d(body_id)?;
        let target = Vector2::new(global.position.x, global.position.y + drop);
        let result = self.physics_move_body_2d(body_id, target, CHARACTER_MOVE_MARGIN, filter);
        if result.is_none_or(|result| result.clipped) {
            self.character_fall_speed_2d.insert(body_id, 0.0);
        }
        result
    }

    /// script-invoked engine gravity 4 char bodies. integrate internal fall
    /// speed frm world gravity, sweep down, reset on ground hit. separate frm
    /// move_and_slide: cal each step when engine gravity wanted, skip 4 custom.
    pub fn physics_apply_gravity_3d(
        &mut self,
        body_id: NodeID,
        dt: f32,
        max_fall_speed: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        let is_char = matches!(
            self.nodes.get(body_id).map(|node| &node.data),
            Some(SceneNodeData::CharacterBody3D(_))
        );
        if !is_char || !dt.is_finite() || dt <= 0.0 {
            return None;
        }
        let gravity = self.physics_gravity();
        let fall = self.character_fall_speed_3d.entry(body_id).or_insert(0.0);
        let limit = max_fall_speed.abs().max(0.001);
        *fall = (*fall + gravity * dt).clamp(-limit, limit);
        let drop = *fall * dt;
        let global = self.get_global_transform_3d(body_id)?;
        let target = Vector3::new(
            global.position.x,
            global.position.y + drop,
            global.position.z,
        );
        let result = self.physics_move_body_3d(body_id, target, CHARACTER_MOVE_MARGIN, filter);
        if result.is_none_or(|result| result.clipped) {
            self.character_fall_speed_3d.insert(body_id, 0.0);
        }
        result
    }

    /// kp last sweep hit per char body; solver narrow phase skip
    /// kinematic-vs-fixed pairs so contacts_* merge these in.
    /// new hit node -> emit collision signal at move time (char never
    /// enters solver narrow phase vs static, so signal pass miss it).
    pub(super) fn record_character_sweep_hit_2d(
        &mut self,
        body_id: NodeID,
        result: &PhysicsMoveResult2D,
    ) {
        let is_char = matches!(
            self.nodes.get(body_id).map(|node| &node.data),
            Some(SceneNodeData::CharacterBody2D(_))
        );
        if !is_char {
            return;
        }
        match result.hit {
            Some(hit) => {
                let prev = self
                    .character_sweep_hit_2d
                    .insert(body_id, (hit.node, hit.point, hit.normal));
                if prev.is_none_or(|(node, _, _)| node != hit.node) {
                    self.emit_collision_signals_for_pairs(&[BodyPair::sorted(body_id, hit.node)]);
                }
            }
            None => {
                self.character_sweep_hit_2d.remove(&body_id);
            }
        }
    }

    pub(super) fn record_character_sweep_hit_3d(
        &mut self,
        body_id: NodeID,
        result: &PhysicsMoveResult3D,
    ) {
        let is_char = matches!(
            self.nodes.get(body_id).map(|node| &node.data),
            Some(SceneNodeData::CharacterBody3D(_))
        );
        if !is_char {
            return;
        }
        match result.hit {
            Some(hit) => {
                let prev = self
                    .character_sweep_hit_3d
                    .insert(body_id, (hit.node, hit.point, hit.normal));
                if prev.is_none_or(|(node, _, _)| node != hit.node) {
                    self.emit_collision_signals_for_pairs(&[BodyPair::sorted(body_id, hit.node)]);
                }
            }
            None => {
                self.character_sweep_hit_3d.remove(&body_id);
            }
        }
    }

    /// drop sweep hits 4 dead / re-typed bodies
    pub(super) fn prune_character_sweep_hits(&mut self) {
        let nodes = &self.nodes;
        self.character_sweep_hit_2d.retain(|id, _| {
            matches!(
                nodes.get(*id).map(|node| &node.data),
                Some(SceneNodeData::CharacterBody2D(_))
            )
        });
        self.character_sweep_hit_3d.retain(|id, _| {
            matches!(
                nodes.get(*id).map(|node| &node.data),
                Some(SceneNodeData::CharacterBody3D(_))
            )
        });
        self.character_fall_speed_2d.retain(|id, _| {
            matches!(
                nodes.get(*id).map(|node| &node.data),
                Some(SceneNodeData::CharacterBody2D(_))
            )
        });
        self.character_fall_speed_3d.retain(|id, _| {
            matches!(
                nodes.get(*id).map(|node| &node.data),
                Some(SceneNodeData::CharacterBody3D(_))
            )
        });
    }

    pub fn physics_contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        self.ensure_physics_world_synced_2d();
        let mut out = self.physics.contacts_2d(body_id);
        if let Some(&(node, point, normal)) = self.character_sweep_hit_2d.get(&body_id)
            && !out.iter().any(|contact| contact.node == node)
        {
            out.push(PhysicsContact2D {
                node,
                point,
                normal,
                impulse: 0.0,
            });
        }
        out
    }

    pub fn physics_contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        self.ensure_physics_world_synced_3d();
        let mut out = self.physics.contacts_3d(body_id);
        if let Some(&(node, point, normal)) = self.character_sweep_hit_3d.get(&body_id)
            && !out.iter().any(|contact| contact.node == node)
        {
            out.push(PhysicsContact3D {
                node,
                point,
                normal,
                impulse: 0.0,
            });
        }
        out
    }
}
