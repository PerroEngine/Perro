use super::*;

impl Runtime {
    pub(in super::super) fn best_audio_portal_2d(
        &mut self,
        from: Vector2,
        to: Vector2,
        mask: BitMask,
    ) -> Option<AudioPortalPath2D> {
        let initial_dir = from.direction_to(to);
        if initial_dir.length_squared() <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioPortalPath2D> = None;
        let mut stack = vec![(from, initial_dir, 0.0f32, from, 1.0f32, 0usize, 0u32, None)];
        while let Some((
            origin,
            direction,
            traveled,
            perceived,
            strength,
            hops,
            bounces,
            skip_portal,
        )) = stack.pop()
        {
            let to_listener = to - origin;
            let listener_distance = to_listener.dot(direction);
            let listener_reachable = listener_distance > 0.0001
                && (to_listener - direction * listener_distance).length()
                    <= AUDIO_PORTAL_MISS_TOLERANCE;
            let hit = self.nearest_audio_portal_hit_2d(origin, direction, skip_portal);
            if hops > 0
                && listener_reachable
                && hit
                    .as_ref()
                    .is_none_or(|hit| listener_distance < hit.distance - AUDIO_PORTAL_EPSILON)
            {
                self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
                let block_hit = self.prepared_audio_raycast_2d(
                    origin,
                    direction,
                    listener_distance,
                    &PhysicsQueryFilter {
                        layers: mask,
                        include_areas: false,
                        exclude_nodes: Vec::new(),
                        ..PhysicsQueryFilter::default()
                    },
                );
                let blocked = block_hit.is_some_and(|hit| hit.distance <= listener_distance + 0.25);
                if blocked {
                    if bounces < self.audio.config.max_bounces_2d
                        && let Some(hit) = block_hit
                        && let Some(reflected) = reflect_2d(direction, hit.normal)
                    {
                        stack.push((
                            hit.point + reflected * AUDIO_PORTAL_EPSILON,
                            reflected,
                            traveled + hit.distance,
                            perceived,
                            strength,
                            hops,
                            bounces + 1,
                            None,
                        ));
                    }
                } else {
                    let distance = traveled + listener_distance;
                    if best.as_ref().is_none_or(|path| distance < path.distance) {
                        best = Some(AudioPortalPath2D {
                            exit: perceived,
                            strength,
                            distance,
                        });
                    }
                }
            }
            let Some(hit) = hit else {
                continue;
            };
            if hops >= MAX_AUDIO_PORTAL_HOPS {
                continue;
            }
            self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
            let block_hit = self.prepared_audio_raycast_2d(
                origin,
                direction,
                hit.distance,
                &PhysicsQueryFilter {
                    layers: mask,
                    include_areas: false,
                    exclude_nodes: Vec::new(),
                    ..PhysicsQueryFilter::default()
                },
            );
            let blocked = block_hit.is_some_and(|ray_hit| {
                ray_hit.distance < hit.distance - AUDIO_PORTAL_MISS_TOLERANCE
            });
            if blocked {
                if bounces < self.audio.config.max_bounces_2d
                    && let Some(ray_hit) = block_hit
                    && let Some(reflected) = reflect_2d(direction, ray_hit.normal)
                {
                    stack.push((
                        ray_hit.point + reflected * AUDIO_PORTAL_EPSILON,
                        reflected,
                        traveled + ray_hit.distance,
                        perceived,
                        strength,
                        hops,
                        bounces + 1,
                        None,
                    ));
                }
                continue;
            }
            for target in hit.targets.iter().copied() {
                let Some(exit_transform) = self.get_global_transform_2d(target) else {
                    continue;
                };
                let Some(SceneNodeData::AudioPortal2D(exit)) =
                    self.nodes.get(target).map(|n| &n.data)
                else {
                    continue;
                };
                if !exit.active || target == hit.portal_id {
                    continue;
                }
                let exit_point = transform_point_2d(exit_transform, hit.local_entry);
                let exit_dir = transform_dir_2d(exit_transform, hit.local_dir);
                if exit_dir.length_squared() <= 0.0001 {
                    continue;
                }
                stack.push((
                    exit_point + exit_dir * AUDIO_PORTAL_EPSILON,
                    exit_dir,
                    traveled + hit.distance,
                    exit_point,
                    strength.min(hit.strength).min(exit.strength),
                    hops + 1,
                    bounces,
                    Some(target),
                ));
            }
        }
        best
    }

