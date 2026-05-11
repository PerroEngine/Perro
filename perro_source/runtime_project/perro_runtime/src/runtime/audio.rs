use super::Runtime;
use super::physics::{AudioRaycastInput, AudioRaycastResult};
use crate::rs_ctx::QueuedSpatialAudioPos;
use perro_ids::NodeID;
use perro_nodes::{
    AudioDiffusion, AudioMaterial, AudioZoneEffect, CollisionShape2D, CollisionShape3D,
    SceneNodeData,
};
use perro_resource_context::sub_apis::AudioAPI;
use perro_runtime_context::sub_apis::{
    AudioEffects, RuntimeAudio, RuntimeAudioAPI, SpatialAudioOptions,
};
use perro_structs::{Vector2, Vector3};
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

const PARALLEL_AUDIO_RAY_THRESHOLD: usize = 128;
const LISTENER_FIELD_SOUND_THRESHOLD_2D: usize = 64;
const LISTENER_FIELD_SOUND_THRESHOLD_3D: usize = 128;
const LISTENER_FIELD_RAYS_2D: usize = 64;
const LISTENER_FIELD_RAYS_3D: usize = 96;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AudioPropagationConfigRt {
    pub listener_max_distance: f32,
    pub propagation_tick_hz: f32,
    pub energy_cutoff: f32,
    pub debug_rays: bool,
    pub max_bounces_2d: u32,
    pub rays_per_tick_2d: u32,
    pub max_ray_distance_2d: f32,
    pub max_bounces_3d: u32,
    pub rays_per_tick_3d: u32,
    pub max_ray_distance_3d: f32,
}

