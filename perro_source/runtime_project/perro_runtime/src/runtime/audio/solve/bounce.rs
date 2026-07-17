use super::*;

impl Runtime {
    pub(in super::super) fn bounce_energy(&self, reflection: f32, max_bounces: u32) -> f32 {
        let mut energy = reflection.clamp(0.0, 1.0);
        let mut total = 0.0;
        for _ in 0..bounded_audio_bounces(max_bounces) {
            if energy < self.audio.config.energy_cutoff {
                break;
            }
            total += energy;
            energy *= reflection.clamp(0.0, 1.0);
        }
        total.clamp(0.0, 1.0)
    }

    pub(super) fn queue_audio_debug_absorption_3d(
        &mut self,
        point: Vector3,
        normal: Vector3,
        energy: f32,
    ) {
        let mut tangent = normal.cross(Vector3::new(0.0, 1.0, 0.0));
        if tangent.length_squared() <= 0.0001 {
            tangent = normal.cross(Vector3::new(1.0, 0.0, 0.0));
        }
        if tangent.length_squared() <= 0.0001 {
            self.queue_audio_debug_point_3d(
                point,
                audio_debug_color(AUDIO_DEBUG_ABSORB, energy),
                energy,
            );
            return;
        }
        tangent = tangent.normalized();
        let bitangent = normal.cross(tangent).normalized();
        let scale = 0.18 + energy.clamp(0.0, 1.0) * 0.22;
        for offset in [
            tangent * scale,
            -tangent * scale,
            bitangent * scale,
            -bitangent * scale,
            (tangent + bitangent).normalized() * scale * 0.8,
        ] {
            self.queue_audio_debug_point_3d(
                point + offset,
                audio_debug_color(AUDIO_DEBUG_ABSORB, energy),
                energy,
            );
        }
    }

    pub(in super::super) fn trace_audio_bounce_path_2d(
        &mut self,
        source: Vector2,
        listener: Vector2,
        audio_layer: BitMask,
        range: f32,
    ) -> Option<AudioBouncePath2D> {
        let mut best = self.trace_audio_bounce_ray_2d(
            source,
            listener,
            source.direction_to(listener),
            audio_layer,
            range,
            false,
        );
        for i in 0..AUDIO_BOUNCE_RAYS_2D {
            let angle = i as f32 * TAU / AUDIO_BOUNCE_RAYS_2D as f32;
            let direction = Vector2::new(angle.cos(), angle.sin());
            if let Some(path) = self.trace_audio_bounce_ray_2d(
                source,
                listener,
                direction,
                audio_layer,
                range,
                false,
            ) && best
                .as_ref()
                .is_none_or(|best| path.volume > best.volume || path.distance < best.distance)
            {
                best = Some(path);
            }
        }
        // (The listener-emitted reverse pass is subsumed by the bidirectional
        // reconciler in solve_2d, which finds window-exit apertures directly.)
        best
    }