    pub(in super::super) fn nearest_audio_portal_hit_2d(
        &mut self,
        from: Vector2,
        direction: Vector2,
        skip_portal: Option<NodeID>,
    ) -> Option<AudioPortalHit2D> {
        if direction.length_squared() <= 0.0001 {
            return None;
        }
        let dir = direction.normalized();
        let sweep = dir * self.audio.config.max_ray_distance_2d;
        let mut best: Option<AudioPortalHit2D> = None;
        for index in 0..self.audio.audio_portal_ids_2d.len() {
            let portal_id = self.audio.audio_portal_ids_2d[index];
            if skip_portal == Some(portal_id) {
                continue;
            }
            let Some(SceneNodeData::AudioPortal2D(portal)) =
                self.nodes.get(portal_id).map(|n| &n.data)
            else {
                continue;
            };
            if !portal.active {
                continue;
            }
            let strength = portal.strength;
            let targets = portal.targets.clone();
            if targets.is_empty() {
                continue;
            }
            let Some(entry_transform) = self.get_global_transform_2d(portal_id) else {
                continue;
            };
            self.audio.scratch_child_ids.clear();
            if let Some(children) = self.nodes.children(portal_id) {
                self.audio.scratch_child_ids.extend_from_slice(children);
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half_w, half_h)) = self.audio_effect_zone_shape_2d(child) else {
                    continue;
                };
                if let Some((t, _normal)) = segment_aabb(from, sweep, center, half_w, half_h) {
                    let distance = t * self.audio.config.max_ray_distance_2d;
                    if distance <= AUDIO_PORTAL_EPSILON {
                        continue;
                    }
                    if best.as_ref().is_some_and(|hit| distance >= hit.distance) {
                        continue;
                    }
                    let entry = from + sweep * t;
                    let local_entry = inverse_transform_point_2d(entry_transform, entry);
                    best = Some(AudioPortalHit2D {
                        portal_id,
                        local_entry,
                        local_dir: inverse_transform_dir_2d(entry_transform, dir),
                        targets: targets.clone(),
                        strength,
                        distance,
                    });
                }
            }
        }
        best
    }

    pub(in super::super) fn best_audio_portal_3d(
        &mut self,
        from: Vector3,
        to: Vector3,
    ) -> Option<AudioPortalPath3D> {
        let initial_dir = from.direction_to(to);
        if initial_dir.length_squared() <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioPortalPath3D> = None;
        let mut stack = vec![(from, initial_dir, 0.0f32, from, 1.0f32, 0usize, 0u32, None)];
        while let Some((
            origin,
            direction,
            traveled,
            perceived,
            strength,
            hops,
            bounces,
            skip_portal,
        )) = stack.pop()
        {
            let to_listener = to - origin;
            let listener_distance = to_listener.dot(direction);
            let listener_reachable = listener_distance > 0.0001
                && (to_listener - direction * listener_distance).length()
                    <= AUDIO_PORTAL_MISS_TOLERANCE;
            let hit = self.nearest_audio_portal_hit_3d(origin, direction, skip_portal);
            if hops > 0
                && listener_reachable
                && hit
                    .as_ref()
                    .is_none_or(|hit| listener_distance < hit.distance - AUDIO_PORTAL_EPSILON)
            {
                self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
                let block_hit =
                    self.prepared_audio_raycast_3d(origin, direction, listener_distance, false);
                let blocked = block_hit.is_some_and(|hit| hit.distance <= listener_distance + 0.25);
                if blocked {
                    if bounces < self.audio.config.max_bounces_3d
                        && let Some(hit) = block_hit
                        && let Some(reflected) = reflect_3d(direction, hit.normal)
                    {
                        stack.push((
                            hit.point + reflected * AUDIO_PORTAL_EPSILON,
                            reflected,
                            traveled + hit.distance,
                            perceived,
                            strength,
                            hops,
                            bounces + 1,
                            None,
                        ));
                    }
                } else {
                    let distance = traveled + listener_distance;
                    if best.as_ref().is_none_or(|path| distance < path.distance) {
                        best = Some(AudioPortalPath3D {
                            exit: perceived,
                            strength,
                            distance,
                        });
                    }
                }
            }
            let Some(hit) = hit else {
                continue;
            };
            if hops >= MAX_AUDIO_PORTAL_HOPS {
                continue;
            }
            self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
            let block_hit = self.prepared_audio_raycast_3d(origin, direction, hit.distance, false);
            let blocked = block_hit.is_some_and(|ray_hit| {
                ray_hit.distance < hit.distance - AUDIO_PORTAL_MISS_TOLERANCE
            });
            if blocked {
                if bounces < self.audio.config.max_bounces_3d
                    && let Some(ray_hit) = block_hit
                    && let Some(reflected) = reflect_3d(direction, ray_hit.normal)
                {
                    stack.push((
                        ray_hit.point + reflected * AUDIO_PORTAL_EPSILON,
                        reflected,
                        traveled + ray_hit.distance,
                        perceived,
                        strength,
                        hops,
                        bounces + 1,
                        None,
                    ));
                }
                continue;
            }
            for target in hit.targets.iter().copied() {
                let Some(exit_transform) = self.get_global_transform_3d(target) else {
                    continue;
                };
                let Some(SceneNodeData::AudioPortal3D(exit)) =
                    self.nodes.get(target).map(|n| &n.data)
                else {
                    continue;
                };
                if !exit.active || target == hit.portal_id {
                    continue;
                }
                let exit_point = transform_point_3d(exit_transform, hit.local_entry);
                let exit_dir = transform_dir_3d(exit_transform, hit.local_dir);
                if exit_dir.length_squared() <= 0.0001 {
                    continue;
                }
                stack.push((
                    exit_point + exit_dir * AUDIO_PORTAL_EPSILON,
                    exit_dir,
                    traveled + hit.distance,
                    exit_point,
                    strength.min(hit.strength).min(exit.strength),
                    hops + 1,
                    bounces,
                    Some(target),
                ));
            }
        }
        best
    }

    pub(in super::super) fn nearest_audio_portal_hit_3d(
        &mut self,
        from: Vector3,
        direction: Vector3,
        skip_portal: Option<NodeID>,
    ) -> Option<AudioPortalHit3D> {
        if direction.length_squared() <= 0.0001 {
            return None;
        }
        let dir = direction.normalized();
        let sweep = dir * self.audio.config.max_ray_distance_3d;
        let mut best: Option<AudioPortalHit3D> = None;
        for index in 0..self.audio.audio_portal_ids_3d.len() {
            let portal_id = self.audio.audio_portal_ids_3d[index];
            if skip_portal == Some(portal_id) {
                continue;
            }
            let Some(SceneNodeData::AudioPortal3D(portal)) =
                self.nodes.get(portal_id).map(|n| &n.data)
            else {
                continue;
            };
            if !portal.active {
                continue;
            }
            let strength = portal.strength;
            let targets = portal.targets.clone();
            if targets.is_empty() {
                continue;
            }
            let Some(entry_transform) = self.get_global_transform_3d(portal_id) else {
                continue;
            };
            self.audio.scratch_child_ids.clear();
            if let Some(children) = self.nodes.children(portal_id) {
                self.audio.scratch_child_ids.extend_from_slice(children);
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half)) = self.audio_effect_zone_shape_3d(child) else {
                    continue;
                };
                if let Some(t) = segment_aabb_3d(from, sweep, center, half) {
                    let distance = t * self.audio.config.max_ray_distance_3d;
                    if distance <= AUDIO_PORTAL_EPSILON {
                        continue;
                    }
                    if best.as_ref().is_some_and(|hit| distance >= hit.distance) {
                        continue;
                    }
                    let entry = from + sweep * t;
                    let local_entry = inverse_transform_point_3d(entry_transform, entry);
                    best = Some(AudioPortalHit3D {
                        portal_id,
                        local_entry,
                        local_dir: inverse_transform_dir_3d(entry_transform, dir),
                        targets: targets.clone(),
                        strength,
                        distance,
                    });
                }
            }
        }
        best
    }
}
