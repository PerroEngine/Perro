//! Spatial audio propagation and runtime audio API glue.

use super::Runtime;
use super::physics::{AudioRaycastInput, AudioRaycastResult};
use crate::rs_ctx::QueuedSpatialAudioPos;
use perro_ids::NodeID;
use perro_nodes::{CollisionShape2D, CollisionShape3D, SceneNodeData};
use perro_render_bridge::{Command2D, Command3D, DrawShape2DCommand, RenderCommand};
use perro_resource_api::sub_apis::AudioAPI;
use perro_runtime_api::sub_apis::{
    AttachedMidiTarget, AudioDirection, AudioEffects, PhysicsQueryFilter, RuntimeAudio,
    RuntimeAudioAPI, SpatialAudioOptions,
};
use perro_structs::{
    AudioDiffusion, AudioEffect, AudioMaterial, BitMask, DrawShape2D, Transform2D, Transform3D,
    Vector2, Vector3,
};
use std::f32::consts::TAU;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const PARALLEL_AUDIO_RAY_THRESHOLD: usize = 128;
const LISTENER_FIELD_SOUND_THRESHOLD_2D: usize = 64;
const LISTENER_FIELD_SOUND_THRESHOLD_3D: usize = 128;
const LISTENER_FIELD_RAYS_2D: usize = 64;
const LISTENER_FIELD_RAYS_3D: usize = 96;
const AUDIO_BOUNCE_RAYS_2D: usize = 8;
const AUDIO_BOUNCE_RAYS_3D: usize = 6;
const MAX_AUDIO_PORTAL_HOPS: usize = 32;
const AUDIO_PORTAL_EPSILON: f32 = 0.01;
const AUDIO_PORTAL_MISS_TOLERANCE: f32 = 0.25;

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

#[derive(Clone, Copy)]
struct SpatialMidiNoteStart {
    id: u64,
    note: perro_pawdio::Note,
    options: perro_pawdio::MidiNoteOptions,
    held: bool,
    pos: SpatialSoundPos,
    spatial: SpatialAudioOptions,
    last_2d: Option<Vector2>,
    last_3d: Option<Vector3>,
}

