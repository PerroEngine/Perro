use super::*;

// Hits this close to the listener are the listener's own body (camera inside a
// character collider casts solid rays with t = 0); ignore them.
const LISTENER_EMBED_EPSILON: f32 = 0.05;

// Occluded sounds probe a small cloud of points around the source; the open
// fraction lets energy diffract around edges instead of a binary wall cutoff.
// Two rings: probes near the direct line weigh more than the wide ring, so
// openness rises gradually as the listener sweeps past a corner instead of
// snapping between coarse fractions.
const AUDIO_DIFFUSION_SPREAD: f32 = 1.25;
const AUDIO_DIFFUSION_SPREAD_NEAR: f32 = 0.45;
const AUDIO_DIFFUSION_NEAR_WEIGHT: f32 = 1.0;
const AUDIO_DIFFUSION_FAR_WEIGHT: f32 = 0.6;
// How much of the unoccluded level leaks through a fully open side.
const AUDIO_DIFFUSION_LEAK: f32 = 0.6;

// --- Bidirectional ray reconciliation (Phase 1) ---
// A listener-side and source-side path point reconcile into one aperture when a
// verification raycast between them is unobstructed. Pairs within
// RECONCILE_EPSILON are treated as coincident (tightest matches, preferred in
// scoring); pairs out to RECONCILE_VERIFY_MAX still reconcile if the connecting
// segment is clear. Both cases verify the segment so points straddling a thin
// wall never falsely reconcile.
const RECONCILE_EPSILON: f32 = 0.5;
const RECONCILE_VERIFY_MAX: f32 = 2.0;
// Spacing between free-segment sample points (world units). Fixed spacing keeps
// sample density high near apertures regardless of how far a ray travels, so
// listener-side and source-side points can reconcile within RECONCILE_VERIFY_MAX.
const RECONCILE_SAMPLE_SPACING: f32 = 0.75;
// Cap on samples per segment to bound work on very long free rays.
const RECONCILE_MAX_SAMPLES: usize = 32;
// Full fan re-search cadence: verified caches survive this many ticks before a
// forced re-search even when verification keeps passing.
const APERTURE_RESEARCH_TICKS: u32 = 10;
// Number of ray directions per side in the reconciling fan (2D).
const RECONCILE_FAN_2D: usize = AUDIO_BOUNCE_RAYS_2D;
const RECONCILE_FAN_3D: usize = AUDIO_BOUNCE_RAYS_3D;

// Persistent field (Phase 2): probes refreshed per tick, round-robin. The rest
// keep their stored value, so total openness rays/tick drop from the full fan
// (4/6) to PROBE_SLICE while the blended openness stays stable.
const PROBE_SLICE: usize = 2;
// Openness hysteresis: opening reads fast, closing fades slow (mirror
// smooth_volume in scene.rs) so a probe flipping on alternate ticks does not
// oscillate the level.
const OPENNESS_RISE: f32 = 0.6;
const OPENNESS_FALL: f32 = 0.25;

fn attached_node_of(sound: &ActiveSpatialSound) -> Option<NodeID> {
    match sound.pos {
        SpatialSoundPos::Attached(node) => Some(node),
        _ => None,
    }
}

const AUDIO_DEBUG_DIRECT: [f32; 4] = [0.1, 1.0, 0.55, 1.0];
const AUDIO_DEBUG_THROUGH: [f32; 4] = [0.68, 0.18, 1.0, 0.9];
const AUDIO_DEBUG_BOUNCE: [f32; 4] = [1.0, 0.56, 0.12, 0.9];
const AUDIO_DEBUG_ABSORB: [f32; 4] = [0.34, 0.18, 0.74, 0.55];

#[inline]
fn audio_debug_color(color: [f32; 4], energy: f32) -> [f32; 4] {
    let strength = (0.25 + energy.clamp(0.0, 1.0).sqrt() * 0.75).clamp(0.0, 1.0);
    [
        color[0] * strength,
        color[1] * strength,
        color[2] * strength,
        color[3],
    ]
}