impl Default for AudioPropagationConfigRt {
    fn default() -> Self {
        Self {
            listener_max_distance: 500.0,
            propagation_tick_hz: 20.0,
            energy_cutoff: 0.02,
            debug_rays: false,
            max_bounces_2d: 4,
            rays_per_tick_2d: 64,
            max_ray_distance_2d: 500.0,
            max_bounces_3d: 4,
            rays_per_tick_3d: 128,
            max_ray_distance_3d: 500.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AudioPropagationCounters {
    pub active_positional: u32,
    pub raycasts: u32,
    pub cache_hits: u32,
    pub propagation_time: Duration,
    pub bark_update_time: Duration,
}

#[derive(Clone, Copy, Debug)]
enum SpatialSoundPos {
    TwoD(Vector2),
    ThreeD(Vector3),
    Attached(NodeID),
}

#[derive(Clone, Debug)]
struct ActiveSpatialSound {
    source: String,
    looped: bool,
    volume: f32,
    effects: AudioEffects,
    options: SpatialAudioOptions,
    pos: SpatialSoundPos,
    last_2d: Option<Vector2>,
    last_3d: Option<Vector3>,
    playback_id: Option<u64>,
    elapsed_since_prop: f32,
    remaining: Option<f32>,
    last_result: Option<PropagationResult>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PropagationResult {
    pub pan: [f32; 3],
    pub volume: f32,
    pub low_pass: f32,
    pub reflection: f32,
    pub reverb_send: f32,
    pub echo: f32,
    pub occlusion: f32,
    pub perceived_2d: Option<Vector2>,
    pub perceived_3d: Option<Vector3>,
}

#[derive(Clone, Copy, Debug)]
struct AudioHit2D {
    node: NodeID,
    point: Vector2,
    normal: Vector2,
    distance: f32,
    material: AudioMaterial,
    thickness: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct AudioZoneMix {
    reverb_send: f32,
    echo: f32,
    dampening: f32,
}

impl AudioZoneMix {
    fn add(&mut self, effect: AudioZoneEffect) {
        self.reverb_send = self.reverb_send.max(effect.reverb_send.clamp(0.0, 1.0));
        self.echo = self.echo.max(effect.echo.clamp(0.0, 1.0));
        self.dampening = self.dampening.max(effect.dampening.clamp(0.0, 1.0));
    }
}

pub(crate) struct AudioPropagationState {
    pub config: AudioPropagationConfigRt,
    sounds: Vec<ActiveSpatialSound>,
    scratch_ids: Vec<NodeID>,
    scratch_child_ids: Vec<NodeID>,
    scratch_ray_inputs: Vec<AudioRaycastInput>,
    scratch_ray_indices: Vec<usize>,
    scratch_ray_outputs: Vec<AudioRaycastResult>,
    scratch_sound_ray_results: Vec<AudioRaycastResult>,
    scratch_field_dirs_3d: Vec<Vector3>,
    has_audio_mask_2d: bool,
    has_audio_portal_2d: bool,
    has_audio_zone_2d: bool,
    has_audio_zone_3d: bool,
    audio_scene_flags_node_count: usize,
    pub counters: AudioPropagationCounters,
}

impl AudioPropagationState {
    pub fn new() -> Self {
        Self {
            config: AudioPropagationConfigRt::default(),
            sounds: Vec::new(),
            scratch_ids: Vec::new(),
            scratch_child_ids: Vec::new(),
            scratch_ray_inputs: Vec::new(),
            scratch_ray_indices: Vec::new(),
            scratch_ray_outputs: Vec::new(),
            scratch_sound_ray_results: Vec::new(),
            scratch_field_dirs_3d: Vec::new(),
            has_audio_mask_2d: false,
            has_audio_portal_2d: false,
            has_audio_zone_2d: false,
            has_audio_zone_3d: false,
            audio_scene_flags_node_count: usize::MAX,
            counters: AudioPropagationCounters::default(),
        }
    }
}

impl Default for AudioPropagationState {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub(crate) fn configure_audio_from_project(&mut self) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let cfg = project.config.audio;
        self.audio.config = AudioPropagationConfigRt {
            listener_max_distance: cfg.listener_max_distance,
            propagation_tick_hz: cfg.propagation_tick_hz,
            energy_cutoff: cfg.energy_cutoff,
            debug_rays: cfg.debug_rays,
            max_bounces_2d: cfg.propagation_2d.max_bounces,
            rays_per_tick_2d: cfg.propagation_2d.rays_per_tick,
            max_ray_distance_2d: cfg.propagation_2d.max_ray_distance,
            max_bounces_3d: cfg.propagation_3d.max_bounces,
            rays_per_tick_3d: cfg.propagation_3d.rays_per_tick,
            max_ray_distance_3d: cfg.propagation_3d.max_ray_distance,
        };
    }

    pub(crate) fn update_audio_propagation(&mut self, dt: f32) {
        let start = Instant::now();
        self.audio.counters = AudioPropagationCounters::default();
        self.drain_resource_spatial_audio();
        if self.audio.sounds.is_empty() {
            return;
        }
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        self.refresh_audio_scene_flags();
        let tick = if self.audio.config.propagation_tick_hz <= 0.0 {
            0.0
        } else {
            1.0 / self.audio.config.propagation_tick_hz
        };
        let mut sounds = std::mem::take(&mut self.audio.sounds);
        for sound in &mut sounds {
            if let Some(remaining) = &mut sound.remaining {
                *remaining -= dt.max(0.0);
            }
        }
        sounds.retain(|sound| sound.looped || sound.remaining.is_none_or(|v| v > 0.0));
        let dt = dt.max(0.0);
        self.audio.scratch_ray_inputs.clear();
        self.audio.scratch_ray_indices.clear();
        self.audio.scratch_sound_ray_results.clear();
        self.audio
            .scratch_sound_ray_results
            .resize(sounds.len(), AudioRaycastResult::None);
        let due_2d_count = sounds
            .iter()
            .filter(|sound| sound.elapsed_since_prop + dt >= tick && sound.last_2d.is_some())
            .count();
        let due_3d_count = sounds
            .iter()
            .filter(|sound| sound.elapsed_since_prop + dt >= tick && sound.last_3d.is_some())
            .count();
        let use_field_2d = due_2d_count >= LISTENER_FIELD_SOUND_THRESHOLD_2D && due_3d_count == 0;
        let use_field_3d = due_3d_count >= LISTENER_FIELD_SOUND_THRESHOLD_3D && due_2d_count == 0;
        if use_field_2d {
            self.solve_listener_field_2d(&mut sounds, dt, tick);
        } else if use_field_3d {
            self.solve_listener_field_3d(&mut sounds, dt, tick);
        }
        if use_field_2d || use_field_3d {
            self.finish_audio_sound_updates(sounds, start);
            return;
        }
        for (index, sound) in sounds.iter_mut().enumerate() {
            sound.elapsed_since_prop += dt;
            if sound.elapsed_since_prop < tick {
                continue;
            }
            self.refresh_spatial_position(sound);
            if let Some(pos) = sound.last_2d {
                if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_2d {
                    continue;
                }
                let listener = self
                    .resource_api
                    .audio_listener_2d
                    .lock()
                    .ok()
                    .and_then(|guard| *guard)
                    .unwrap_or_default();
                let listener_pos = Vector2::new(listener.position[0], listener.position[1]);
                let distance = listener_pos.distance_to(pos);
                let direction = listener_pos.direction_to(pos);
                self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
                if sound.options.enable_propagation {
                    self.audio.scratch_ray_indices.push(index);
                    self.audio.scratch_ray_inputs.push(AudioRaycastInput::TwoD {
                        origin: listener_pos,
                        direction,
                        max_distance: distance.min(self.audio.config.max_ray_distance_2d),
                        mask: sound.options.occlusion_mask,
                    });
                } else {
                    self.audio.scratch_sound_ray_results[index] = AudioRaycastResult::TwoD(None);
                }
            } else if let Some(pos) = sound.last_3d {
                if self.audio.counters.raycasts >= self.audio.config.rays_per_tick_3d {
                    continue;
                }
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
                let distance = listener_pos.distance_to(pos);
                let direction = listener_pos.direction_to(pos);
                self.audio.counters.raycasts = self.audio.counters.raycasts.saturating_add(1);
                if sound.options.enable_propagation {
                    self.audio.scratch_ray_indices.push(index);
                    self.audio
                        .scratch_ray_inputs
                        .push(AudioRaycastInput::ThreeD {
                            origin: listener_pos,
                            direction,
                            max_distance: distance.min(self.audio.config.max_ray_distance_3d),
                            include_areas: false,
                        });
                } else {
                    self.audio.scratch_sound_ray_results[index] = AudioRaycastResult::ThreeD(None);
                }
            }
        }
        let due_2d = self
            .audio
            .scratch_ray_inputs
            .iter()
            .any(|input| matches!(input, AudioRaycastInput::TwoD { .. }));
        let due_3d = self
            .audio
            .scratch_ray_inputs
            .iter()
            .any(|input| matches!(input, AudioRaycastInput::ThreeD { .. }));
        if due_2d {
            self.prepare_audio_raycast_2d();
        }
        if due_3d {
            self.prepare_audio_raycast_3d();
        }
        let mut ray_inputs = std::mem::take(&mut self.audio.scratch_ray_inputs);
        let mut ray_indices = std::mem::take(&mut self.audio.scratch_ray_indices);
        let mut ray_outputs = std::mem::take(&mut self.audio.scratch_ray_outputs);
        ray_outputs.resize(ray_inputs.len(), AudioRaycastResult::None);
        self.cast_prepared_audio_rays(
            &ray_inputs,
            &mut ray_outputs,
            ray_inputs.len() >= PARALLEL_AUDIO_RAY_THRESHOLD,
        );
        for (sound_index, output) in ray_indices.iter().zip(ray_outputs.iter()) {
            self.audio.scratch_sound_ray_results[*sound_index] = *output;
        }
        ray_inputs.clear();
        ray_indices.clear();
        ray_outputs.clear();
        self.audio.scratch_ray_inputs = ray_inputs;
        self.audio.scratch_ray_indices = ray_indices;
        self.audio.scratch_ray_outputs = ray_outputs;
        for (index, sound) in sounds.iter_mut().enumerate() {
            if sound.elapsed_since_prop < tick {
                self.audio.counters.cache_hits = self.audio.counters.cache_hits.saturating_add(1);
                continue;
            }
            sound.elapsed_since_prop = 0.0;
            if matches!(
                self.audio.scratch_sound_ray_results[index],
                AudioRaycastResult::None
            ) {
                self.audio.counters.cache_hits = self.audio.counters.cache_hits.saturating_add(1);
                continue;
            }
            let result = match (
                sound.last_2d,
                sound.last_3d,
                self.audio.scratch_sound_ray_results[index],
            ) {
                (Some(pos), _, AudioRaycastResult::TwoD(hit)) => self.solve_2d(pos, sound, hit),
                (_, Some(pos), AudioRaycastResult::ThreeD(hit)) => self.solve_3d(pos, sound, hit),
                _ => None,
            };
            if let Some(result) = result {
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
                            pan: perro_pawdio::AudioPan::new(
                                result.pan[0],
                                result.pan[1],
                                result.pan[2],
                            ),
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
        }
        self.audio.counters.active_positional = sounds.len() as u32;
        self.audio.counters.propagation_time = start.elapsed();
        self.audio.sounds = sounds;
    }

    fn finish_audio_sound_updates(&mut self, sounds: Vec<ActiveSpatialSound>, start: Instant) {
        self.audio.counters.active_positional = sounds.len() as u32;
        self.audio.counters.propagation_time = start.elapsed();
        self.audio.sounds = sounds;
    }

    fn refresh_audio_scene_flags(&mut self) {
        let node_count = self.nodes.len();
        if self.audio.audio_scene_flags_node_count == node_count {
            return;
        }
        self.audio.audio_scene_flags_node_count = node_count;
        self.audio.has_audio_mask_2d = false;
        self.audio.has_audio_portal_2d = false;
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
                && self.audio.has_audio_zone_2d
                && self.audio.has_audio_zone_3d
            {
                break;
            }
        }
    }

    fn solve_listener_field_2d(&mut self, sounds: &mut [ActiveSpatialSound], dt: f32, tick: f32) {
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

    fn solve_listener_field_3d(&mut self, sounds: &mut [ActiveSpatialSound], dt: f32, tick: f32) {
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

    fn apply_spatial_result(&mut self, sound: &mut ActiveSpatialSound, result: PropagationResult) {
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

    fn drain_resource_spatial_audio(&mut self) {
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
    }

    fn refresh_spatial_position(&mut self, sound: &mut ActiveSpatialSound) {
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

    fn solve_2d(
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

    fn solve_3d(
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

    fn bounce_energy(&self, reflection: f32, max_bounces: u32) -> f32 {
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

    fn audio_material_for_node(&self, node: NodeID) -> Option<AudioMaterial> {
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

    fn audio_diffusion_for_node(&self, node: NodeID) -> AudioDiffusion {
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

    fn audio_thickness_2d(&self, node: NodeID) -> f32 {
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

    fn first_audio_mask_2d(&mut self, from: Vector2, to: Vector2) -> Option<AudioHit2D> {
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

    fn best_audio_portal_2d(&mut self, from: Vector2, to: Vector2) -> Option<(Vector2, f32)> {
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

    fn audio_zone_mix_2d(&mut self, listener_pos: Vector2, source_pos: Vector2) -> AudioZoneMix {
        let mut mix = AudioZoneMix::default();
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioZone2D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioZone2D(zone)) = self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.enabled {
                continue;
            }
            let effect = zone.effect;
            let affect_listener = zone.affect_listener;
            let affect_emitters = zone.affect_emitters;
            let affect_path = zone.affect_path;
            let listener_inside =
                affect_listener && self.point_in_audio_zone_2d(zone_id, listener_pos);
            let source_inside = affect_emitters && self.point_in_audio_zone_2d(zone_id, source_pos);
            let path_inside =
                affect_path && self.segment_hits_audio_zone_2d(zone_id, listener_pos, source_pos);
            if listener_inside || source_inside || path_inside {
                mix.add(effect);
            }
        }
        mix
    }

    fn point_in_audio_zone_2d(&mut self, zone: NodeID, point: Vector2) -> bool {
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half_w, half_h)) = self.audio_zone_shape_2d(child) else {
                continue;
            };
            if point.x >= center.x - half_w
                && point.x <= center.x + half_w
                && point.y >= center.y - half_h
                && point.y <= center.y + half_h
            {
                return true;
            }
        }
        false
    }

    fn segment_hits_audio_zone_2d(&mut self, zone: NodeID, from: Vector2, to: Vector2) -> bool {
        let dir = to - from;
        if dir.length() <= 0.0001 {
            return false;
        }
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half_w, half_h)) = self.audio_zone_shape_2d(child) else {
                continue;
            };
            if segment_aabb(from, dir, center, half_w, half_h).is_some() {
                return true;
            }
        }
        false
    }

    fn audio_zone_shape_2d(&mut self, node: NodeID) -> Option<(Vector2, f32, f32)> {
        let shape_kind = self
            .nodes
            .get(node)
            .and_then(|shape_node| match &shape_node.data {
                SceneNodeData::CollisionShape2D(shape) => Some(shape.shape),
                _ => None,
            })?;
        let global = self.get_global_transform_2d(node)?;
        let sx = global.scale.x.abs().max(0.0001);
        let sy = global.scale.y.abs().max(0.0001);
        let (half_w, half_h) = match shape_kind {
            perro_nodes::Shape2D::Quad { width, height } => {
                (width.abs() * sx * 0.5, height.abs() * sy * 0.5)
            }
            perro_nodes::Shape2D::Circle { radius } => (radius.abs() * sx, radius.abs() * sy),
            perro_nodes::Shape2D::Triangle { width, height, .. } => {
                (width.abs() * sx * 0.5, height.abs() * sy * 0.5)
            }
        };
        Some((global.position, half_w, half_h))
    }

    fn audio_zone_mix_3d(&mut self, listener_pos: Vector3, source_pos: Vector3) -> AudioZoneMix {
        let mut mix = AudioZoneMix::default();
        self.audio.scratch_ids.clear();
        for (id, node) in self.nodes.iter() {
            if matches!(node.data, SceneNodeData::AudioZone3D(_)) {
                self.audio.scratch_ids.push(id);
            }
        }
        for index in 0..self.audio.scratch_ids.len() {
            let zone_id = self.audio.scratch_ids[index];
            let Some(SceneNodeData::AudioZone3D(zone)) = self.nodes.get(zone_id).map(|n| &n.data)
            else {
                continue;
            };
            if !zone.enabled {
                continue;
            }
            let effect = zone.effect;
            let affect_listener = zone.affect_listener;
            let affect_emitters = zone.affect_emitters;
            let affect_path = zone.affect_path;
            let listener_inside =
                affect_listener && self.point_in_audio_zone_3d(zone_id, listener_pos);
            let source_inside = affect_emitters && self.point_in_audio_zone_3d(zone_id, source_pos);
            let path_inside =
                affect_path && self.segment_hits_audio_zone_3d(zone_id, listener_pos, source_pos);
            if listener_inside || source_inside || path_inside {
                mix.add(effect);
            }
        }
        mix
    }

    fn point_in_audio_zone_3d(&mut self, zone: NodeID, point: Vector3) -> bool {
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half)) = self.audio_zone_shape_3d(child) else {
                continue;
            };
            if point.x >= center.x - half.x
                && point.x <= center.x + half.x
                && point.y >= center.y - half.y
                && point.y <= center.y + half.y
                && point.z >= center.z - half.z
                && point.z <= center.z + half.z
            {
                return true;
            }
        }
        false
    }

    fn segment_hits_audio_zone_3d(&mut self, zone: NodeID, from: Vector3, to: Vector3) -> bool {
        let dir = to - from;
        if dir.length() <= 0.0001 {
            return false;
        }
        self.audio.scratch_child_ids.clear();
        if let Some(node) = self.nodes.get(zone) {
            self.audio
                .scratch_child_ids
                .extend_from_slice(node.children_slice());
        }
        for index in 0..self.audio.scratch_child_ids.len() {
            let child = self.audio.scratch_child_ids[index];
            let Some((center, half)) = self.audio_zone_shape_3d(child) else {
                continue;
            };
            if segment_aabb_3d(from, dir, center, half).is_some() {
                return true;
            }
        }
        false
    }

    fn audio_zone_shape_3d(&mut self, node: NodeID) -> Option<(Vector3, Vector3)> {
        let shape_kind = self
            .nodes
            .get(node)
            .and_then(|shape_node| match &shape_node.data {
                SceneNodeData::CollisionShape3D(shape) => Some(shape.shape.clone()),
                _ => None,
            })?;
        let global = self.get_global_transform_3d(node)?;
        let scale = Vector3::new(
            global.scale.x.abs().max(0.0001),
            global.scale.y.abs().max(0.0001),
            global.scale.z.abs().max(0.0001),
        );
        let half = match shape_kind {
            perro_nodes::Shape3D::Cube { size }
            | perro_nodes::Shape3D::TriPrism { size }
            | perro_nodes::Shape3D::TriangularPyramid { size }
            | perro_nodes::Shape3D::SquarePyramid { size } => Vector3::new(
                size.x.abs() * scale.x * 0.5,
                size.y.abs() * scale.y * 0.5,
                size.z.abs() * scale.z * 0.5,
            ),
            perro_nodes::Shape3D::Sphere { radius } => Vector3::new(
                radius.abs() * scale.x,
                radius.abs() * scale.y,
                radius.abs() * scale.z,
            ),
            perro_nodes::Shape3D::Capsule { radius, .. }
            | perro_nodes::Shape3D::Cylinder { radius, .. }
            | perro_nodes::Shape3D::Cone { radius, .. } => Vector3::new(
                radius.abs() * scale.x,
                radius.abs() * scale.y,
                radius.abs() * scale.z,
            ),
            perro_nodes::Shape3D::TriMesh { .. } => scale,
        };
        Some((global.position, half))
    }

    fn audio_thickness_3d(&self, node: NodeID) -> f32 {
        self.nodes
            .get(node)
            .and_then(|n| {
                n.children_slice()
                    .iter()
                    .find_map(|child| self.nodes.get(*child))
            })
            .and_then(|n| match &n.data {
                SceneNodeData::CollisionShape3D(CollisionShape3D { shape, .. }) => match shape {
                    perro_nodes::Shape3D::Cube { size } => Some(size.x.min(size.y).min(size.z)),
                    perro_nodes::Shape3D::Sphere { radius } => Some(radius * 2.0),
                    perro_nodes::Shape3D::Capsule { radius, .. }
                    | perro_nodes::Shape3D::Cylinder { radius, .. }
                    | perro_nodes::Shape3D::Cone { radius, .. } => Some(radius * 2.0),
                    perro_nodes::Shape3D::TriPrism { size }
                    | perro_nodes::Shape3D::TriangularPyramid { size }
                    | perro_nodes::Shape3D::SquarePyramid { size } => {
                        Some(size.x.min(size.y).min(size.z))
                    }
                    perro_nodes::Shape3D::TriMesh { .. } => Some(1.0),
                },
                _ => None,
            })
            .unwrap_or(1.0)
    }

    fn start_spatial_sound(
        &mut self,
        audio: RuntimeAudio<'_>,
        pos: SpatialSoundPos,
        options: SpatialAudioOptions,
        last_2d: Option<Vector2>,
        last_3d: Option<Vector3>,
    ) -> bool {
        let range = options.range.max(0.0001);
        let bus_id = options.bus_id;
        let pan = perro_pawdio::AudioPan::CENTER;
        let playback_id = self.resource_api.bark.lock().ok().and_then(|guard| {
            guard.as_ref().and_then(|player| {
                player.play_spatial_source(perro_pawdio::AudioPlaybackRequest {
                    id: 0,
                    source: audio.source,
                    bus_id,
                    looped: audio.looped,
                    volume: audio.volume,
                    speed: audio.effects.speed,
                    pan,
                    low_pass: audio.effects.low_pass,
                    reverb_send: audio.effects.reverb_send,
                    echo: audio.effects.echo,
                    reflection: audio.effects.reflection,
                    occlusion: audio.effects.occlusion,
                    eq: perro_pawdio::AudioEq {
                        low_gain: audio.effects.eq.low_gain,
                        mid_gain: audio.effects.eq.mid_gain,
                        high_gain: audio.effects.eq.high_gain,
                    },
                    compression: perro_pawdio::AudioCompression {
                        threshold: audio.effects.compression.threshold,
                        ratio: audio.effects.compression.ratio,
                        attack: audio.effects.compression.attack,
                        release: audio.effects.compression.release,
                    },
                    from_start: audio.from_start,
                    from_end: audio.from_end,
                })
            })
        });
        let remaining = if audio.looped {
            None
        } else {
            self.resource_api.audio_length_seconds(audio.source)
        };
        self.audio.sounds.push(ActiveSpatialSound {
            source: audio.source.to_string(),
            looped: audio.looped,
            volume: audio.volume,
            effects: audio.effects,
            options: SpatialAudioOptions { range, ..options },
            pos,
            last_2d,
            last_3d,
            playback_id,
            elapsed_since_prop: f32::MAX,
            remaining,
            last_result: None,
        });
        true
    }

    fn play_runtime_audio_2d(
        &mut self,
        audio: RuntimeAudio<'_>,
        position: Vector2,
        options: SpatialAudioOptions,
    ) -> bool {
        self.start_spatial_sound(
            audio,
            SpatialSoundPos::TwoD(position),
            options,
            Some(position),
            None,
        )
    }

    fn play_runtime_audio_3d(
        &mut self,
        audio: RuntimeAudio<'_>,
        position: Vector3,
        options: SpatialAudioOptions,
    ) -> bool {
        self.start_spatial_sound(
            audio,
            SpatialSoundPos::ThreeD(position),
            options,
            None,
            Some(position),
        )
    }
}

impl RuntimeAudioAPI for Runtime {
    fn play_runtime_audio_attached(
        &mut self,
        audio: RuntimeAudio<'_>,
        node: NodeID,
        options: SpatialAudioOptions,
    ) -> bool {
        let Some(spatial) = self.nodes.get(node).map(|n| n.spatial()) else {
            return false;
        };
        match spatial {
            perro_nodes::Spatial::TwoD => {
                let Some(global) = self.get_global_transform_2d(node) else {
                    return false;
                };
                self.start_spatial_sound(
                    audio,
                    SpatialSoundPos::Attached(node),
                    options,
                    Some(global.position),
                    None,
                )
            }
            perro_nodes::Spatial::ThreeD => {
                let Some(global) = self.get_global_transform_3d(node) else {
                    return false;
                };
                self.start_spatial_sound(
                    audio,
                    SpatialSoundPos::Attached(node),
                    options,
                    None,
                    Some(global.position),
                )
            }
            perro_nodes::Spatial::None => false,
        }
    }

    fn stop_runtime_audio_attached(&mut self, node: NodeID, source: &str) -> bool {
        let mut stopped = false;
        let mut i = 0usize;
        while i < self.audio.sounds.len() {
            let matches = matches!(self.audio.sounds[i].pos, SpatialSoundPos::Attached(id) if id == node)
                && self.audio.sounds[i].source == source;
            if matches {
                if let Some(id) = self.audio.sounds[i].playback_id
                    && let Ok(guard) = self.resource_api.bark.lock()
                    && let Some(player) = guard.as_ref()
                {
                    let _ = player.stop_playback(id);
                }
                self.audio.sounds.remove(i);
                stopped = true;
            } else {
                i += 1;
            }
        }
        stopped
    }
}

fn segment_aabb(
    from: Vector2,
    delta: Vector2,
    center: Vector2,
    half_w: f32,
    half_h: f32,
) -> Option<(f32, Vector2)> {
    let min = Vector2::new(center.x - half_w, center.y - half_h);
    let max = Vector2::new(center.x + half_w, center.y + half_h);
    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;
    let mut normal = Vector2::ZERO;
    for axis in 0..2 {
        let origin = if axis == 0 { from.x } else { from.y };
        let dir = if axis == 0 { delta.x } else { delta.y };
        let lo = if axis == 0 { min.x } else { min.y };
        let hi = if axis == 0 { max.x } else { max.y };
        if dir.abs() <= 0.000001 {
            if origin < lo || origin > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir;
        let mut t1 = (lo - origin) * inv;
        let mut t2 = (hi - origin) * inv;
        let mut n = if axis == 0 {
            Vector2::new(-1.0, 0.0)
        } else {
            Vector2::new(0.0, -1.0)
        };
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            n = -n;
        }
        if t1 > t_min {
            t_min = t1;
            normal = n;
        }
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    (0.0..=1.0).contains(&t_min).then_some((t_min, normal))
}

fn segment_aabb_3d(from: Vector3, delta: Vector3, center: Vector3, half: Vector3) -> Option<f32> {
    let min = center - half;
    let max = center + half;
    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;
    for axis in 0..3 {
        let origin = match axis {
            0 => from.x,
            1 => from.y,
            _ => from.z,
        };
        let dir = match axis {
            0 => delta.x,
            1 => delta.y,
            _ => delta.z,
        };
        let lo = match axis {
            0 => min.x,
            1 => min.y,
            _ => min.z,
        };
        let hi = match axis {
            0 => max.x,
            1 => max.y,
            _ => max.z,
        };
        if dir.abs() <= 0.000001 {
            if origin < lo || origin > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir;
        let mut t1 = (lo - origin) * inv;
        let mut t2 = (hi - origin) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    (0.0..=1.0).contains(&t_min).then_some(t_min)
}

fn inverse_rotate_vec3(rotation: [f32; 4], v: Vector3) -> Vector3 {
    let [x, y, z, w] = normalized_quat(rotation);
    rotate_vec3([-x, -y, -z, w], v)
}

fn normalized_quat(rotation: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = rotation;
    let len_sq = x * x + y * y + z * z + w * w;
    if len_sq <= 0.000_001 || !len_sq.is_finite() {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = len_sq.sqrt().recip();
    [x * inv, y * inv, z * inv, w * inv]
}

fn rotate_vec3(rotation: [f32; 4], v: Vector3) -> Vector3 {
    let [x, y, z, w] = normalized_quat(rotation);
    let qv = Vector3::new(x, y, z);
    let t = qv.cross(v) * 2.0;
    v + t * w + qv.cross(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::{
        AudioMask2D, AudioPortal2D, AudioZone2D, AudioZone3D, CollisionShape2D, CollisionShape3D,
        SceneNode, SceneNodeData, StaticBody2D,
    };
    use perro_resource_context::sub_apis::{Audio, Audio2D, Audio3D};
    use perro_runtime_context::sub_apis::NodeAPI;
    use perro_structs::{Quaternion, Transform2D, Transform3D};

    fn looped_audio() -> RuntimeAudio<'static> {
        RuntimeAudio {
            source: "res://missing.wav",
            looped: true,
            volume: 1.0,
            effects: AudioEffects::new(),
            from_start: 0.0,
            from_end: 0.0,
        }
    }

    #[test]
    fn no_active_sounds_skip_propagation() {
        let mut runtime = Runtime::new();
        runtime.update_audio_propagation(1.0 / 60.0);
        assert_eq!(runtime.audio.counters.active_positional, 0);
        assert_eq!(runtime.audio.counters.raycasts, 0);
    }

    #[test]
    fn unobstructed_sound_stays_direct() {
        let mut runtime = Runtime::new();
        assert!(runtime.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        runtime.update_audio_propagation(1.0);
        let result = runtime.audio.sounds[0].last_result.expect("result");
        assert_eq!(result.occlusion, 0.0);
        assert!(result.volume > 0.4);
        assert_eq!(result.perceived_2d, Some(Vector2::new(5.0, 0.0)));
    }

    #[test]
    fn resource_2d_and_3d_audio_enter_propagation_queue() {
        let mut runtime = Runtime::new();
        assert!(runtime.resource_api.play_audio_2d(
            None,
            Audio2D::from_audio(
                Audio::new("res://point2d.wav"),
                Vector2::new(5.0, 0.0),
                10.0
            ),
        ));
        assert!(runtime.resource_api.play_audio_3d(
            None,
            Audio3D::from_audio(
                Audio::new("res://point3d.wav"),
                Vector3::new(0.0, 0.0, -5.0),
                10.0,
            ),
        ));
        assert!(runtime.audio.sounds.is_empty());
        runtime.update_audio_propagation(1.0);
        assert_eq!(runtime.audio.sounds.len(), 2);
        assert!(
            runtime
                .audio
                .sounds
                .iter()
                .any(|sound| matches!(sound.pos, SpatialSoundPos::TwoD(_)))
        );
        assert!(
            runtime
                .audio
                .sounds
                .iter()
                .any(|sound| matches!(sound.pos, SpatialSoundPos::ThreeD(_)))
        );
    }

    #[test]
    fn wall_between_listener_and_source_muffles() {
        let mut runtime = Runtime::new();
        let wall = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, wall, shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            wall,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(runtime.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        runtime.update_audio_propagation(1.0);
        let result = runtime.audio.sounds[0].last_result.expect("result");
        assert!(result.occlusion > 0.0);
        assert!(result.low_pass > 0.0);
        assert!(result.volume < 0.5);
    }

    #[test]
    fn thin_collider_transmits_more_than_thick_collider() {
        let mut thin = Runtime::new();
        let wall = NodeAPI::create::<StaticBody2D>(&mut thin);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut thin);
        assert!(NodeAPI::reparent(&mut thin, wall, shape));
        if let Some(node) = thin.nodes.get_mut(shape)
            && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
        {
            shape.shape = perro_nodes::Shape2D::Quad {
                width: 0.1,
                height: 1.0,
            };
        }
        assert!(NodeAPI::set_global_transform_2d(
            &mut thin,
            wall,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(thin.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        thin.update_audio_propagation(1.0);
        let thin_volume = thin.audio.sounds[0].last_result.expect("result").volume;

        let mut thick = Runtime::new();
        let wall = NodeAPI::create::<StaticBody2D>(&mut thick);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut thick);
        assert!(NodeAPI::reparent(&mut thick, wall, shape));
        if let Some(node) = thick.nodes.get_mut(shape)
            && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
        {
            shape.shape = perro_nodes::Shape2D::Quad {
                width: 4.0,
                height: 1.0,
            };
        }
        assert!(NodeAPI::set_global_transform_2d(
            &mut thick,
            wall,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(thick.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        thick.update_audio_propagation(1.0);
        let thick_volume = thick.audio.sounds[0].last_result.expect("result").volume;
        assert!(thin_volume > thick_volume);
    }

    #[test]
    fn corner_path_changes_perceived_direction() {
        let mut runtime = Runtime::new();
        let wall = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, wall, shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            wall,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(runtime.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        runtime.update_audio_propagation(1.0);
        let result = runtime.audio.sounds[0].last_result.expect("result");
        assert_ne!(result.perceived_2d, Some(Vector2::new(5.0, 0.0)));
    }

    #[test]
    fn audio_mask_blocks_without_physical_collision() {
        let mut runtime = Runtime::new();
        let mask = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::AudioMask2D(
                AudioMask2D::new(),
            )));
        let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, mask, shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            mask,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(runtime.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        runtime.update_audio_propagation(1.0);
        let result = runtime.audio.sounds[0].last_result.expect("result");
        assert!(result.occlusion > 0.0);
    }

    #[test]
    fn reflection_loses_strength_per_bounce_and_stops_at_cutoff() {
        let mut runtime = Runtime::new();
        runtime.audio.config.energy_cutoff = 0.02;
        runtime.audio.config.max_bounces_2d = 4;
        let four_bounces = runtime.bounce_energy(0.5, runtime.audio.config.max_bounces_2d);
        runtime.audio.config.max_bounces_2d = 1;
        let one_bounce = runtime.bounce_energy(0.5, runtime.audio.config.max_bounces_2d);
        assert!(four_bounces > one_bounce);
        runtime.audio.config.energy_cutoff = 0.6;
        assert_eq!(runtime.bounce_energy(0.5, 4), 0.0);
    }

    #[test]
    fn audio_portal_improves_corner_opening_path() {
        let mut without_portal = Runtime::new();
        let wall = NodeAPI::create::<StaticBody2D>(&mut without_portal);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut without_portal);
        assert!(NodeAPI::reparent(&mut without_portal, wall, shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut without_portal,
            wall,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(without_portal.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        without_portal.update_audio_propagation(1.0);
        let blocked = without_portal.audio.sounds[0].last_result.expect("result");

        let mut with_portal = Runtime::new();
        let wall = NodeAPI::create::<StaticBody2D>(&mut with_portal);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut with_portal);
        assert!(NodeAPI::reparent(&mut with_portal, wall, shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut with_portal,
            wall,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        let portal = with_portal
            .nodes
            .insert(SceneNode::new(SceneNodeData::AudioPortal2D(
                AudioPortal2D::new(),
            )));
        let portal_shape = NodeAPI::create::<CollisionShape2D>(&mut with_portal);
        assert!(NodeAPI::reparent(&mut with_portal, portal, portal_shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut with_portal,
            portal,
            Transform2D::new(Vector2::new(2.5, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(with_portal.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        with_portal.update_audio_propagation(1.0);
        let opened = with_portal.audio.sounds[0].last_result.expect("result");
        assert!(opened.volume > blocked.volume);
        assert!(opened.low_pass < blocked.low_pass);
        assert_eq!(opened.perceived_2d, Some(Vector2::new(2.0, 0.0)));
    }

    #[test]
    fn audio_zone_2d_mixes_effect_when_source_enters() {
        let mut runtime = Runtime::new();
        let zone = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::AudioZone2D(
                AudioZone2D::new(),
            )));
        if let Some(node) = runtime.nodes.get_mut(zone)
            && let SceneNodeData::AudioZone2D(zone) = &mut node.data
        {
            zone.effect.reverb_send = 0.7;
            zone.effect.echo = 0.4;
            zone.effect.dampening = 0.5;
            zone.affect_listener = false;
            zone.affect_path = false;
        }
        let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, zone, shape));
        if let Some(node) = runtime.nodes.get_mut(shape)
            && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
        {
            shape.shape = perro_nodes::Shape2D::Quad {
                width: 4.0,
                height: 4.0,
            };
        }
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            shape,
            Transform2D::new(Vector2::new(5.0, 0.0), 0.0, Vector2::ONE),
        ));
        assert!(runtime.play_runtime_audio_2d(
            looped_audio(),
            Vector2::new(5.0, 0.0),
            SpatialAudioOptions::new(10.0),
        ));
        runtime.update_audio_propagation(1.0);
        let result = runtime.audio.sounds[0].last_result.expect("result");
        assert!(result.reverb_send >= 0.7);
        assert!(result.reflection >= 0.4);
        assert!(result.low_pass >= 0.5);
        assert!(result.volume < 0.5);
    }

    #[test]
    fn audio_zone_3d_mixes_effect_when_path_crosses() {
        let mut runtime = Runtime::new();
        let zone = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::AudioZone3D(
                AudioZone3D::new(),
            )));
        if let Some(node) = runtime.nodes.get_mut(zone)
            && let SceneNodeData::AudioZone3D(zone) = &mut node.data
        {
            zone.effect.reverb_send = 0.6;
            zone.effect.echo = 0.3;
            zone.effect.dampening = 0.4;
            zone.affect_listener = false;
            zone.affect_emitters = false;
        }
        let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, zone, shape));
        if let Some(node) = runtime.nodes.get_mut(shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.shape = perro_nodes::Shape3D::Cube {
                size: Vector3::new(2.0, 2.0, 2.0),
            };
        }
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            shape,
            Transform3D::new(
                Vector3::new(0.0, 0.0, -2.5),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        assert!(runtime.play_runtime_audio_3d(
            looped_audio(),
            Vector3::new(0.0, 0.0, -5.0),
            SpatialAudioOptions::new(10.0),
        ));
        runtime.update_audio_propagation(1.0);
        let result = runtime.audio.sounds[0].last_result.expect("result");
        assert!(result.reverb_send >= 0.6);
        assert!(result.reflection >= 0.3);
        assert!(result.low_pass >= 0.4);
        assert!(result.volume < 0.5);
    }

    #[test]
    fn attached_sound_follows_and_freezes_after_remove() {
        let mut runtime = Runtime::new();
        let node = NodeAPI::create::<perro_nodes::Node2D>(&mut runtime);
        assert!(runtime.play_runtime_audio_attached(
            looped_audio(),
            node,
            SpatialAudioOptions::new(10.0),
        ));
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            node,
            Transform2D::new(Vector2::new(3.0, 0.0), 0.0, Vector2::ONE),
        ));
        runtime.update_audio_propagation(1.0);
        assert_eq!(
            runtime.audio.sounds[0].last_2d,
            Some(Vector2::new(3.0, 0.0))
        );
        assert!(NodeAPI::remove_node(&mut runtime, node));
        runtime.update_audio_propagation(1.0);
        assert_eq!(
            runtime.audio.sounds[0].last_2d,
            Some(Vector2::new(3.0, 0.0))
        );
    }

    #[test]
    fn stop_attached_matches_node_and_source() {
        let mut runtime = Runtime::new();
        let a = NodeAPI::create::<perro_nodes::Node2D>(&mut runtime);
        let b = NodeAPI::create::<perro_nodes::Node2D>(&mut runtime);
        assert!(runtime.play_runtime_audio_attached(
            looped_audio(),
            a,
            SpatialAudioOptions::new(10.0)
        ));
        assert!(runtime.play_runtime_audio_attached(
            looped_audio(),
            b,
            SpatialAudioOptions::new(10.0)
        ));
        assert!(runtime.stop_runtime_audio_attached(a, "res://missing.wav"));
        assert_eq!(runtime.audio.sounds.len(), 1);
        assert!(matches!(runtime.audio.sounds[0].pos, SpatialSoundPos::Attached(id) if id == b));
    }
}
