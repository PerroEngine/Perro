use super::*;

impl Runtime {
    pub(in super::super) fn solve_2d(
        &mut self,
        source_pos: Vector2,
        sound: &mut ActiveSpatialSound,
        physics_hit: Option<perro_runtime_api::sub_apis::PhysicsRayHit2D>,
        listener: perro_pawdio::AudioListener2D,
        listener_options: perro_structs::AudioListenerOptions,
    ) -> Option<PropagationResult> {
        let listener_pos = Vector2::new(listener.position[0], listener.position[1]);
        let range = sound.options.range.max(0.0001);
        let distance = listener_pos.distance_to(source_pos);
        if distance > range.min(self.audio.config.listener_max_distance) {
            return None;
        }
        let mask_hit = if sound.options.enable_propagation && self.audio.has_audio_mask_2d {
            self.first_audio_mask_2d(listener_pos, source_pos, sound.options.audio_layer)
        } else {
            None
        };
        let direct_attenuation = distance_attenuation(distance, range);
        let mut attenuation =
            direct_attenuation * self.emitter_attenuation_2d(sound, source_pos, listener_pos);
        let unoccluded_attenuation = attenuation;
        let mut low_pass = 0.0;
        let mut occlusion = 0.0;
        let mut perceived = source_pos;
        let mut reflection = 0.0;
        let mut bounce_reverb_send = 0.0;
        let mut bounce_echo = 0.0;
        let attached_node = attached_node_of(sound);
        let physics_hit = physics_hit
            .filter(|a| Some(a.node) != attached_node && a.distance > LISTENER_EMBED_EPSILON)
            .and_then(|a| {
                let material = self.audio_material_for_node(a.node)?;
                Some(AudioHit2D {
                    node: a.node,
                    point: a.point,
                    normal: a.normal,
                    distance: a.distance,
                    material,
                    thickness: self.audio_thickness_2d(a.node),
                })
            });
        let hit = match (physics_hit, mask_hit) {
            (Some(a), Some(b)) if b.distance < a.distance => Some(AudioHit2D {
                node: b.node,
                point: b.point,
                normal: b.normal,
                distance: b.distance,
                material: b.material,
                thickness: b.thickness,
            }),
            (Some(a), _) => Some(a),
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
            let through_energy = (attenuation * transmission).clamp(0.05, 1.0);
            self.queue_audio_debug_ray_2d(
                listener_pos,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_THROUGH, through_energy),
                through_energy,
            );
            self.queue_audio_debug_ray_2d(
                hit.point,
                source_pos,
                audio_debug_color(AUDIO_DEBUG_THROUGH, through_energy),
                through_energy,
            );
            self.queue_audio_debug_ray_2d(
                hit.point,
                perceived,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, reflection),
                reflection,
            );
        }
        if hit.is_some() && sound.options.enable_propagation {
            let audio_layer = sound.options.audio_layer;
            let (openness, open_shift) = self.occlusion_openness_2d(
                &mut sound.field,
                listener_pos,
                source_pos,
                audio_layer,
                attached_node,
            );
            if openness > 0.0 {
                occlusion *= 1.0 - 0.85 * openness;
                low_pass = (low_pass * (1.0 - 0.7 * openness)).clamp(0.0, 1.0);
                // Quadratic ramp that reaches the full unoccluded level at
                // openness 1: no level jump the frame the direct ray clears.
                let open_gain =
                    openness * (AUDIO_DIFFUSION_LEAK + (1.0 - AUDIO_DIFFUSION_LEAK) * openness);
                attenuation = attenuation.max(unoccluded_attenuation * open_gain);
                // Pull the perceived position toward the open edge; a
                // symmetric opening (shift ~0) keeps the reflection point.
                if open_shift.length_squared() > 0.01 {
                    perceived = source_pos + open_shift * 0.75;
                }
            }
        }
        if self.audio.has_audio_portal_2d
            && let Some(path) =
                self.best_audio_portal_2d(source_pos, listener_pos, sound.options.audio_layer)
        {
            let portal_strength = path.strength.clamp(0.0, 1.0);
            let portal_attenuation = distance_attenuation(path.distance, range);
            attenuation = attenuation.max(portal_attenuation * (0.65 + portal_strength * 0.35));
            low_pass *= 1.0 - portal_strength * 0.75;
            occlusion *= 1.0 - portal_strength * 0.75;
            perceived = path.exit;
            reflection = (reflection + portal_strength * 0.1).clamp(0.0, 1.0);
        }
        if sound.options.enable_propagation
            && let Some(path) = self.trace_audio_bounce_path_2d(
                source_pos,
                listener_pos,
                sound.options.audio_layer,
                range,
            )
        {
            let bounce_attenuation = distance_attenuation(path.distance, range) * path.volume;
            if bounce_attenuation > attenuation {
                attenuation = bounce_attenuation;
                perceived = path.perceived;
            }
            low_pass = low_pass.max(path.low_pass);
            reflection = reflection.max(path.reflection);
            bounce_reverb_send = path.reverb_send;
            bounce_echo = path.echo;
        }
        // Bidirectional reconciliation: when the direct ray is blocked, find the
        // aperture where listener-side and source-side paths meet and let it
        // compete for attenuation + perceived position (Phase 1).
        if hit.is_some() && sound.options.enable_propagation {
            let attached = attached_node;
            let cached = sound
                .aperture_2d
                .filter(|_| sound.aperture_age < APERTURE_RESEARCH_TICKS)
                .and_then(|cache| {
                    self.verify_aperture_2d(
                        listener_pos,
                        source_pos,
                        cache.point,
                        sound.options.audio_layer,
                        attached,
                    )
                    .map(|total| (cache.point, total, cache.loss))
                });
            let aperture = match cached {
                Some(hit) => {
                    sound.aperture_age = sound.aperture_age.saturating_add(1);
                    Some(hit)
                }
                None => {
                    let found = self.reconcile_aperture_2d(
                        listener_pos,
                        source_pos,
                        sound.options.audio_layer,
                        attached,
                        range,
                    );
                    sound.aperture_2d =
                        found.map(|(point, _total, loss)| ApertureCache2D { point, loss });
                    sound.aperture_age = 0;
                    found
                }
            };
            if let Some((point, total, loss)) = aperture {
                let recon_attenuation = distance_attenuation(total, range) * loss;
                if recon_attenuation > attenuation {
                    attenuation = recon_attenuation;
                    perceived = point;
                    occlusion *= 0.35;
                    low_pass *= 0.6;
                }
                self.queue_audio_debug_ray_2d(
                    listener_pos,
                    point,
                    audio_debug_color(AUDIO_DEBUG_BOUNCE, recon_attenuation),
                    recon_attenuation,
                );
            }
        }
        let (sin, cos) = (-listener.rotation_radians).sin_cos();
        let local = perceived - listener_pos;
        let local_x = local.x * cos - local.y * sin;
        let local_y = local.x * sin + local.y * cos;
        let zone = if self.audio.has_audio_effect_zone_2d {
            self.audio_effect_zone_mix_2d(listener_pos, source_pos, sound.options.audio_layer)
        } else {
            AudioEffectZoneMix::default()
        };
        let listener_effects = listener_effect_mix(listener_options, sound.options.audio_layer);
        low_pass = low_pass.max(zone.dampening).max(sound.effects.low_pass);
        low_pass = low_pass.max(listener_effects.dampening);
        reflection = reflection
            .max(zone.echo)
            .max(bounce_echo)
            .max(listener_effects.echo)
            .max(sound.effects.reflection);
        let reverb_send = (reflection * 0.25)
            .max(zone.reverb_send)
            .max(zone.echo * 0.2)
            .max(bounce_reverb_send)
            .max(listener_effects.reverb_send)
            .max(listener_effects.echo * 0.2)
            .max(sound.effects.reverb_send);
        let echo = zone
            .echo
            .max(bounce_echo)
            .max(listener_effects.echo)
            .max(sound.effects.echo)
            .clamp(0.0, 1.0);
        occlusion = occlusion.max(sound.effects.occlusion);
        attenuation *= 1.0
            - zone
                .dampening
                .max(listener_effects.dampening)
                .clamp(0.0, 1.0)
                * 0.35;
        let result = PropagationResult {
            pan: spatial_pan([local_x, local_y, 0.0]),
            volume: sound.volume * attenuation,
            low_pass,
            reflection,
            reverb_send,
            echo,
            occlusion,
            perceived_2d: Some(perceived),
            perceived_3d: None,
        };
        if hit.is_none() {
            self.queue_audio_debug_ray_2d(listener_pos, perceived, AUDIO_DEBUG_DIRECT, attenuation);
        }
        Some(result)
    }

    pub(in super::super) fn solve_3d(
        &mut self,
        source_pos: Vector3,
        sound: &mut ActiveSpatialSound,
        hit: Option<perro_runtime_api::sub_apis::PhysicsRayHit3D>,
        listener: perro_pawdio::AudioListener3D,
        listener_options: perro_structs::AudioListenerOptions,
    ) -> Option<PropagationResult> {
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
        let mut attenuation = distance_attenuation(distance, range)
            * self.emitter_attenuation_3d(sound, source_pos, listener_pos);
        let unoccluded_attenuation = attenuation;
        let mut low_pass = 0.0;
        let mut occlusion = 0.0;
        let mut perceived = source_pos;
        let mut reflection = 0.0;
        let mut bounce_reverb_send = 0.0;
        let mut bounce_echo = 0.0;
        let mask_hit = if sound.options.enable_propagation && self.audio.has_audio_mask_3d {
            self.first_audio_mask_3d(listener_pos, source_pos, sound.options.audio_layer)
        } else {
            None
        };
        let attached_node = attached_node_of(sound);
        let physics_hit = hit
            .filter(|a| Some(a.node) != attached_node && a.distance > LISTENER_EMBED_EPSILON)
            .and_then(|a| {
                let material = self.audio_material_for_node(a.node)?;
                Some(AudioHit3D {
                    node: a.node,
                    point: a.point,
                    normal: a.normal,
                    distance: a.distance,
                    material,
                    thickness: self.audio_thickness_3d(a.node),
                })
            });
        let hit = match (physics_hit, mask_hit) {
            (Some(a), Some(b)) if b.distance < a.distance => Some(b),
            (Some(a), _) => Some(a),
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
            let through_energy = (attenuation * transmission).clamp(0.05, 1.0);
            self.queue_audio_debug_ray_3d(
                listener_pos,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_THROUGH, through_energy),
                through_energy,
            );
            self.queue_audio_debug_ray_3d(
                hit.point,
                source_pos,
                audio_debug_color(AUDIO_DEBUG_THROUGH, through_energy),
                through_energy,
            );
            if let Some(reflected) = reflect_3d(dir, hit.normal) {
                self.queue_audio_debug_ray_3d(
                    hit.point,
                    hit.point + reflected * 0.8,
                    audio_debug_color(AUDIO_DEBUG_BOUNCE, reflection),
                    reflection,
                );
            }
            let absorbed = (material.absorption.clamp(0.0, 1.0) * occlusion).clamp(0.0, 1.0);
            if absorbed > 0.01 {
                self.queue_audio_debug_absorption_3d(hit.point, hit.normal, absorbed);
            }
        }
        if hit.is_some() && sound.options.enable_propagation {
            let audio_layer = sound.options.audio_layer;
            let (openness, open_shift) = self.occlusion_openness_3d(
                &mut sound.field,
                listener_pos,
                source_pos,
                audio_layer,
                attached_node,
            );
            if openness > 0.0 {
                occlusion *= 1.0 - 0.85 * openness;
                low_pass = (low_pass * (1.0 - 0.7 * openness)).clamp(0.0, 1.0);
                // Quadratic ramp that reaches the full unoccluded level at
                // openness 1: no level jump the frame the direct ray clears.
                let open_gain =
                    openness * (AUDIO_DIFFUSION_LEAK + (1.0 - AUDIO_DIFFUSION_LEAK) * openness);
                attenuation = attenuation.max(unoccluded_attenuation * open_gain);
                // Pull the perceived position toward the open edge; a
                // symmetric opening (shift ~0) keeps the reflection point.
                if open_shift.length_squared() > 0.01 {
                    perceived = source_pos + open_shift * 0.75;
                }
            }
        }
        if self.audio.has_audio_portal_3d
            && let Some(path) = self.best_audio_portal_3d(source_pos, listener_pos)
        {
            let portal_strength = path.strength.clamp(0.0, 1.0);
            let portal_attenuation = distance_attenuation(path.distance, range);
            attenuation = attenuation.max(portal_attenuation * (0.65 + portal_strength * 0.35));
            low_pass *= 1.0 - portal_strength * 0.75;
            occlusion *= 1.0 - portal_strength * 0.75;
            perceived = path.exit;
            reflection = (reflection + portal_strength * 0.1).clamp(0.0, 1.0);
        }
        if sound.options.enable_propagation
            && let Some(path) = self.trace_audio_bounce_path_3d(
                source_pos,
                listener_pos,
                sound.options.audio_layer,
                range,
            )
        {
            let bounce_attenuation = distance_attenuation(path.distance, range) * path.volume;
            if bounce_attenuation > attenuation {
                attenuation = bounce_attenuation;
                perceived = path.perceived;
            }
            low_pass = low_pass.max(path.low_pass);
            reflection = reflection.max(path.reflection);
            bounce_reverb_send = path.reverb_send;
            bounce_echo = path.echo;
        }
        // Bidirectional reconciliation (Phase 1): see solve_2d.
        if hit.is_some() && sound.options.enable_propagation {
            let attached = attached_node;
            let cached = sound
                .aperture_3d
                .filter(|_| sound.aperture_age < APERTURE_RESEARCH_TICKS)
                .and_then(|cache| {
                    self.verify_aperture_3d(
                        listener_pos,
                        source_pos,
                        cache.point,
                        sound.options.audio_layer,
                        attached,
                    )
                    .map(|total| (cache.point, total, cache.loss))
                });
            let aperture = match cached {
                Some(hit) => {
                    sound.aperture_age = sound.aperture_age.saturating_add(1);
                    Some(hit)
                }
                None => {
                    let found = self.reconcile_aperture_3d(
                        listener_pos,
                        source_pos,
                        sound.options.audio_layer,
                        attached,
                        range,
                    );
                    sound.aperture_3d =
                        found.map(|(point, _total, loss)| ApertureCache3D { point, loss });
                    sound.aperture_age = 0;
                    found
                }
            };
            if let Some((point, total, loss)) = aperture {
                let recon_attenuation = distance_attenuation(total, range) * loss;
                if recon_attenuation > attenuation {
                    attenuation = recon_attenuation;
                    perceived = point;
                    occlusion *= 0.35;
                    low_pass *= 0.6;
                }
                self.queue_audio_debug_ray_3d(
                    listener_pos,
                    point,
                    audio_debug_color(AUDIO_DEBUG_BOUNCE, recon_attenuation),
                    recon_attenuation,
                );
            }
        }
        let local = inverse_rotate_vec3(listener.rotation, perceived - listener_pos);
        // Ears pan only on the horizontal axis; give strongly above/below
        // sources a subtle darkening so elevation still reads.
        let local_len = local.length();
        if local_len > 0.0001 {
            low_pass = low_pass.max((local.y.abs() / local_len) * 0.12);
        }
        let zone = if self.audio.has_audio_effect_zone_3d {
            self.audio_effect_zone_mix_3d(listener_pos, source_pos, sound.options.audio_layer)
        } else {
            AudioEffectZoneMix::default()
        };
        let listener_effects = listener_effect_mix(listener_options, sound.options.audio_layer);
        low_pass = low_pass.max(zone.dampening).max(sound.effects.low_pass);
        low_pass = low_pass.max(listener_effects.dampening);
        reflection = reflection
            .max(zone.echo)
            .max(bounce_echo)
            .max(listener_effects.echo)
            .max(sound.effects.reflection);
        let reverb_send = (reflection * 0.25)
            .max(zone.reverb_send)
            .max(zone.echo * 0.2)
            .max(bounce_reverb_send)
            .max(listener_effects.reverb_send)
            .max(listener_effects.echo * 0.2)
            .max(sound.effects.reverb_send);
        let echo = zone
            .echo
            .max(bounce_echo)
            .max(listener_effects.echo)
            .max(sound.effects.echo)
            .clamp(0.0, 1.0);
        occlusion = occlusion.max(sound.effects.occlusion);
        attenuation *= 1.0
            - zone
                .dampening
                .max(listener_effects.dampening)
                .clamp(0.0, 1.0)
                * 0.35;
        let result = PropagationResult {
            pan: spatial_pan([local.x, local.y, -local.z]),
            volume: sound.volume * attenuation,
            low_pass,
            reflection,
            reverb_send,
            echo,
            occlusion,
            perceived_2d: None,
            perceived_3d: Some(perceived),
        };
        if hit.is_none() {
            self.queue_audio_debug_ray_3d(listener_pos, perceived, AUDIO_DEBUG_DIRECT, attenuation);
        }
        Some(result)
    }

    // Probe a small cloud around an occluded source ("sound wave cloud") to let
    // energy diffract around edges. Phase 2: only PROBE_SLICE probes are
    // recast per tick (round-robin via `field.cursor`); the rest reuse their
    // stored open weight. The returned openness is the hysteresis-blended
    // integral of all seen probes, so it does not flicker when one probe flips
    // on alternate ticks. Returns (smoothed open fraction, average open offset).
}
