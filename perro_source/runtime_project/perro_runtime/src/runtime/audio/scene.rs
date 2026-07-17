use super::*;

impl Runtime {
    pub(super) fn finish_audio_sound_updates(
        &mut self,
        sounds: Vec<ActiveSpatialSound>,
        start: Instant,
    ) {
        self.audio.counters.active_positional = sounds.len() as u32;
        self.audio.counters.propagation_time = start.elapsed();
        self.audio.sounds = sounds;
        self.clear_stale_audio_debug_rays();
    }

    pub(super) fn queue_audio_debug_ray_2d(
        &mut self,
        from: Vector2,
        to: Vector2,
        color: [f32; 4],
        energy: f32,
    ) {
        if !self.audio.config.debug_rays {
            return;
        }
        self.queue_render_command(RenderCommand::TwoD(Command2D::DrawShape {
            draw: DrawShape2DCommand {
                shape: DrawShape2D::line(to - from, color.into(), audio_debug_ray_width(energy)),
                position: [from.x, from.y],
            },
        }));
    }

    pub(super) fn queue_audio_debug_ray_3d(
        &mut self,
        from: Vector3,
        to: Vector3,
        color: [f32; 4],
        energy: f32,
    ) {
        if !self.audio.config.debug_rays {
            return;
        }
        let index = self.audio.debug_ray_count_3d;
        self.audio.debug_ray_count_3d = self.audio.debug_ray_count_3d.saturating_add(1);
        self.queue_render_command(RenderCommand::ThreeD(Box::new(
            Command3D::DrawDebugLine3D {
                node: audio_debug_ray_node(index),
                start: [from.x, from.y, from.z],
                end: [to.x, to.y, to.z],
                thickness: audio_debug_ray_thickness(energy),
                color,
            },
        )));
    }

    pub(super) fn queue_audio_debug_point_3d(
        &mut self,
        position: Vector3,
        color: [f32; 4],
        energy: f32,
    ) {
        if !self.audio.config.debug_rays {
            return;
        }
        let index = self.audio.debug_ray_count_3d;
        self.audio.debug_ray_count_3d = self.audio.debug_ray_count_3d.saturating_add(1);
        self.queue_render_command(RenderCommand::ThreeD(Box::new(
            Command3D::DrawDebugPoint3D {
                node: audio_debug_ray_node(index),
                position: [position.x, position.y, position.z],
                size: audio_debug_dot_size(energy),
                color,
            },
        )));
    }

