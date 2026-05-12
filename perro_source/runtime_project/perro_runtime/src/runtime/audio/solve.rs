use super::*;

impl Runtime {
    pub(super) fn solve_2d(
        &mut self,
        source_pos: Vector2,
        sound: &ActiveSpatialSound,
        physics_hit: Option<perro_runtime_context::sub_apis::PhysicsRayHit2D>,
    ) -> Option<PropagationResult> {
        let listener = self
            .resource_api
            .audio_listener_2d
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or_default();
        let listener_pos = Vector2::new(listener.position[0], listener.position[1]);
        let range = sound.options.range.max(0.0001);
        let distance = listener_pos.distance_to(source_pos);
        if distance > range.min(self.audio.config.listener_max_distance) {
            return None;
        }
        let mask_hit = if sound.options.enable_propagation && self.audio.has_audio_mask_2d {
            self.first_audio_mask_2d(listener_pos, source_pos)
        } else {
            None
        };
        let direct_attenuation = 1.0 - (distance / range).clamp(0.0, 1.0);
        let mut attenuation =
            direct_attenuation * self.emitter_attenuation_2d(sound, source_pos, listener_pos);
        let mut low_pass = 0.0;
        let mut occlusion = 0.0;
        let mut perceived = source_pos;
        let mut reflection = 0.0;
        let hit = match (physics_hit, mask_hit) {
            (Some(a), Some(b)) if b.distance < a.distance => Some(AudioHit2D {
                node: b.node,
                point: b.point,
                normal: b.normal,
                distance: b.distance,
                material: b.material,
                thickness: b.thickness,
            }),
            (Some(a), _) => {
                let material = self.audio_material_for_node(a.node).unwrap_or_default();
                Some(AudioHit2D {
                    node: a.node,
                    point: a.point,
                    normal: a.normal,
                    distance: a.distance,
                    material,
                    thickness: self.audio_thickness_2d(a.node),
                })
            }
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        if let Some(hit) = hit {
            let material = hit.material;
            let diffusion = self.audio_diffusion_for_node(hit.node);
            let thickness = hit.thickness.max(0.05) * material.thickness_multiplier;
            let transmission = material.transmission.clamp(0.0, 1.0);
            let damping = diffusion.damping.clamp(0.0, 1.0);
            let compression = diffusion.compression.clamp(0.0, 1.0);
            let hardness = diffusion.hardness.clamp(0.0, 1.0);
            occlusion = (1.0 - transmission).clamp(0.0, 1.0);
            attenuation *=
                (transmission + (0.2 + compression * 0.1) / (1.0 + thickness)).clamp(0.0, 1.0);
            low_pass = (material.low_pass_strength * (1.0 + thickness * 0.15 + damping * 0.35))
                .clamp(0.0, 1.0);
            let tangent = Vector2::new(-hit.normal.y, hit.normal.x);
            perceived = hit.point + tangent * 0.5;
            reflection = self.bounce_energy(
                (material.reflection * (0.75 + hardness * 0.5)).clamp(0.0, 1.0),
                self.audio.config.max_bounces_2d,
            );
        }
        if self.audio.has_audio_portal_2d
            && let Some(path) =
                self.best_audio_portal_2d(source_pos, listener_pos, sound.options.occlusion_mask)
        {
            let portal_strength = path.strength.clamp(0.0, 1.0);
            let portal_attenuation = 1.0 - (path.distance / range).clamp(0.0, 1.0);
            attenuation = attenuation.max(portal_attenuation * (0.65 + portal_strength * 0.35));
            low_pass *= 1.0 - portal_strength * 0.75;
            occlusion *= 1.0 - portal_strength * 0.75;
            perceived = path.exit;
            reflection = (reflection + portal_strength * 0.1).clamp(0.0, 1.0);
        }
        let (sin, cos) = (-listener.rotation_radians).sin_cos();
        let local = perceived - listener_pos;
        let local_x = local.x * cos - local.y * sin;
        let local_y = local.x * sin + local.y * cos;
        let zone = if self.audio.has_audio_zone_2d {
            self.audio_zone_mix_2d(listener_pos, source_pos)
        } else {
            AudioZoneMix::default()
        };
        low_pass = low_pass.max(zone.dampening).max(sound.effects.low_pass);
        reflection = reflection.max(zone.echo).max(sound.effects.reflection);
        let reverb_send = (reflection * 0.25)
            .max(zone.reverb_send)
            .max(zone.echo * 0.2)
            .max(sound.effects.reverb_send);
        let echo = zone.echo.max(sound.effects.echo).clamp(0.0, 1.0);
        occlusion = occlusion.max(sound.effects.occlusion);
        attenuation *= 1.0 - zone.dampening.clamp(0.0, 1.0) * 0.35;
        let result = PropagationResult {
            pan: [local_x / range, local_y / range, 0.0],
            volume: sound.volume * attenuation,
            low_pass,
            reflection,
            reverb_send,
            echo,
            occlusion,
            perceived_2d: Some(perceived),
            perceived_3d: None,
        };
        self.queue_audio_debug_ray_2d(listener_pos, perceived);
        Some(result)
    }

    pub(super) fn solve_3d(
        &mut self,
        source_pos: Vector3,
        sound: &ActiveSpatialSound,
        hit: Option<perro_runtime_context::sub_apis::PhysicsRayHit3D>,
    ) -> Option<PropagationResult> {
        let listener = self
            .resource_api
            .audio_listener_3d
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or_default();
        let listener_pos = Vector3::new(
            listener.position[0],
            listener.position[1],
            listener.position[2],
        );
        let range = sound.options.range.max(0.0001);
        let distance = listener_pos.distance_to(source_pos);
        if distance > range.min(self.audio.config.listener_max_distance) {
            return None;
        }
        let dir = listener_pos.direction_to(source_pos);
        let mut attenuation = (1.0 - (distance / range).clamp(0.0, 1.0))
            * self.emitter_attenuation_3d(sound, source_pos, listener_pos);
        let mut low_pass = 0.0;
        let mut occlusion = 0.0;
        let mut perceived = source_pos;
        let mut reflection = 0.0;
        let mask_hit = if sound.options.enable_propagation && self.audio.has_audio_mask_3d {
            self.first_audio_mask_3d(listener_pos, source_pos, sound.options.occlusion_mask)
        } else {
            None
        };
        let hit = match (hit, mask_hit) {
            (Some(a), Some(b)) if b.distance < a.distance => Some(b),
            (Some(a), _) => {
                let material = self.audio_material_for_node(a.node).unwrap_or_default();
                Some(AudioHit3D {
                    node: a.node,
                    point: a.point,
                    normal: a.normal,
                    distance: a.distance,
                    material,
                    thickness: self.audio_thickness_3d(a.node),
                })
            }
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        if let Some(hit) = hit {
            let diffusion = self.audio_diffusion_for_node(hit.node);
            let material = hit.material;
            let thickness = hit.thickness.max(0.05) * material.thickness_multiplier;
            let transmission = material.transmission.clamp(0.0, 1.0);
            let damping = diffusion.damping.clamp(0.0, 1.0);
            let compression = diffusion.compression.clamp(0.0, 1.0);
            let hardness = diffusion.hardness.clamp(0.0, 1.0);
            occlusion = (1.0 - transmission).clamp(0.0, 1.0);
            attenuation *=
                (transmission + (0.2 + compression * 0.1) / (1.0 + thickness)).clamp(0.0, 1.0);
            low_pass = (material.low_pass_strength * (1.0 + thickness * 0.1 + damping * 0.35))
                .clamp(0.0, 1.0);
            perceived = hit.point + hit.normal.cross(dir).normalized() * 0.5;
            reflection = self.bounce_energy(
                (material.reflection * (0.75 + hardness * 0.5)).clamp(0.0, 1.0),
                self.audio.config.max_bounces_3d,
            );
        }
        if self.audio.has_audio_portal_3d
            && let Some(path) = self.best_audio_portal_3d(source_pos, listener_pos)
        {
            let portal_strength = path.strength.clamp(0.0, 1.0);
            let portal_attenuation = 1.0 - (path.distance / range).clamp(0.0, 1.0);
            attenuation = attenuation.max(portal_attenuation * (0.65 + portal_strength * 0.35));
            low_pass *= 1.0 - portal_strength * 0.75;
            occlusion *= 1.0 - portal_strength * 0.75;
            perceived = path.exit;
            reflection = (reflection + portal_strength * 0.1).clamp(0.0, 1.0);
        }
        let local = inverse_rotate_vec3(listener.rotation, perceived - listener_pos);
        let zone = if self.audio.has_audio_zone_3d {
            self.audio_zone_mix_3d(listener_pos, source_pos)
        } else {
            AudioZoneMix::default()
        };
        low_pass = low_pass.max(zone.dampening).max(sound.effects.low_pass);
        reflection = reflection.max(zone.echo).max(sound.effects.reflection);
        let reverb_send = (reflection * 0.25)
            .max(zone.reverb_send)
            .max(zone.echo * 0.2)
            .max(sound.effects.reverb_send);
        let echo = zone.echo.max(sound.effects.echo).clamp(0.0, 1.0);
        occlusion = occlusion.max(sound.effects.occlusion);
        attenuation *= 1.0 - zone.dampening.clamp(0.0, 1.0) * 0.35;
        let result = PropagationResult {
            pan: [local.x / range, local.y / range, -local.z / range],
            volume: sound.volume * attenuation,
            low_pass,
            reflection,
            reverb_send,
            echo,
            occlusion,
            perceived_2d: None,
            perceived_3d: Some(perceived),
        };
        self.queue_audio_debug_ray_3d(listener_pos, perceived);
        Some(result)
    }

    pub(super) fn bounce_energy(&self, reflection: f32, max_bounces: u32) -> f32 {
        let mut energy = reflection.clamp(0.0, 1.0);
        let mut total = 0.0;
        for _ in 0..max_bounces {
            if energy < self.audio.config.energy_cutoff {
                break;
            }
            total += energy;
            energy *= reflection.clamp(0.0, 1.0);
        }
        total.clamp(0.0, 1.0)
    }

    pub(super) fn emitter_attenuation_2d(
        &mut self,
        sound: &ActiveSpatialSound,
        source_pos: Vector2,
        listener_pos: Vector2,
    ) -> f32 {
        if matches!(sound.options.direction_2d, AudioDirection::Omni) {
            return 1.0;
        }
        let Some((mode, direction)) = self.emitter_direction_2d(sound) else {
            return 1.0;
        };
        let to_listener = source_pos.direction_to(listener_pos);
        let dot = direction.dot(to_listener).clamp(-1.0, 1.0);
        emitter_lobe(mode, dot)
    }

    pub(super) fn emitter_attenuation_3d(
        &mut self,
        sound: &ActiveSpatialSound,
        source_pos: Vector3,
        listener_pos: Vector3,
    ) -> f32 {
        if matches!(sound.options.direction_3d, AudioDirection::Omni) {
            return 1.0;
        }
        let Some((mode, direction)) = self.emitter_direction_3d(sound) else {
            return 1.0;
        };
        let to_listener = source_pos.direction_to(listener_pos);
        let dot = direction.dot(to_listener).clamp(-1.0, 1.0);
        emitter_lobe(mode, dot)
    }

    fn emitter_direction_2d(
        &mut self,
        sound: &ActiveSpatialSound,
    ) -> Option<(EmitterMode, Vector2)> {
        let (mode, fallback) = match sound.options.direction_2d {
            AudioDirection::Omni => return None,
            AudioDirection::Directional(v) => (EmitterMode::Directional, v),
            AudioDirection::InverseDirectional(v) => (EmitterMode::InverseDirectional, v),
            AudioDirection::Bidirectional(v) => (EmitterMode::Bidirectional, v),
        };
        let direction = match sound.pos {
            SpatialSoundPos::Attached(node) => {
                let transform = self.get_global_transform_2d(node)?;
                Vector2::new(transform.rotation.sin(), -transform.rotation.cos())
            }
            _ => fallback,
        };
        (direction.length_squared() > 0.0001).then_some((mode, direction.normalized()))
    }

    fn emitter_direction_3d(
        &mut self,
        sound: &ActiveSpatialSound,
    ) -> Option<(EmitterMode, Vector3)> {
        let (mode, fallback) = match sound.options.direction_3d {
            AudioDirection::Omni => return None,
            AudioDirection::Directional(v) => (EmitterMode::Directional, v),
            AudioDirection::InverseDirectional(v) => (EmitterMode::InverseDirectional, v),
            AudioDirection::Bidirectional(v) => (EmitterMode::Bidirectional, v),
        };
        let direction = match sound.pos {
            SpatialSoundPos::Attached(node) => {
                let transform = self.get_global_transform_3d(node)?;
                rotate_vec3(
                    [
                        transform.rotation.x,
                        transform.rotation.y,
                        transform.rotation.z,
                        transform.rotation.w,
                    ],
                    Vector3::new(0.0, 0.0, -1.0),
                )
            }
            _ => fallback,
        };
        (direction.length_squared() > 0.0001).then_some((mode, direction.normalized()))
    }

    pub(super) fn audio_material_for_node(&self, node: NodeID) -> Option<AudioMaterial> {
        let data = &self.nodes.get(node)?.data;
        match data {
            SceneNodeData::CollisionShape2D(v) if v.audio_interaction => Some(v.audio_material),
            SceneNodeData::CollisionShape3D(v) if v.audio_interaction => Some(v.audio_material),
            SceneNodeData::StaticBody2D(v) if v.audio_interaction => Some(v.audio_material),
            SceneNodeData::StaticBody3D(v) if v.audio_interaction => Some(v.audio_material),
            SceneNodeData::RigidBody2D(v) if v.audio_interaction => Some(v.audio_material),
            SceneNodeData::RigidBody3D(v) if v.audio_interaction => Some(v.audio_material),
            SceneNodeData::AudioMask2D(v) if v.enabled => Some(v.material),
            SceneNodeData::AudioMask3D(v) if v.enabled => Some(v.material),
            _ => Some(AudioMaterial::default()),
        }
    }

    pub(super) fn audio_diffusion_for_node(&self, node: NodeID) -> AudioDiffusion {
        let Some(data) = self.nodes.get(node).map(|n| &n.data) else {
            return AudioDiffusion::default();
        };
        match data {
            SceneNodeData::CollisionShape2D(v) if v.audio_interaction => v.audio_diffusion,
            SceneNodeData::CollisionShape3D(v) if v.audio_interaction => v.audio_diffusion,
            SceneNodeData::StaticBody2D(v) if v.audio_interaction => v.audio_diffusion,
            SceneNodeData::StaticBody3D(v) if v.audio_interaction => v.audio_diffusion,
            SceneNodeData::RigidBody2D(v) if v.audio_interaction => v.audio_diffusion,
            SceneNodeData::RigidBody3D(v) if v.audio_interaction => v.audio_diffusion,
            _ => AudioDiffusion::default(),
        }
    }

    pub(super) fn audio_thickness_2d(&self, node: NodeID) -> f32 {
        self.nodes
            .get(node)
            .and_then(|n| {
                n.children_slice()
                    .iter()
                    .find_map(|child| self.nodes.get(*child))
            })
            .and_then(|n| match &n.data {
                SceneNodeData::CollisionShape2D(CollisionShape2D { shape, .. }) => match shape {
                    perro_nodes::Shape2D::Quad { width, height } => Some(width.min(*height)),
                    perro_nodes::Shape2D::Circle { radius } => Some(radius * 2.0),
                    perro_nodes::Shape2D::Triangle { width, height, .. } => {
                        Some(width.min(*height))
                    }
                },
                _ => None,
            })
            .unwrap_or(1.0)
    }

    pub(super) fn first_audio_mask_2d(&mut self, from: Vector2, to: Vector2) -> Option<AudioHit2D> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioHit2D> = None;
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioMask2D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let mask_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioMask2D(mask)) = self.nodes.get(mask_id).map(|n| &n.data)
            else {
                continue;
            };
            if !mask.enabled {
                continue;
            }
            let material = mask.material;
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(mask_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some(shape_kind) =
                    self.nodes
                        .get(child)
                        .and_then(|shape_node| match &shape_node.data {
                            SceneNodeData::CollisionShape2D(shape) => Some(shape.shape),
                            _ => None,
                        })
                else {
                    continue;
                };
                let Some(global) = self.get_global_transform_2d(child) else {
                    continue;
                };
                let (half_w, half_h) = match shape_kind {
                    perro_nodes::Shape2D::Quad { width, height } => {
                        (width.abs() * 0.5, height.abs() * 0.5)
                    }
                    perro_nodes::Shape2D::Circle { radius } => (radius.abs(), radius.abs()),
                    perro_nodes::Shape2D::Triangle { width, height, .. } => {
                        (width.abs() * 0.5, height.abs() * 0.5)
                    }
                };
                if let Some((t, normal)) = segment_aabb(from, dir, global.position, half_w, half_h)
                {
                    let distance = t * len;
                    if best.as_ref().is_none_or(|hit| distance < hit.distance) {
                        best = Some(AudioHit2D {
                            node: mask_id,
                            point: from + dir * t,
                            normal,
                            distance,
                            material,
                            thickness: (half_w.min(half_h) * 2.0).max(0.05),
                        });
                    }
                }
            }
        }
        best
    }

    pub(super) fn first_audio_mask_3d(
        &mut self,
        from: Vector3,
        to: Vector3,
        occlusion_mask: u32,
    ) -> Option<AudioHit3D> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioHit3D> = None;
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioMask3D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let mask_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioMask3D(mask)) = self.nodes.get(mask_id).map(|n| &n.data)
            else {
                continue;
            };
            if !mask.enabled || (mask.material.occlusion_mask & occlusion_mask) == 0 {
                continue;
            }
            let material = mask.material;
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(mask_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half)) = self.audio_zone_shape_3d(child) else {
                    continue;
                };
                let Some((t, normal)) = segment_aabb_3d_with_normal(from, dir, center, half) else {
                    continue;
                };
                let distance = t * len;
                if best.as_ref().is_none_or(|hit| distance < hit.distance) {
                    best = Some(AudioHit3D {
                        node: mask_id,
                        point: from + dir * t,
                        normal,
                        distance,
                        material,
                        thickness: (half.x.min(half.y).min(half.z) * 2.0).max(0.05),
                    });
                }
            }
        }
        best
    }

    pub(super) fn best_audio_portal_2d(
        &mut self,
        from: Vector2,
        to: Vector2,
        mask: u32,
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
                        mask,
                        include_areas: false,
                        exclude_nodes: Vec::new(),
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
                    mask,
                    include_areas: false,
                    exclude_nodes: Vec::new(),
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
                if !exit.enabled || target == hit.portal_id {
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

    pub(super) fn nearest_audio_portal_hit_2d(
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
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioPortal2D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let portal_id = self.audio.scratch_ids[index];
            if skip_portal == Some(portal_id) {
                continue;
            }
            let Some(SceneNodeData::AudioPortal2D(portal)) =
                self.nodes.get(portal_id).map(|n| &n.data)
            else {
                continue;
            };
            if !portal.enabled {
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
            if let Some(node) = self.nodes.get(portal_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half_w, half_h)) = self.audio_zone_shape_2d(child) else {
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

    pub(super) fn best_audio_portal_3d(
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
                if !exit.enabled || target == hit.portal_id {
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

    pub(super) fn nearest_audio_portal_hit_3d(
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
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioPortal3D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let portal_id = self.audio.scratch_ids[index];
            if skip_portal == Some(portal_id) {
                continue;
            }
            let Some(SceneNodeData::AudioPortal3D(portal)) =
                self.nodes.get(portal_id).map(|n| &n.data)
            else {
                continue;
            };
            if !portal.enabled {
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
            if let Some(node) = self.nodes.get(portal_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
            }
            for child_index in 0..self.audio.scratch_child_ids.len() {
                let child = self.audio.scratch_child_ids[child_index];
                let Some((center, half)) = self.audio_zone_shape_3d(child) else {
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

#[derive(Clone, Copy)]
enum EmitterMode {
    Directional,
    InverseDirectional,
    Bidirectional,
}

fn emitter_lobe(mode: EmitterMode, dot: f32) -> f32 {
    match mode {
        EmitterMode::Directional => directional_lobe(dot),
        EmitterMode::InverseDirectional => directional_lobe(-dot),
        EmitterMode::Bidirectional => 0.15 + 0.85 * dot.abs().powf(1.5),
    }
}

fn directional_lobe(dot: f32) -> f32 {
    0.15 + 0.85 * dot.max(0.0).powf(1.5)
}
