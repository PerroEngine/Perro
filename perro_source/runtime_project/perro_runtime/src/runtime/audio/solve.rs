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
        let mut attenuation = direct_attenuation;
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
            if self.audio.has_audio_portal_2d
                && let Some((portal_point, portal_strength)) =
                    self.best_audio_portal_2d(listener_pos, source_pos)
            {
                let portal_strength = portal_strength.clamp(0.0, 1.0);
                attenuation = attenuation.max(direct_attenuation * (0.65 + portal_strength * 0.35));
                low_pass *= 1.0 - portal_strength * 0.75;
                occlusion *= 1.0 - portal_strength * 0.75;
                perceived = portal_point;
                reflection = (reflection + portal_strength * 0.1).clamp(0.0, 1.0);
            }
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
        Some(PropagationResult {
            pan: [local_x / range, local_y / range, 0.0],
            volume: sound.volume * attenuation,
            low_pass,
            reflection,
            reverb_send,
            echo,
            occlusion,
            perceived_2d: Some(perceived),
            perceived_3d: None,
        })
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
        let mut attenuation = 1.0 - (distance / range).clamp(0.0, 1.0);
        let mut low_pass = 0.0;
        let mut occlusion = 0.0;
        let mut perceived = source_pos;
        let mut reflection = 0.0;
        if let Some(hit) = hit {
            let material = self.audio_material_for_node(hit.node).unwrap_or_default();
            let diffusion = self.audio_diffusion_for_node(hit.node);
            let thickness =
                self.audio_thickness_3d(hit.node).max(0.05) * material.thickness_multiplier;
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
        let local = inverse_rotate_vec3(listener.rotation, source_pos - listener_pos);
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
        Some(PropagationResult {
            pan: [local.x / range, local.y / range, -local.z / range],
            volume: sound.volume * attenuation,
            low_pass,
            reflection,
            reverb_send,
            echo,
            occlusion,
            perceived_2d: None,
            perceived_3d: Some(perceived),
        })
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

    pub(super) fn best_audio_portal_2d(
        &mut self,
        from: Vector2,
        to: Vector2,
    ) -> Option<(Vector2, f32)> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<(Vector2, f32, f32)> = None;
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioPortal2D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let portal_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioPortal2D(portal)) =
                self.nodes.get(portal_id).map(|n| &n.data)
            else {
                continue;
            };
            if !portal.enabled {
                continue;
            }
            let strength = portal.strength;
            self.audio.scratch_child_ids.clear();
            if let Some(node) = self.nodes.get(portal_id) {
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
                if let Some((t, _normal)) = segment_aabb(from, dir, global.position, half_w, half_h)
                {
                    let distance = t * len;
                    if best
                        .as_ref()
                        .is_none_or(|(_, _, best_distance)| distance < *best_distance)
                    {
                        best = Some((from + dir * t, strength, distance));
                    }
                }
            }
        }
        best.map(|(point, strength, _)| (point, strength))
    }
}
