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
    }

    pub(super) fn refresh_audio_scene_flags(&mut self) {
        let node_count = self.nodes.len();
        if self.audio.audio_scene_flags_node_count == node_count {
            return;
        }
        self.audio.audio_scene_flags_node_count = node_count;
        self.audio.has_audio_mask_2d = false;
        self.audio.has_audio_portal_2d = false;
        self.audio.has_audio_portal_3d = false;
        self.audio.has_audio_zone_2d = false;
        self.audio.has_audio_zone_3d = false;
        for (_, node) in self.nodes.iter() {
            match &node.data {
                SceneNodeData::AudioMask2D(_) => {
                    self.audio.has_audio_mask_2d = true;
                }
                SceneNodeData::AudioPortal2D(_) => {
                    self.audio.has_audio_portal_2d = true;
                }
                SceneNodeData::AudioPortal3D(_) => {
                    self.audio.has_audio_portal_3d = true;
                }
                SceneNodeData::AudioZone2D(_) => {
                    self.audio.has_audio_zone_2d = true;
                }
                SceneNodeData::AudioZone3D(_) => {
                    self.audio.has_audio_zone_3d = true;
                }
                _ => {}
            }
            if self.audio.has_audio_mask_2d
                && self.audio.has_audio_portal_2d
                && self.audio.has_audio_portal_3d
                && self.audio.has_audio_zone_2d
                && self.audio.has_audio_zone_3d
            {
                break;
            }
        }
    }

    pub(super) fn solve_listener_field_2d(
        &mut self,
        sounds: &mut [ActiveSpatialSound],
        dt: f32,
        tick: f32,
    ) {
        let listener = self
            .resource_api
            .audio_listener_2d
            .lock()
            .ok()
            .and_then(|guard| *guard)
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
                mask: u32::MAX,
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
            if let Some(result) = self.solve_2d(pos, sound, hit) {
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
            if let Some(result) = self.solve_3d(pos, sound, hit) {
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
                    eq: perro_runtime_context::sub_apis::AudioEq {
                        low_gain: request.effects.eq.low_gain,
                        mid_gain: request.effects.eq.mid_gain,
                        high_gain: request.effects.eq.high_gain,
                    },
                    compression: perro_runtime_context::sub_apis::AudioCompression {
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
                bus_id: request.bus_id,
                occlusion_mask: u32::MAX,
                enable_propagation: true,
            };
            match request.pos {
                QueuedSpatialAudioPos::TwoD(position) => {
                    self.play_runtime_audio_2d(audio, position, options);
                }
                QueuedSpatialAudioPos::ThreeD(position) => {
                    self.play_runtime_audio_3d(audio, position, options);
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
                bus_id: match &request.kind {
                    crate::rs_ctx::QueuedSpatialMidiKind::Note { options, .. } => options.bus_id,
                    crate::rs_ctx::QueuedSpatialMidiKind::File { song, .. } => song.bus_id,
                },
                occlusion_mask: u32::MAX,
                enable_propagation: true,
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