impl Runtime {
    pub(super) fn solve_2d(
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

    pub(super) fn solve_3d(
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
    fn occlusion_openness_2d(
        &mut self,
        field: &mut PropagationField,
        listener_pos: Vector2,
        source_pos: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> (f32, Vector2) {
        let to_source = source_pos - listener_pos;
        let distance = to_source.length();
        if distance <= 0.0001 {
            return (0.0, Vector2::ZERO);
        }
        let dir = to_source * distance.recip();
        let perp = Vector2::new(-dir.y, dir.x);
        let offsets = [
            (
                perp * AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (
                perp * -AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (perp * AUDIO_DIFFUSION_SPREAD, AUDIO_DIFFUSION_FAR_WEIGHT),
            (perp * -AUDIO_DIFFUSION_SPREAD, AUDIO_DIFFUSION_FAR_WEIGHT),
        ];
        let n = offsets.len();
        let exclude: Vec<NodeID> = attached_node.into_iter().collect();
        // First-time sample: probe every slot so openness starts accurate.
        let refresh = if field.initialized { PROBE_SLICE } else { n };
        for step in 0..refresh {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                break;
            }
            let idx = (field.cursor + step) % n;
            let (offset, _weight) = offsets[idx];
            let probe = source_pos + offset;
            let probe_delta = probe - listener_pos;
            let probe_dist = probe_delta.length();
            if probe_dist <= 0.0001 {
                continue;
            }
            self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
            let blocked_physics = self
                .prepared_audio_raycast_2d(
                    listener_pos,
                    probe_delta * probe_dist.recip(),
                    probe_dist,
                    &PhysicsQueryFilter {
                        layers: audio_layer,
                        include_areas: false,
                        exclude_nodes: exclude.clone(),
                        ..PhysicsQueryFilter::default()
                    },
                )
                .is_some_and(|hit| hit.distance > LISTENER_EMBED_EPSILON);
            let blocked = blocked_physics
                || (self.audio.has_audio_mask_2d
                    && self
                        .first_audio_mask_2d(listener_pos, probe, audio_layer)
                        .is_some());
            field.probe_open[idx] = if blocked { 0.0 } else { 1.0 };
            field.probe_seen[idx] = true;
            if !blocked {
                self.queue_audio_debug_ray_2d(
                    listener_pos,
                    probe,
                    audio_debug_color(AUDIO_DEBUG_DIRECT, 0.5),
                    0.5,
                );
            }
        }
        field.cursor = (field.cursor + refresh) % n;
        field.initialized = true;

        let mut attempted = 0.0f32;
        let mut open = 0.0f32;
        let mut open_shift = Vector2::ZERO;
        for (idx, (offset, weight)) in offsets.iter().enumerate() {
            if !field.probe_seen[idx] {
                continue;
            }
            attempted += weight;
            if field.probe_open[idx] > 0.5 {
                open += weight;
                open_shift += *offset * *weight;
            }
        }
        let instant = if attempted > 0.0 {
            open / attempted
        } else {
            0.0
        };
        field.smoothed_openness = smooth_openness(field.smoothed_openness, instant);
        let shift = if open > 0.0 {
            open_shift / open
        } else {
            Vector2::ZERO
        };
        (field.smoothed_openness, shift)
    }

    fn occlusion_openness_3d(
        &mut self,
        field: &mut PropagationField,
        listener_pos: Vector3,
        source_pos: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> (f32, Vector3) {
        let zero = Vector3::new(0.0, 0.0, 0.0);
        let to_source = source_pos - listener_pos;
        let distance = to_source.length();
        if distance <= 0.0001 {
            return (0.0, zero);
        }
        let dir = to_source * distance.recip();
        let mut tangent = dir.cross(Vector3::new(0.0, 1.0, 0.0));
        if tangent.length_squared() <= 0.0001 {
            tangent = dir.cross(Vector3::new(1.0, 0.0, 0.0));
        }
        if tangent.length_squared() <= 0.0001 {
            return (0.0, zero);
        }
        let tangent = tangent.normalized();
        let bitangent = dir.cross(tangent).normalized();
        let offsets = [
            (
                tangent * AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (
                tangent * -AUDIO_DIFFUSION_SPREAD_NEAR,
                AUDIO_DIFFUSION_NEAR_WEIGHT,
            ),
            (tangent * AUDIO_DIFFUSION_SPREAD, AUDIO_DIFFUSION_FAR_WEIGHT),
            (
                tangent * -AUDIO_DIFFUSION_SPREAD,
                AUDIO_DIFFUSION_FAR_WEIGHT,
            ),
            (
                bitangent * AUDIO_DIFFUSION_SPREAD,
                AUDIO_DIFFUSION_FAR_WEIGHT,
            ),
            (
                bitangent * -AUDIO_DIFFUSION_SPREAD,
                AUDIO_DIFFUSION_FAR_WEIGHT,
            ),
        ];
        let n = offsets.len();
        let refresh = if field.initialized { PROBE_SLICE } else { n };
        for step in 0..refresh {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                break;
            }
            let idx = (field.cursor + step) % n;
            let (offset, _weight) = offsets[idx];
            let probe = source_pos + offset;
            let probe_delta = probe - listener_pos;
            let probe_dist = probe_delta.length();
            if probe_dist <= 0.0001 {
                continue;
            }
            self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
            let blocked_physics = self
                .prepared_audio_raycast_3d(
                    listener_pos,
                    probe_delta * probe_dist.recip(),
                    probe_dist,
                    false,
                )
                .is_some_and(|hit| {
                    Some(hit.node) != attached_node && hit.distance > LISTENER_EMBED_EPSILON
                });
            let blocked = blocked_physics
                || (self.audio.has_audio_mask_3d
                    && self
                        .first_audio_mask_3d(listener_pos, probe, audio_layer)
                        .is_some());
            field.probe_open[idx] = if blocked { 0.0 } else { 1.0 };
            field.probe_seen[idx] = true;
            if !blocked {
                self.queue_audio_debug_ray_3d(
                    listener_pos,
                    probe,
                    audio_debug_color(AUDIO_DEBUG_DIRECT, 0.5),
                    0.5,
                );
            }
        }
        field.cursor = (field.cursor + refresh) % n;
        field.initialized = true;

        let mut attempted = 0.0f32;
        let mut open = 0.0f32;
        let mut open_shift = zero;
        for (idx, (offset, weight)) in offsets.iter().enumerate() {
            if !field.probe_seen[idx] {
                continue;
            }
            attempted += weight;
            if field.probe_open[idx] > 0.5 {
                open += weight;
                open_shift += *offset * *weight;
            }
        }
        let instant = if attempted > 0.0 {
            open / attempted
        } else {
            0.0
        };
        field.smoothed_openness = smooth_openness(field.smoothed_openness, instant);
        let shift = if open > 0.0 { open_shift / open } else { zero };
        (field.smoothed_openness, shift)
    }

    // Verify a cached aperture cheaply: two raycasts (listener->aperture,
    // aperture->source) must both be unobstructed. Returns updated total path
    // distance on success. Used before falling back to the full fan re-search.
    fn verify_aperture_2d(
        &mut self,
        listener_pos: Vector2,
        source_pos: Vector2,
        aperture: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let leg_a =
            self.reconcile_segment_clear_2d(listener_pos, aperture, audio_layer, attached_node)?;
        let leg_b =
            self.reconcile_segment_clear_2d(aperture, source_pos, audio_layer, attached_node)?;
        Some(leg_a + leg_b)
    }

    fn verify_aperture_3d(
        &mut self,
        listener_pos: Vector3,
        source_pos: Vector3,
        aperture: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let leg_a =
            self.reconcile_segment_clear_3d(listener_pos, aperture, audio_layer, attached_node)?;
        let leg_b =
            self.reconcile_segment_clear_3d(aperture, source_pos, audio_layer, attached_node)?;
        Some(leg_a + leg_b)
    }

    // Returns segment length if the straight path a->b is unobstructed (no
    // physics body and no audio mask between). Counts one raycast.
    fn reconcile_segment_clear_2d(
        &mut self,
        a: Vector2,
        b: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let delta = b - a;
        let dist = delta.length();
        if dist <= 0.0001 {
            return Some(0.0);
        }
        self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
        let dir = delta * dist.recip();
        // Small skin at each end skips the surface a bounce point sits ON while
        // still catching any wall strictly BETWEEN the two points (so two
        // points straddling a thin wall do NOT falsely reconcile).
        let skin = AUDIO_PORTAL_EPSILON.min(dist * 0.25);
        let start = a + dir * skin;
        let seg = (dist - skin * 2.0).max(0.0);
        let blocked_physics = self
            .prepared_audio_raycast_2d(
                start,
                dir,
                seg,
                &PhysicsQueryFilter {
                    layers: audio_layer,
                    include_areas: false,
                    exclude_nodes: attached_node.into_iter().collect(),
                    ..PhysicsQueryFilter::default()
                },
            )
            .is_some_and(|hit| hit.distance > LISTENER_EMBED_EPSILON);
        if blocked_physics {
            return None;
        }
        if self.audio.has_audio_mask_2d
            && self
                .first_audio_mask_2d(start, b - dir * skin, audio_layer)
                .is_some()
        {
            return None;
        }
        Some(dist)
    }

    fn reconcile_segment_clear_3d(
        &mut self,
        a: Vector3,
        b: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
    ) -> Option<f32> {
        let delta = b - a;
        let dist = delta.length();
        if dist <= 0.0001 {
            return Some(0.0);
        }
        self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
        let dir = delta * dist.recip();
        let skin = AUDIO_PORTAL_EPSILON.min(dist * 0.25);
        let start = a + dir * skin;
        let seg = (dist - skin * 2.0).max(0.0);
        let blocked_physics = self
            .prepared_audio_raycast_3d(start, dir, seg, false)
            .is_some_and(|hit| {
                Some(hit.node) != attached_node && hit.distance > LISTENER_EMBED_EPSILON
            });
        if blocked_physics {
            return None;
        }
        if self.audio.has_audio_mask_3d
            && self
                .first_audio_mask_3d(start, b - dir * skin, audio_layer)
                .is_some()
        {
            return None;
        }
        Some(dist)
    }

    // Bidirectional reconciler (Phase 1). Casts a fan from the listener and a
    // fan from the source, records each ray's polyline points, then finds the
    // listener-point / source-point pair that reconciles (within epsilon or via
    // a short verification ray). The matched midpoint is a virtual source
    // (aperture). Returns (aperture, total path distance, loss).
    pub(super) fn reconcile_aperture_2d(
        &mut self,
        listener_pos: Vector2,
        source_pos: Vector2,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
        range: f32,
    ) -> Option<(Vector2, f32, f32)> {
        let mut listener_pts = std::mem::take(&mut self.audio.scratch_reconcile_listener_2d);
        let mut source_pts = std::mem::take(&mut self.audio.scratch_reconcile_source_2d);
        listener_pts.clear();
        source_pts.clear();
        self.collect_reconcile_points_2d(
            listener_pos,
            source_pos,
            audio_layer,
            range,
            &mut listener_pts,
        );
        self.collect_reconcile_points_2d(
            source_pos,
            listener_pos,
            audio_layer,
            range,
            &mut source_pts,
        );

        let mut best: Option<(Vector2, f32, f32)> = None;
        for lp in listener_pts.iter().copied() {
            for sp in source_pts.iter().copied() {
                let gap = lp.point.distance_to(sp.point);
                if gap > RECONCILE_VERIFY_MAX {
                    continue;
                }
                // Always verify the connecting segment is unobstructed, even for
                // sub-epsilon gaps: two points can sit within epsilon on
                // opposite sides of a thin wall and must NOT reconcile.
                let (aperture, verify_dist) = match self.reconcile_segment_clear_2d(
                    lp.point,
                    sp.point,
                    audio_layer,
                    attached_node,
                ) {
                    Some(d) => ((lp.point + sp.point) * 0.5, d),
                    None => continue,
                };
                let total = lp.traveled + verify_dist + sp.traveled;
                if total > range {
                    continue;
                }
                // Effective loss: higher is better. Give sub-epsilon coincident
                // matches a small edge over verified-across-a-gap matches.
                let tight = if gap <= RECONCILE_EPSILON { 1.05 } else { 1.0 };
                let loss = (lp.loss * sp.loss * tight).min(1.0);
                let score = loss / (1.0 + total);
                if best
                    .as_ref()
                    .is_none_or(|&(_, bt, bl)| score > bl / (1.0 + bt))
                {
                    best = Some((aperture, total, loss));
                }
            }
        }
        listener_pts.clear();
        source_pts.clear();
        self.audio.scratch_reconcile_listener_2d = listener_pts;
        self.audio.scratch_reconcile_source_2d = source_pts;
        best
    }

    pub(super) fn reconcile_aperture_3d(
        &mut self,
        listener_pos: Vector3,
        source_pos: Vector3,
        audio_layer: BitMask,
        attached_node: Option<NodeID>,
        range: f32,
    ) -> Option<(Vector3, f32, f32)> {
        let mut listener_pts = std::mem::take(&mut self.audio.scratch_reconcile_listener_3d);
        let mut source_pts = std::mem::take(&mut self.audio.scratch_reconcile_source_3d);
        listener_pts.clear();
        source_pts.clear();
        self.collect_reconcile_points_3d(
            listener_pos,
            source_pos,
            audio_layer,
            range,
            &mut listener_pts,
        );
        self.collect_reconcile_points_3d(
            source_pos,
            listener_pos,
            audio_layer,
            range,
            &mut source_pts,
        );

        let mut best: Option<(Vector3, f32, f32)> = None;
        for lp in listener_pts.iter().copied() {
            for sp in source_pts.iter().copied() {
                let gap = lp.point.distance_to(sp.point);
                if gap > RECONCILE_VERIFY_MAX {
                    continue;
                }
                let (aperture, verify_dist) = match self.reconcile_segment_clear_3d(
                    lp.point,
                    sp.point,
                    audio_layer,
                    attached_node,
                ) {
                    Some(d) => ((lp.point + sp.point) * 0.5, d),
                    None => continue,
                };
                let total = lp.traveled + verify_dist + sp.traveled;
                if total > range {
                    continue;
                }
                let tight = if gap <= RECONCILE_EPSILON { 1.05 } else { 1.0 };
                let loss = (lp.loss * sp.loss * tight).min(1.0);
                let score = loss / (1.0 + total);
                if best
                    .as_ref()
                    .is_none_or(|&(_, bt, bl)| score > bl / (1.0 + bt))
                {
                    best = Some((aperture, total, loss));
                }
            }
        }
        listener_pts.clear();
        source_pts.clear();
        self.audio.scratch_reconcile_listener_3d = listener_pts;
        self.audio.scratch_reconcile_source_3d = source_pts;
        best
    }

    // Cast a fan from `origin` (aimed generally toward `target`) and record
    // each ray's bounce points plus interior free-segment samples with their
    // accumulated traveled-distance and loss. Budget-guarded by rays_per_tick.
    fn collect_reconcile_points_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint2D>,
    ) {
        for i in 0..RECONCILE_FAN_2D {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                break;
            }
            let angle = i as f32 * TAU / RECONCILE_FAN_2D as f32;
            let dir = Vector2::new(angle.cos(), angle.sin());
            self.march_reconcile_ray_2d(origin, target, dir, audio_layer, range, out);
        }
    }

    fn collect_reconcile_points_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint3D>,
    ) {
        for i in 0..RECONCILE_FAN_3D {
            if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                break;
            }
            let n = RECONCILE_FAN_3D as f32;
            let y = 1.0 - (i as f32 + 0.5) * 2.0 / n;
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let theta = i as f32 * 2.399_963_1;
            let dir = Vector3::new(theta.cos() * radius, y, theta.sin() * radius);
            self.march_reconcile_ray_3d(origin, target, dir, audio_layer, range, out);
        }
    }

    fn march_reconcile_ray_2d(
        &mut self,
        origin: Vector2,
        target: Vector2,
        initial_direction: Vector2,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint2D>,
    ) {
        let mut pos = origin;
        let mut direction = initial_direction;
        if direction.length_squared() <= 0.0001 {
            return;
        }
        direction = direction.normalized();
        let mut traveled = 0.0f32;
        let mut loss = 1.0f32;
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
            let hit = self.nearest_audio_bounce_hit_2d(pos, direction, remaining, audio_layer);
            let seg_len = hit.as_ref().map(|h| h.distance).unwrap_or(remaining);
            // Sample points along the free segment at fixed spacing so density
            // stays high near apertures on long rays.
            let samples =
                ((seg_len / RECONCILE_SAMPLE_SPACING) as usize).min(RECONCILE_MAX_SAMPLES);
            for s in 1..=samples {
                let d = s as f32 * RECONCILE_SAMPLE_SPACING;
                if d >= seg_len {
                    break;
                }
                out.push(ReconcilePoint2D {
                    point: pos + direction * d,
                    traveled: traveled + d,
                    loss,
                });
            }
            let Some(hit) = hit else {
                // Free ray end: record its far endpoint.
                out.push(ReconcilePoint2D {
                    point: pos + direction * seg_len,
                    traveled: traveled + seg_len,
                    loss,
                });
                break;
            };
            traveled += hit.distance;
            loss *= hit.volume_loss.clamp(0.0, 1.0);
            out.push(ReconcilePoint2D {
                point: hit.point,
                traveled,
                loss,
            });
            let reflect_energy = hit.reflection.clamp(0.0, 1.0);
            if reflect_energy < self.audio.config.energy_cutoff
                || loss < self.audio.config.energy_cutoff
            {
                break;
            }
            let Some(reflected) = reflect_2d(direction, hit.normal) else {
                break;
            };
            loss *= reflect_energy;
            self.queue_audio_debug_ray_2d(
                pos,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, loss),
                loss,
            );
            pos = hit.point + reflected * AUDIO_PORTAL_EPSILON;
            direction = reflected;
        }
        let _ = target;
    }

    fn march_reconcile_ray_3d(
        &mut self,
        origin: Vector3,
        target: Vector3,
        initial_direction: Vector3,
        audio_layer: BitMask,
        range: f32,
        out: &mut Vec<ReconcilePoint3D>,
    ) {
        let mut pos = origin;
        let mut direction = initial_direction;
        if direction.length_squared() <= 0.0001 {
            return;
        }
        direction = direction.normalized();
        let mut traveled = 0.0f32;
        let mut loss = 1.0f32;
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
            let hit = self.nearest_audio_bounce_hit_3d(pos, direction, remaining, audio_layer);
            let seg_len = hit.as_ref().map(|h| h.distance).unwrap_or(remaining);
            let samples =
                ((seg_len / RECONCILE_SAMPLE_SPACING) as usize).min(RECONCILE_MAX_SAMPLES);
            for s in 1..=samples {
                let d = s as f32 * RECONCILE_SAMPLE_SPACING;
                if d >= seg_len {
                    break;
                }
                out.push(ReconcilePoint3D {
                    point: pos + direction * d,
                    traveled: traveled + d,
                    loss,
                });
            }
            let Some(hit) = hit else {
                out.push(ReconcilePoint3D {
                    point: pos + direction * seg_len,
                    traveled: traveled + seg_len,
                    loss,
                });
                break;
            };
            traveled += hit.distance;
            loss *= hit.volume_loss.clamp(0.0, 1.0);
            out.push(ReconcilePoint3D {
                point: hit.point,
                traveled,
                loss,
            });
            let reflect_energy = hit.reflection.clamp(0.0, 1.0);
            if reflect_energy < self.audio.config.energy_cutoff
                || loss < self.audio.config.energy_cutoff
            {
                break;
            }
            let Some(reflected) = reflect_3d(direction, hit.normal) else {
                break;
            };
            loss *= reflect_energy;
            self.queue_audio_debug_ray_3d(
                pos,
                hit.point,
                audio_debug_color(AUDIO_DEBUG_BOUNCE, loss),
                loss,
            );
            pos = hit.point + reflected * AUDIO_PORTAL_EPSILON;
            direction = reflected;
        }
        let _ = target;
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

    fn queue_audio_debug_absorption_3d(&mut self, point: Vector3, normal: Vector3, energy: f32) {
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

    pub(super) fn trace_audio_bounce_path_2d(
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
    fn trace_audio_bounce_ray_2d(
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

    pub(super) fn trace_audio_bounce_path_3d(
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
    fn trace_audio_bounce_ray_3d(
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

    fn nearest_audio_bounce_hit_2d(
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

    fn nearest_audio_bounce_hit_3d(
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

    fn physics_bounce_hit_2d(
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

    fn physics_bounce_hit_3d(
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

    fn material_bounce_hit_2d(&self, hit: AudioHit2D) -> AudioBounceHit2D {
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

    fn material_bounce_hit_3d(&self, hit: AudioHit3D) -> AudioBounceHit3D {
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
        // Bodies default to Some(AudioInteraction::new()) at construction, so
        // colliders occlude out of the box; `audio_interaction = none` is the
        // documented opt-out and must stay silent here.
        match data {
            SceneNodeData::StaticBody2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::StaticBody3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::RigidBody2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::RigidBody3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::CharacterBody2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::CharacterBody3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::Area2D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::Area3D(v) => v.audio_interaction.map(|audio| audio.material),
            SceneNodeData::AudioMask2D(v) if v.active => Some(v.material),
            SceneNodeData::AudioMask3D(v) if v.active => Some(v.material),
            _ => Some(AudioMaterial::default()),
        }
    }

    pub(super) fn audio_diffusion_for_node(&self, node: NodeID) -> AudioDiffusion {
        let Some(data) = self.nodes.get(node).map(|n| &n.data) else {
            return AudioDiffusion::default();
        };
        match data {
            SceneNodeData::StaticBody2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::StaticBody3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::RigidBody2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::RigidBody3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::CharacterBody2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::CharacterBody3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::Area2D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
            SceneNodeData::Area3D(v) => v
                .audio_interaction
                .map(|audio| audio.diffusion)
                .unwrap_or_default(),
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

    pub(super) fn first_audio_mask_2d(
        &mut self,
        from: Vector2,
        to: Vector2,
        audio_layer: BitMask,
    ) -> Option<AudioHit2D> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioHit2D> = None;
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioMask2D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
        for index in 0..self.audio.scratch_ids.len() {
            let mask_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioMask2D(mask)) = self.nodes.get(mask_id).map(|n| &n.data)
            else {
                continue;
            };
            if !mask.active || mask.material.audio_mask.intersects(audio_layer) {
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
        audio_layer: BitMask,
    ) -> Option<AudioHit3D> {
        let dir = to - from;
        let len = dir.length();
        if len <= 0.0001 {
            return None;
        }
        let mut best: Option<AudioHit3D> = None;
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioMask3D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
        for index in 0..self.audio.scratch_ids.len() {
            let mask_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioMask3D(mask)) = self.nodes.get(mask_id).map(|n| &n.data)
            else {
                continue;
            };
            if !mask.active || mask.material.audio_mask.intersects(audio_layer) {
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
                let Some((center, half)) = self.audio_effect_zone_shape_3d(child) else {
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
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioPortal2D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
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
            if let Some(node) = self.nodes.get(portal_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
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
        let mut scratch_ids = std::mem::take(&mut self.audio.scratch_ids);
        scratch_ids.clear();
        crate::runtime::scan_node_type_slots(
            &self.nodes,
            perro_nodes::NodeType::AudioPortal3D,
            |_| true,
            &mut scratch_ids,
        );
        self.audio.scratch_ids = scratch_ids;
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
            if let Some(node) = self.nodes.get(portal_id) {
                self.audio
                    .scratch_child_ids
                    .extend_from_slice(node.children_slice());
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

// Hysteresis blend for stored openness: rise fast on opening, fall slow on
// closing, so a probe flipping blocked/unblocked on alternate ticks does not
// oscillate the perceived level.
#[inline]
fn smooth_openness(prev: f32, next: f32) -> f32 {
    let rate = if next > prev {
        OPENNESS_RISE
    } else {
        OPENNESS_FALL
    };
    prev + (next - prev) * rate
}

fn listener_effect_mix(
    options: perro_structs::AudioListenerOptions,
    audio_layer: BitMask,
) -> AudioEffectZoneMix {
    if options.audio_mask.intersects(audio_layer) {
        return AudioEffectZoneMix::default();
    }
    let mut mix = AudioEffectZoneMix::default();
    for effect in options.effects {
        mix.apply(effect);
    }
    mix
}