    // `perceive_first`: track the first bounce point instead of the last, for
    // listener-emitted rays where the reflector nearest the ear is the one the
    // sound should appear to come from.
    pub(super) fn trace_audio_bounce_ray_2d(
        &mut self,
        source: Vector2,
        listener: Vector2,
        initial_direction: Vector2,
        audio_layer: BitMask,
        range: f32,
        perceive_first: bool,
    ) -> Option<AudioBouncePath2D> {
        let mut origin = source;
        let mut direction = initial_direction;
        if direction.length_squared() <= 0.0001 {
            return None;
        }
        direction = direction.normalized();
        let mut traveled = 0.0;
        let mut volume = 1.0;
        let mut reflection: f32 = 0.0;
        let mut reverb_send: f32 = 0.0;
        let mut echo: f32 = 0.0;
        let mut low_pass: f32 = 0.0;
        let mut perceived = source;
        let mut bounced = false;
        for _ in 0..self.audio.config.max_bounces_2d {
            let remaining = (range - traveled)
                .min(self.audio.config.max_ray_distance_2d)
                .max(0.0);
            if remaining <= AUDIO_PORTAL_EPSILON {
                break;
            }
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                break;
            }
            let to_listener = listener - origin;
            let listener_distance = to_listener.dot(direction);
            let listener_reachable = listener_distance > AUDIO_PORTAL_EPSILON
                && listener_distance <= remaining
                && (to_listener - direction * listener_distance).length()
                    <= AUDIO_PORTAL_MISS_TOLERANCE;
            let hit = self.nearest_audio_bounce_hit_2d(origin, direction, remaining, audio_layer);
            if listener_reachable
                && hit
                    .as_ref()
                    .is_none_or(|hit| listener_distance < hit.distance - AUDIO_PORTAL_EPSILON)
            {
                if !bounced {
                    return None;
                }
                return Some(AudioBouncePath2D {
                    perceived,
                    distance: traveled + listener_distance,
                    reflection,
                    reverb_send,
                    echo,
                    low_pass,
                    volume,
                });
            }
            let Some(hit) = hit else {
                break;
            };
            // A listener-emitted ray's first hit at t ~ 0 is the listener's own
            // body (camera inside a character collider); not a real reflector.
            if perceive_first && !bounced && hit.distance <= LISTENER_EMBED_EPSILON {
                break;
            }
            let Some(reflected) = reflect_2d(direction, hit.normal) else {
                break;
            };
            let reflect_energy = hit.reflection.clamp(0.0, 1.0);
            if reflect_energy < self.audio.config.energy_cutoff {
                break;
            }
            if !(perceive_first && bounced) {
                perceived = hit.point;
            }
            bounced = true;
            traveled += hit.distance;
            volume *= (hit.volume_loss * reflect_energy).clamp(0.0, 1.0);
            if volume < self.audio.config.energy_cutoff {
                break;
            }
            reflection = reflection.max(reflect_energy);
            reverb_send = reverb_send.max(hit.reverb_send);
            echo = echo.max(hit.echo);
            low_pass = low_pass.max(hit.low_pass);
            self.queue_audio_debug_ray_2d(
                origin,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, volume),
                volume,
            );
            origin = hit.point + reflected * AUDIO_PORTAL_EPSILON;
            direction = reflected;
        }
        None
    }

    pub(in super::super) fn trace_audio_bounce_path_3d(
        &mut self,
        source: Vector3,
        listener: Vector3,
        audio_layer: BitMask,
        range: f32,
    ) -> Option<AudioBouncePath3D> {
        let mut best = self.trace_audio_bounce_ray_3d(
            source,
            listener,
            source.direction_to(listener),
            audio_layer,
            range,
            false,
        );
        for i in 0..AUDIO_BOUNCE_RAYS_3D {
            let n = AUDIO_BOUNCE_RAYS_3D as f32;
            let y = 1.0 - (i as f32 + 0.5) * 2.0 / n;
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let theta = i as f32 * 2.399_963_1;
            let direction = Vector3::new(theta.cos() * radius, y, theta.sin() * radius);
            if let Some(path) = self.trace_audio_bounce_ray_3d(
                source,
                listener,
                direction,
                audio_layer,
                range,
                false,
            ) && best
                .as_ref()
                .is_none_or(|best| path.volume > best.volume || path.distance < best.distance)
            {
                best = Some(path);
            }
        }
        // (The listener-emitted reverse pass is subsumed by the bidirectional
        // reconciler in solve_3d.)
        best
    }

    // `perceive_first`: track the first bounce point instead of the last, for
    // listener-emitted rays where the reflector nearest the ear is the one the
    // sound should appear to come from.
    pub(super) fn trace_audio_bounce_ray_3d(
        &mut self,
        source: Vector3,
        listener: Vector3,
        initial_direction: Vector3,
        audio_layer: BitMask,
        range: f32,
        perceive_first: bool,
    ) -> Option<AudioBouncePath3D> {
        let mut origin = source;
        let mut direction = initial_direction;
        if direction.length_squared() <= 0.0001 {
            return None;
        }
        direction = direction.normalized();
        let mut traveled = 0.0;
        let mut volume = 1.0;
        let mut reflection: f32 = 0.0;
        let mut reverb_send: f32 = 0.0;
        let mut echo: f32 = 0.0;
        let mut low_pass: f32 = 0.0;
        let mut perceived = source;
        let mut bounced = false;
        for _ in 0..self.audio.config.max_bounces_3d {
            let remaining = (range - traveled)
                .min(self.audio.config.max_ray_distance_3d)
                .max(0.0);
            if remaining <= AUDIO_PORTAL_EPSILON {
                break;
            }
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                break;
            }
            let to_listener = listener - origin;
            let listener_distance = to_listener.dot(direction);
            let listener_reachable = listener_distance > AUDIO_PORTAL_EPSILON
                && listener_distance <= remaining
                && (to_listener - direction * listener_distance).length()
                    <= AUDIO_PORTAL_MISS_TOLERANCE;
            let hit = self.nearest_audio_bounce_hit_3d(origin, direction, remaining, audio_layer);
            if listener_reachable
                && hit
                    .as_ref()
                    .is_none_or(|hit| listener_distance < hit.distance - AUDIO_PORTAL_EPSILON)
            {
                if !bounced {
                    return None;
                }
                return Some(AudioBouncePath3D {
                    perceived,
                    distance: traveled + listener_distance,
                    reflection,
                    reverb_send,
                    echo,
                    low_pass,
                    volume,
                });
            }
            let Some(hit) = hit else {
                break;
            };
            // A listener-emitted ray's first hit at t ~ 0 is the listener's own
            // body (camera inside a character collider); not a real reflector.
            if perceive_first && !bounced && hit.distance <= LISTENER_EMBED_EPSILON {
                break;
            }
            let Some(reflected) = reflect_3d(direction, hit.normal) else {
                break;
            };
            let reflect_energy = hit.reflection.clamp(0.0, 1.0);
            if reflect_energy < self.audio.config.energy_cutoff {
                break;
            }
            if !(perceive_first && bounced) {
                perceived = hit.point;
            }
            bounced = true;
            traveled += hit.distance;
            volume *= (hit.volume_loss * reflect_energy).clamp(0.0, 1.0);
            if volume < self.audio.config.energy_cutoff {
                break;
            }
            reflection = reflection.max(reflect_energy);
            reverb_send = reverb_send.max(hit.reverb_send);
            echo = echo.max(hit.echo);
            low_pass = low_pass.max(hit.low_pass);
            self.queue_audio_debug_ray_3d(
                origin,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, volume),
                volume,
            );
            origin = hit.point + reflected * AUDIO_PORTAL_EPSILON;
            direction = reflected;
        }
        None
    }

    pub(super) fn nearest_audio_bounce_hit_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        audio_layer: BitMask,
    ) -> Option<AudioBounceHit2D> {
        self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
        let mut best = self
            .prepared_audio_raycast_2d(
                origin,
                direction,
                max_distance,
                &PhysicsQueryFilter {
                    layers: audio_layer,
                    include_areas: false,
                    exclude_nodes: Vec::new(),
                    ..PhysicsQueryFilter::default()
                },
            )
            .and_then(|hit| self.physics_bounce_hit_2d(hit));
        if let Some(hit) =
            self.first_audio_mask_2d(origin, origin + direction * max_distance, audio_layer)
        {
            let mask_hit = self.material_bounce_hit_2d(hit);
            if best
                .as_ref()
                .is_none_or(|best| mask_hit.distance < best.distance)
            {
                best = Some(mask_hit);
            }
        }
        if self.audio.has_audio_effect_zone_2d
            && let Some(zone_hit) =
                self.first_audio_bounce_zone_2d(origin, direction, max_distance, audio_layer)
            && best
                .as_ref()
                .is_none_or(|best| zone_hit.distance < best.distance)
        {
            best = Some(zone_hit);
        }
        best
    }

    pub(super) fn nearest_audio_bounce_hit_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        audio_layer: BitMask,
    ) -> Option<AudioBounceHit3D> {
        self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
        let mut best = self
            .prepared_audio_raycast_3d(origin, direction, max_distance, false)
            .and_then(|hit| self.physics_bounce_hit_3d(hit));
        if let Some(hit) =
            self.first_audio_mask_3d(origin, origin + direction * max_distance, audio_layer)
        {
            let mask_hit = self.material_bounce_hit_3d(hit);
            if best
                .as_ref()
                .is_none_or(|best| mask_hit.distance < best.distance)
            {
                best = Some(mask_hit);
            }
        }
        if self.audio.has_audio_effect_zone_3d
            && let Some(zone_hit) =
                self.first_audio_bounce_zone_3d(origin, direction, max_distance, audio_layer)
            && best
                .as_ref()
                .is_none_or(|best| zone_hit.distance < best.distance)
        {
            best = Some(zone_hit);
        }
        best
    }

    pub(super) fn physics_bounce_hit_2d(
        &self,
        hit: perro_runtime_api::sub_apis::PhysicsRayHit2D,
    ) -> Option<AudioBounceHit2D> {
        let material = self.audio_material_for_node(hit.node)?;
        let thickness = self.audio_thickness_2d(hit.node);
        Some(self.material_bounce_hit_2d(AudioHit2D {
            node: hit.node,
            point: hit.point,
            normal: hit.normal,
            distance: hit.distance,
            material,
            thickness,
        }))
    }

    pub(super) fn physics_bounce_hit_3d(
        &self,
        hit: perro_runtime_api::sub_apis::PhysicsRayHit3D,
    ) -> Option<AudioBounceHit3D> {
        let material = self.audio_material_for_node(hit.node)?;
        let thickness = self.audio_thickness_3d(hit.node);
        Some(self.material_bounce_hit_3d(AudioHit3D {
            node: hit.node,
            point: hit.point,
            normal: hit.normal,
            distance: hit.distance,
            material,
            thickness,
        }))
    }

    pub(super) fn material_bounce_hit_2d(&self, hit: AudioHit2D) -> AudioBounceHit2D {
        let diffusion = self.audio_diffusion_for_node(hit.node);
        let thickness = hit.thickness.max(0.05) * hit.material.thickness_multiplier;
        let damping = diffusion.damping.clamp(0.0, 1.0);
        let hardness = diffusion.hardness.clamp(0.0, 1.0);
        let absorption = hit.material.absorption.clamp(0.0, 1.0);
        let reflection = (hit.material.reflection * (1.0 - absorption) * (0.75 + hardness * 0.5))
            .clamp(0.0, 1.0);
        AudioBounceHit2D {
            point: hit.point,
            normal: hit.normal,
            distance: hit.distance,
            reflection,
            reverb_send: reflection * 0.25,
            echo: reflection * 0.35,
            low_pass: (hit.material.low_pass_strength * (0.5 + damping * 0.35)).clamp(0.0, 1.0),
            volume_loss: ((1.0 - absorption * 0.5) / (1.0 + thickness * 0.05)).clamp(0.0, 1.0),
        }
    }

    pub(super) fn material_bounce_hit_3d(&self, hit: AudioHit3D) -> AudioBounceHit3D {
        let diffusion = self.audio_diffusion_for_node(hit.node);
        let thickness = hit.thickness.max(0.05) * hit.material.thickness_multiplier;
        let damping = diffusion.damping.clamp(0.0, 1.0);
        let hardness = diffusion.hardness.clamp(0.0, 1.0);
        let absorption = hit.material.absorption.clamp(0.0, 1.0);
        let reflection = (hit.material.reflection * (1.0 - absorption) * (0.75 + hardness * 0.5))
            .clamp(0.0, 1.0);
        AudioBounceHit3D {
            point: hit.point,
            normal: hit.normal,
            distance: hit.distance,
            reflection,
            reverb_send: reflection * 0.25,
            echo: reflection * 0.35,
            low_pass: (hit.material.low_pass_strength * (0.5 + damping * 0.35)).clamp(0.0, 1.0),
            volume_loss: ((1.0 - absorption * 0.5) / (1.0 + thickness * 0.05)).clamp(0.0, 1.0),
        }
    }
}