#[derive(Clone, Debug)]
struct ActiveSpatialSound {
    source: String,
    kind: ActiveSpatialSoundKind,
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

#[derive(Clone, Debug)]
enum ActiveSpatialSoundKind {
    Audio,
    MidiNote,
    MidiFile,
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

#[derive(Clone, Copy, Debug)]
struct AudioHit3D {
    node: NodeID,
    point: Vector3,
    normal: Vector3,
    distance: f32,
    material: AudioMaterial,
    thickness: f32,
}

#[derive(Clone, Copy, Debug)]
struct AudioBounceHit2D {
    point: Vector2,
    normal: Vector2,
    distance: f32,
    reflection: f32,
    reverb_send: f32,
    echo: f32,
    low_pass: f32,
    volume_loss: f32,
}

#[derive(Clone, Copy, Debug)]
struct AudioBounceHit3D {
    point: Vector3,
    normal: Vector3,
    distance: f32,
    reflection: f32,
    reverb_send: f32,
    echo: f32,
    low_pass: f32,
    volume_loss: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct AudioBouncePath2D {
    perceived: Vector2,
    distance: f32,
    reflection: f32,
    reverb_send: f32,
    echo: f32,
    low_pass: f32,
    volume: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct AudioBouncePath3D {
    perceived: Vector3,
    distance: f32,
    reflection: f32,
    reverb_send: f32,
    echo: f32,
    low_pass: f32,
    volume: f32,
}

#[derive(Clone, Copy, Debug)]
struct AudioPortalPath2D {
    exit: Vector2,
    strength: f32,
    distance: f32,
}

#[derive(Clone, Debug)]
struct AudioPortalHit2D {
    portal_id: NodeID,
    local_entry: Vector2,
    local_dir: Vector2,
    targets: Vec<NodeID>,
    strength: f32,
    distance: f32,
}

#[derive(Clone, Copy, Debug)]
struct AudioPortalPath3D {
    exit: Vector3,
    strength: f32,
    distance: f32,
}

#[derive(Clone, Debug)]
struct AudioPortalHit3D {
    portal_id: NodeID,
    local_entry: Vector3,
    local_dir: Vector3,
    targets: Vec<NodeID>,
    strength: f32,
    distance: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct AudioEffectZoneMix {
    reverb_send: f32,
    echo: f32,
    dampening: f32,
}

impl AudioEffectZoneMix {
    fn apply(&mut self, effect: AudioEffect) {
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
    has_audio_mask_3d: bool,
    has_audio_portal_2d: bool,
    has_audio_portal_3d: bool,
    has_audio_effect_zone_2d: bool,
    has_audio_effect_zone_3d: bool,
    audio_scene_flags_node_count: usize,
    debug_ray_count_3d: u32,
    prev_debug_ray_count_3d: u32,
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
            has_audio_mask_3d: false,
            has_audio_portal_2d: false,
            has_audio_portal_3d: false,
            has_audio_effect_zone_2d: false,
            has_audio_effect_zone_3d: false,
            audio_scene_flags_node_count: usize::MAX,
            debug_ray_count_3d: 0,
            prev_debug_ray_count_3d: 0,
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
        self.audio.debug_ray_count_3d = 0;
        self.drain_resource_spatial_audio();
        if self.audio.sounds.is_empty() {
            self.clear_stale_audio_debug_rays();
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
                        mask: sound.options.audio_layer,
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
        self.clear_stale_audio_debug_rays();
    }
}

mod helpers;
mod scene;
mod solve;
mod zones;
use helpers::*;

impl RuntimeAudioAPI for Runtime {
    fn set_audio_debug_rays(&mut self, enabled: bool) {
        self.audio.config.debug_rays = enabled;
        if !enabled {
            self.audio.debug_ray_count_3d = 0;
            self.clear_stale_audio_debug_rays();
        }
    }

    fn audio_debug_rays_enabled(&mut self) -> bool {
        self.audio.config.debug_rays
    }

    fn play_runtime_audio_attached(
        &mut self,
        bus_id: Option<perro_ids::AudioBusID>,
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
                    bus_id,
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
                    bus_id,
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

    fn play_midi_note_attached(
        &mut self,
        note: perro_pawdio::Note,
        node: NodeID,
        options: perro_pawdio::MidiNoteOptions,
        spatial: SpatialAudioOptions,
    ) -> bool {
        let id = self
            .resource_api
            .next_spatial_midi_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        self.start_midi_note_attached_inner(id, note, node, options, spatial, false)
    }

    fn start_midi_note_attached(
        &mut self,
        note: perro_pawdio::Note,
        node: NodeID,
        options: perro_pawdio::MidiNoteOptions,
        spatial: SpatialAudioOptions,
    ) -> Option<perro_pawdio::MidiNoteHandle> {
        let id = self
            .resource_api
            .next_spatial_midi_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        self.start_midi_note_attached_inner(id, note, node, options, spatial, true)
            .then_some(perro_pawdio::MidiNoteHandle(id))
    }

    fn play_midi_file_attached(
        &mut self,
        song: perro_pawdio::MidiSong,
        node: NodeID,
        spatial: SpatialAudioOptions,
    ) -> bool {
        let id = self
            .resource_api
            .next_spatial_midi_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let Some(node_spatial) = self.nodes.get(node).map(|n| n.spatial()) else {
            return false;
        };
        match node_spatial {
            perro_nodes::Spatial::TwoD => {
                let Some(global) = self.get_global_transform_2d(node) else {
                    return false;
                };
                self.start_spatial_midi_file(
                    id,
                    song,
                    SpatialSoundPos::Attached(node),
                    spatial,
                    Some(global.position),
                    None,
                )
            }
            perro_nodes::Spatial::ThreeD => {
                let Some(global) = self.get_global_transform_3d(node) else {
                    return false;
                };
                self.start_spatial_midi_file(
                    id,
                    song,
                    SpatialSoundPos::Attached(node),
                    spatial,
                    None,
                    Some(global.position),
                )
            }
            perro_nodes::Spatial::None => false,
        }
    }

    fn release_midi_note(&mut self, handle: perro_pawdio::MidiNoteHandle) -> bool {
        let Ok(guard) = self.resource_api.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.release_midi_note(handle)
    }

    fn stop_midi_attached(&mut self, node: NodeID, target: AttachedMidiTarget<'_>) -> bool {
        let mut stopped = false;
        let mut i = 0usize;
        while i < self.audio.sounds.len() {
            let attached =
                matches!(self.audio.sounds[i].pos, SpatialSoundPos::Attached(id) if id == node);
            let matches_target = match (&self.audio.sounds[i].kind, target) {
                (_, AttachedMidiTarget::Handle(handle)) => {
                    self.audio.sounds[i].playback_id == Some(handle.0)
                }
                (ActiveSpatialSoundKind::MidiFile, AttachedMidiTarget::Source(source)) => {
                    self.audio.sounds[i].source == source
                }
                _ => false,
            };
            if attached && matches_target {
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

impl Runtime {
    fn start_midi_note_attached_inner(
        &mut self,
        id: u64,
        note: perro_pawdio::Note,
        node: NodeID,
        options: perro_pawdio::MidiNoteOptions,
        spatial: SpatialAudioOptions,
        held: bool,
    ) -> bool {
        let Some(node_spatial) = self.nodes.get(node).map(|n| n.spatial()) else {
            return false;
        };
        match node_spatial {
            perro_nodes::Spatial::TwoD => {
                let Some(global) = self.get_global_transform_2d(node) else {
                    return false;
                };
                self.start_spatial_midi_note(SpatialMidiNoteStart {
                    id,
                    note,
                    options,
                    held,
                    pos: SpatialSoundPos::Attached(node),
                    spatial,
                    last_2d: Some(global.position),
                    last_3d: None,
                })
            }
            perro_nodes::Spatial::ThreeD => {
                let Some(global) = self.get_global_transform_3d(node) else {
                    return false;
                };
                self.start_spatial_midi_note(SpatialMidiNoteStart {
                    id,
                    note,
                    options,
                    held,
                    pos: SpatialSoundPos::Attached(node),
                    spatial,
                    last_2d: None,
                    last_3d: Some(global.position),
                })
            }
            perro_nodes::Spatial::None => false,
        }
    }
}

#[cfg(test)]
mod tests;