    pub(super) fn clear_stale_audio_debug_rays(&mut self) {
        let start = self.audio.debug_ray_count_3d;
        let end = self.audio.prev_debug_ray_count_3d;
        for index in start..end {
            self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
                node: audio_debug_ray_node(index),
            })));
        }
        self.audio.prev_debug_ray_count_3d = self.audio.debug_ray_count_3d;
    }

    pub(crate) fn remove_attached_audio_for_node(&mut self, node: NodeID) {
        let mut removed = false;
        let mut i = 0usize;
        while i < self.audio.sounds.len() {
            let attached =
                matches!(self.audio.sounds[i].pos, SpatialSoundPos::Attached(id) if id == node);
            if attached {
                if let Some(id) = self.audio.sounds[i].playback_id
                    && let Ok(guard) = self.resource_api.bark.lock()
                    && let Some(player) = guard.as_ref()
                {
                    let _ = player.stop_playback(id);
                }
                self.audio.sounds.remove(i);
                removed = true;
            } else {
                i += 1;
            }
        }
        if removed && self.audio.config.debug_rays {
            self.audio.debug_ray_count_3d = 0;
            self.clear_stale_audio_debug_rays();
        }
    }

    pub(super) fn refresh_audio_scene_flags(&mut self) {
        // Gate on structural revision: bumps only on insert / remove / clear /
        // reparent, so add+remove pairs that leave the count unchanged still
        // trigger a rescan, while per-tick data mutations do not.
        let structural_revision = self.nodes.structural_revision();
        if self.audio.audio_scene_flags_structural_revision == structural_revision {
            return;
        }
        self.audio.audio_scene_flags_structural_revision = structural_revision;
        self.audio.has_audio_mask_2d = false;
        self.audio.has_audio_mask_3d = false;
        self.audio.has_audio_portal_2d = false;
        self.audio.has_audio_portal_3d = false;
        self.audio.has_audio_effect_zone_2d = false;
        self.audio.has_audio_effect_zone_3d = false;
        self.audio.audio_mask_ids_2d.clear();
        self.audio.audio_mask_ids_3d.clear();
        self.audio.audio_portal_ids_2d.clear();
        self.audio.audio_portal_ids_3d.clear();
        self.audio.audio_effect_zone_ids_2d.clear();
        self.audio.audio_effect_zone_ids_3d.clear();
        // Single pass fills both the has_* fast-gate flags and the per-type id
        // lists the ray/zone helpers iterate; no early break since the lists
        // must be complete.
        for (id, node) in self.nodes.iter() {
            match &node.data {
                SceneNodeData::AudioMask2D(_) => {
                    self.audio.has_audio_mask_2d = true;
                    self.audio.audio_mask_ids_2d.push(id);
                }
                SceneNodeData::AudioMask3D(_) => {
                    self.audio.has_audio_mask_3d = true;
                    self.audio.audio_mask_ids_3d.push(id);
                }
                SceneNodeData::AudioPortal2D(_) => {
                    self.audio.has_audio_portal_2d = true;
                    self.audio.audio_portal_ids_2d.push(id);
                }
                SceneNodeData::AudioPortal3D(_) => {
                    self.audio.has_audio_portal_3d = true;
                    self.audio.audio_portal_ids_3d.push(id);
                }
                SceneNodeData::AudioEffectZone2D(_) => {
                    self.audio.has_audio_effect_zone_2d = true;
                    self.audio.audio_effect_zone_ids_2d.push(id);
                }
                SceneNodeData::AudioEffectZone3D(_) => {
                    self.audio.has_audio_effect_zone_3d = true;
                    self.audio.audio_effect_zone_ids_3d.push(id);
                }
                _ => {}
            }
        }
    }

    pub(super) fn solve_listener_field_2d(
        &mut self,
        sounds: &mut [ActiveSpatialSound],
        dt: f32,
        tick: f32,
    ) {
        let (listener, listener_options) = self
            .resource_api
            .audio_listener_2d
            .lock()
            .ok()
            .map(|slot| (slot.listener.unwrap_or_default(), slot.options.clone()))
            .unwrap_or_default();
        let listener_pos = Vector2::new(listener.position[0], listener.position[1]);
        self.audio.scratch_ray_inputs.clear();
        self.audio.scratch_ray_outputs.clear();
        for i in 0..LISTENER_FIELD_RAYS_2D {
            let angle = i as f32 * TAU / LISTENER_FIELD_RAYS_2D as f32;
            self.audio.scratch_ray_inputs.push(AudioRaycastInput::TwoD {
                origin: listener_pos,
                direction: Vector2::new(angle.cos(), angle.sin()),
                max_distance: self.audio.config.max_ray_distance_2d,
                layers: BitMask::ALL,
            });
        }
        self.prepare_audio_raycast_2d();
        let mut ray_inputs = std::mem::take(&mut self.audio.scratch_ray_inputs);
        let mut ray_outputs = std::mem::take(&mut self.audio.scratch_ray_outputs);
        ray_outputs.resize(ray_inputs.len(), AudioRaycastResult::None);
        self.cast_prepared_audio_rays(&ray_inputs, &mut ray_outputs, false);
        self.audio.counters.raycasts = self
            .audio
            .counters
            .raycasts
            .saturating_add(ray_inputs.len() as u32);

        for sound in sounds {
            sound.elapsed_since_prop += dt;
            if sound.elapsed_since_prop < tick {
                self.audio.counters.cache_hits = self.audio.counters.cache_hits.saturating_add(1);
                continue;
            }
            sound.elapsed_since_prop = 0.0;
            self.refresh_spatial_position(sound);
            let Some(pos) = sound.last_2d else {
                continue;
            };
            let direction = listener_pos.direction_to(pos);
            let angle = direction.y.atan2(direction.x).rem_euclid(TAU);
            let ray_index = ((angle / TAU * LISTENER_FIELD_RAYS_2D as f32).round() as usize)
                % LISTENER_FIELD_RAYS_2D;
            let distance = listener_pos.distance_to(pos);
            let hit = match ray_outputs.get(ray_index).copied().unwrap_or_default() {
                AudioRaycastResult::TwoD(Some(hit)) if hit.distance <= distance + 0.25 => Some(hit),
                _ => None,
            };
            if let Some(result) = self.solve_2d(pos, sound, hit, listener, listener_options.clone())
            {
                self.apply_spatial_result(sound, result);
            }
        }

        ray_inputs.clear();
        ray_outputs.clear();
        self.audio.scratch_ray_inputs = ray_inputs;
        self.audio.scratch_ray_outputs = ray_outputs;
    }

    pub(super) fn solve_listener_field_3d(
        &mut self,
        sounds: &mut [ActiveSpatialSound],
        dt: f32,
        tick: f32,
    ) {
        let (listener, listener_options) = self
            .resource_api
            .audio_listener_3d
            .lock()
            .ok()
            .map(|slot| (slot.listener.unwrap_or_default(), slot.options.clone()))
            .unwrap_or_default();
        let listener_pos = Vector3::new(
            listener.position[0],
            listener.position[1],
            listener.position[2],
        );
        self.audio.scratch_ray_inputs.clear();
        self.audio.scratch_ray_outputs.clear();
        self.audio.scratch_field_dirs_3d.clear();
        for i in 0..LISTENER_FIELD_RAYS_3D {
            let n = LISTENER_FIELD_RAYS_3D as f32;
            let y = 1.0 - (i as f32 + 0.5) * 2.0 / n;
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let theta = i as f32 * 2.399_963_1;
            let dir = Vector3::new(theta.cos() * radius, y, theta.sin() * radius);
            self.audio.scratch_field_dirs_3d.push(dir);
            self.audio
                .scratch_ray_inputs
                .push(AudioRaycastInput::ThreeD {
                    origin: listener_pos,
                    direction: dir,
                    max_distance: self.audio.config.max_ray_distance_3d,
                    include_areas: false,
                });
        }
        self.prepare_audio_raycast_3d();
        let mut ray_inputs = std::mem::take(&mut self.audio.scratch_ray_inputs);
        let mut ray_outputs = std::mem::take(&mut self.audio.scratch_ray_outputs);
        let mut ray_dirs = std::mem::take(&mut self.audio.scratch_field_dirs_3d);
        ray_outputs.resize(ray_inputs.len(), AudioRaycastResult::None);
        self.cast_prepared_audio_rays(&ray_inputs, &mut ray_outputs, false);
        self.audio.counters.raycasts = self
            .audio
            .counters
            .raycasts
            .saturating_add(ray_inputs.len() as u32);

        for sound in sounds {
            sound.elapsed_since_prop += dt;
            if sound.elapsed_since_prop < tick {
                self.audio.counters.cache_hits = self.audio.counters.cache_hits.saturating_add(1);
                continue;
            }
            sound.elapsed_since_prop = 0.0;
            self.refresh_spatial_position(sound);
            let Some(pos) = sound.last_3d else {
                continue;
            };
            let to_sound = pos - listener_pos;
            let distance = to_sound.length();
            if distance <= 0.0001 {
                continue;
            }
            let direction = to_sound * distance.recip();
            let mut best_index = 0usize;
            let mut best_dot = f32::NEG_INFINITY;
            for (index, ray_dir) in ray_dirs.iter().enumerate() {
                let dot =
                    direction.x * ray_dir.x + direction.y * ray_dir.y + direction.z * ray_dir.z;
                if dot > best_dot {
                    best_dot = dot;
                    best_index = index;
                }
            }
            let hit = match ray_outputs.get(best_index).copied().unwrap_or_default() {
                AudioRaycastResult::ThreeD(Some(hit)) if hit.distance <= distance + 0.25 => {
                    Some(hit)
                }
                _ => None,
            };
            if let Some(result) = self.solve_3d(pos, sound, hit, listener, listener_options.clone())
            {
                self.apply_spatial_result(sound, result);
            }
        }

        ray_inputs.clear();
        ray_outputs.clear();
        ray_dirs.clear();
        self.audio.scratch_ray_inputs = ray_inputs;
        self.audio.scratch_ray_outputs = ray_outputs;
        self.audio.scratch_field_dirs_3d = ray_dirs;
    }

    pub(super) fn apply_spatial_result(
        &mut self,
        sound: &mut ActiveSpatialSound,
        result: PropagationResult,
    ) {
        if self.audio.config.debug_rays {
            let _ = (result.perceived_2d, result.perceived_3d);
        }
        // Smooth toward the new solve so 20Hz propagation ticks fade instead
        // of stepping (audible zipper on occlusion transitions).
        let result = match sound.last_result {
            Some(prev) => smooth_propagation_result(prev, result),
            None => result,
        };
        sound.last_result = Some(result);
        let bark_start = Instant::now();
        if let Some(id) = sound.playback_id
            && let Ok(guard) = self.resource_api.bark.lock()
            && let Some(player) = guard.as_ref()
        {
            let _ = player.update_spatial(
                id,
                perro_pawdio::SpatialAudioParams {
                    pan: perro_pawdio::AudioPan::new(result.pan[0], result.pan[1], result.pan[2]),
                    volume: result.volume,
                    low_pass: result.low_pass,
                    reverb_send: result.reverb_send,
                    echo: result.echo,
                    reflection: result.reflection,
                    occlusion: result.occlusion,
                    eq: perro_pawdio::AudioEq {
                        low_gain: sound.effects.eq.low_gain,
                        mid_gain: sound.effects.eq.mid_gain,
                        high_gain: sound.effects.eq.high_gain,
                    },
                    compression: perro_pawdio::AudioCompression {
                        threshold: sound.effects.compression.threshold,
                        ratio: sound.effects.compression.ratio,
                        attack: sound.effects.compression.attack,
                        release: sound.effects.compression.release,
                    },
                },
            );
        }
        self.audio.counters.bark_update_time += bark_start.elapsed();
    }

    pub(super) fn drain_resource_spatial_audio(&mut self) {
        let queued = self
            .resource_api
            .spatial_audio_queue
            .lock()
            .ok()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();
        for request in queued {
            let audio = RuntimeAudio {
                source: request.source.as_str(),
                looped: request.looped,
                volume: request.volume,
                effects: AudioEffects {
                    speed: request.effects.speed,
                    low_pass: request.effects.low_pass,
                    reverb_send: request.effects.reverb_send,
                    echo: request.effects.echo,
                    reflection: request.effects.reflection,
                    occlusion: request.effects.occlusion,
                    eq: perro_runtime_api::sub_apis::AudioEq {
                        low_gain: request.effects.eq.low_gain,
                        mid_gain: request.effects.eq.mid_gain,
                        high_gain: request.effects.eq.high_gain,
                    },
                    compression: perro_runtime_api::sub_apis::AudioCompression {
                        threshold: request.effects.compression.threshold,
                        ratio: request.effects.compression.ratio,
                        attack: request.effects.compression.attack,
                        release: request.effects.compression.release,
                    },
                },
                from_start: request.from_start,
                from_end: request.from_end,
            };
            let options = SpatialAudioOptions {
                range: request.range,
                audio_layer: request.audio_layer,
                enable_propagation: request.enable_propagation,
                direction_2d: match request.direction_2d {
                    perro_resource_api::sub_apis::AudioDirection::Omni => AudioDirection::Omni,
                    perro_resource_api::sub_apis::AudioDirection::Directional(v) => {
                        AudioDirection::Directional(v)
                    }
                    perro_resource_api::sub_apis::AudioDirection::InverseDirectional(v) => {
                        AudioDirection::InverseDirectional(v)
                    }
                    perro_resource_api::sub_apis::AudioDirection::Bidirectional(v) => {
                        AudioDirection::Bidirectional(v)
                    }
                },
                direction_3d: match request.direction_3d {
                    perro_resource_api::sub_apis::AudioDirection::Omni => AudioDirection::Omni,
                    perro_resource_api::sub_apis::AudioDirection::Directional(v) => {
                        AudioDirection::Directional(v)
                    }
                    perro_resource_api::sub_apis::AudioDirection::InverseDirectional(v) => {
                        AudioDirection::InverseDirectional(v)
                    }
                    perro_resource_api::sub_apis::AudioDirection::Bidirectional(v) => {
                        AudioDirection::Bidirectional(v)
                    }
                },
            };
            match request.pos {
                QueuedSpatialAudioPos::TwoD(position) => {
                    self.start_spatial_sound(
                        audio,
                        SpatialSoundPos::TwoD(position),
                        request.bus_id,
                        normalize_spatial_options(options),
                        Some(position),
                        None,
                    );
                }
                QueuedSpatialAudioPos::ThreeD(position) => {
                    self.start_spatial_sound(
                        audio,
                        SpatialSoundPos::ThreeD(position),
                        request.bus_id,
                        normalize_spatial_options(options),
                        None,
                        Some(position),
                    );
                }
            }
        }
        let queued_midi = self
            .resource_api
            .spatial_midi_queue
            .lock()
            .ok()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();
        for request in queued_midi {
            let options = SpatialAudioOptions {
                range: request.range,
                audio_layer: BitMask::ALL,
                enable_propagation: true,
                direction_2d: AudioDirection::Omni,
                direction_3d: AudioDirection::Omni,
            };
            match (request.kind, request.pos) {
                (
                    crate::rs_ctx::QueuedSpatialMidiKind::Note {
                        id,
                        note,
                        options: note_options,
                        held,
                    },
                    QueuedSpatialAudioPos::TwoD(position),
                ) => {
                    self.start_spatial_midi_note(SpatialMidiNoteStart {
                        id,
                        note,
                        options: note_options.as_options(),
                        held,
                        pos: SpatialSoundPos::TwoD(position),
                        spatial: options,
                        last_2d: Some(position),
                        last_3d: None,
                    });
                }
                (
                    crate::rs_ctx::QueuedSpatialMidiKind::Note {
                        id,
                        note,
                        options: note_options,
                        held,
                    },
                    QueuedSpatialAudioPos::ThreeD(position),
                ) => {
                    self.start_spatial_midi_note(SpatialMidiNoteStart {
                        id,
                        note,
                        options: note_options.as_options(),
                        held,
                        pos: SpatialSoundPos::ThreeD(position),
                        spatial: options,
                        last_2d: None,
                        last_3d: Some(position),
                    });
                }
                (
                    crate::rs_ctx::QueuedSpatialMidiKind::File { id, song },
                    QueuedSpatialAudioPos::TwoD(position),
                ) => {
                    self.start_spatial_midi_file(
                        id,
                        song.as_song(),
                        SpatialSoundPos::TwoD(position),
                        options,
                        Some(position),
                        None,
                    );
                }
                (
                    crate::rs_ctx::QueuedSpatialMidiKind::File { id, song },
                    QueuedSpatialAudioPos::ThreeD(position),
                ) => {
                    self.start_spatial_midi_file(
                        id,
                        song.as_song(),
                        SpatialSoundPos::ThreeD(position),
                        options,
                        None,
                        Some(position),
                    );
                }
            }
        }
    }

    pub(super) fn refresh_spatial_position(&mut self, sound: &mut ActiveSpatialSound) {
        match sound.pos {
            SpatialSoundPos::TwoD(position) => {
                sound.last_2d = Some(position);
                sound.last_3d = None;
            }
            SpatialSoundPos::ThreeD(position) => {
                sound.last_3d = Some(position);
                sound.last_2d = None;
            }
            SpatialSoundPos::Attached(node) => {
                let Some(spatial) = self.nodes.get(node).map(|n| n.spatial()) else {
                    return;
                };
                match spatial {
                    perro_nodes::Spatial::TwoD => {
                        if let Some(global) = self.get_global_transform_2d(node) {
                            sound.last_2d = Some(global.position);
                            sound.last_3d = None;
                        }
                    }
                    perro_nodes::Spatial::ThreeD => {
                        if let Some(global) = self.get_global_transform_3d(node) {
                            sound.last_3d = Some(global.position);
                            sound.last_2d = None;
                        }
                    }
                    perro_nodes::Spatial::None => {}
                }
            }
        }
    }
}

const AUDIO_PARAM_SMOOTHING: f32 = 0.5;
// Volume rises faster than it falls: clearing an occluder should read as
// immediate, while onset of occlusion still fades without zipper.
const AUDIO_VOLUME_ATTACK: f32 = 0.8;

#[inline]
fn smooth_toward(prev: f32, next: f32) -> f32 {
    prev + (next - prev) * AUDIO_PARAM_SMOOTHING
}

#[inline]
fn smooth_volume(prev: f32, next: f32) -> f32 {
    let rate = if next > prev {
        AUDIO_VOLUME_ATTACK
    } else {
        AUDIO_PARAM_SMOOTHING
    };
    prev + (next - prev) * rate
}

fn smooth_propagation_result(
    prev: PropagationResult,
    next: PropagationResult,
) -> PropagationResult {
    PropagationResult {
        pan: [
            smooth_toward(prev.pan[0], next.pan[0]),
            smooth_toward(prev.pan[1], next.pan[1]),
            smooth_toward(prev.pan[2], next.pan[2]),
        ],
        volume: smooth_volume(prev.volume, next.volume),
        low_pass: smooth_toward(prev.low_pass, next.low_pass),
        reflection: smooth_toward(prev.reflection, next.reflection),
        reverb_send: smooth_toward(prev.reverb_send, next.reverb_send),
        echo: smooth_toward(prev.echo, next.echo),
        occlusion: smooth_toward(prev.occlusion, next.occlusion),
        perceived_2d: next.perceived_2d,
        perceived_3d: next.perceived_3d,
    }
}

fn audio_debug_ray_node(index: u32) -> NodeID {
    NodeID::from_u64(0xAD00_0000_0000_0000u64 | index.saturating_add(1) as u64)
}

#[inline]
fn audio_debug_ray_thickness(energy: f32) -> f32 {
    0.012 + energy.clamp(0.0, 1.0).sqrt() * 0.06
}

#[inline]
fn audio_debug_ray_width(energy: f32) -> f32 {
    1.0 + energy.clamp(0.0, 1.0).sqrt() * 4.0
}

#[inline]
fn audio_debug_dot_size(energy: f32) -> f32 {
    0.06 + energy.clamp(0.0, 1.0).sqrt() * 0.18
}
