use super::state::{RuntimeLocalizationState, RuntimeResourceState};
use crate::runtime_project::{
    StaticAnimationLookup, StaticAnimationTreeLookup, StaticAudioLookup, StaticCsvLookup,
    StaticLocalizationLookup, StaticMaterialLookup, StaticSkeletonLookup,
};
use perro_ids::{SoundFontID, string_to_u64};
use perro_pawdio::{AudioController, MidiChannel, MidiProgram, MidiSound, Note};
use perro_project::LocalizationConfig;
use perro_render_bridge::{RenderCommand, RenderEvent};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub(crate) enum QueuedSpatialAudioPos {
    TwoD(perro_structs::Vector2),
    ThreeD(perro_structs::Vector3),
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedSpatialAudio {
    pub source: String,
    pub bus_id: Option<perro_ids::AudioBusID>,
    pub looped: bool,
    pub volume: f32,
    pub effects: perro_resource_api::sub_apis::AudioEffects,
    pub from_start: f32,
    pub from_end: f32,
    pub range: f32,
    pub audio_layer: perro_structs::BitMask,
    pub enable_propagation: bool,
    pub pos: QueuedSpatialAudioPos,
    pub direction_2d: perro_resource_api::sub_apis::AudioDirection<perro_structs::Vector2>,
    pub direction_3d: perro_resource_api::sub_apis::AudioDirection<perro_structs::Vector3>,
}

#[derive(Clone, Debug)]
pub(crate) enum QueuedMidiSound {
    BuiltIn,
    SoundFont(SoundFontID),
}

impl QueuedMidiSound {
    pub(crate) fn from_sound(sound: MidiSound) -> Self {
        match sound {
            MidiSound::BuiltIn => Self::BuiltIn,
            MidiSound::SoundFont(id) => Self::SoundFont(id),
        }
    }

    pub(crate) fn as_sound(&self) -> MidiSound {
        match self {
            Self::BuiltIn => MidiSound::BuiltIn,
            Self::SoundFont(id) => MidiSound::SoundFont(*id),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedMidiNoteOptions {
    pub velocity: u8,
    pub sustain: std::time::Duration,
    pub channel: MidiChannel,
    pub program: MidiProgram,
    pub sound: QueuedMidiSound,
    pub bus_id: Option<perro_ids::AudioBusID>,
    pub volume: f32,
    pub pan: perro_pawdio::AudioPan,
}

impl QueuedMidiNoteOptions {
    pub(crate) fn from_options(options: perro_pawdio::MidiNoteOptions) -> Self {
        Self {
            velocity: options.velocity,
            sustain: options.sustain,
            channel: options.channel,
            program: options.program,
            sound: QueuedMidiSound::from_sound(options.sound),
            bus_id: options.bus_id,
            volume: options.volume,
            pan: options.pan,
        }
    }

    pub(crate) fn as_options(&self) -> perro_pawdio::MidiNoteOptions {
        perro_pawdio::MidiNoteOptions {
            velocity: self.velocity,
            sustain: self.sustain,
            channel: self.channel,
            program: self.program,
            sound: self.sound.as_sound(),
            bus_id: self.bus_id,
            volume: self.volume,
            pan: self.pan,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedMidiSong {
    pub source: String,
    pub sound: QueuedMidiSound,
    pub bus_id: Option<perro_ids::AudioBusID>,
    pub volume: f32,
    pub looped: bool,
}

impl QueuedMidiSong {
    pub(crate) fn from_song(song: perro_pawdio::MidiSong) -> Self {
        Self {
            source: song.source.to_string(),
            sound: QueuedMidiSound::from_sound(song.sound),
            bus_id: song.bus_id,
            volume: song.volume,
            looped: song.looped,
        }
    }

    pub(crate) fn as_song(&self) -> perro_pawdio::MidiSong<'_> {
        perro_pawdio::MidiSong {
            source: self.source.as_str(),
            sound: self.sound.as_sound(),
            bus_id: self.bus_id,
            volume: self.volume,
            looped: self.looped,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum QueuedSpatialMidiKind {
    Note {
        id: u64,
        note: Note,
        options: QueuedMidiNoteOptions,
        held: bool,
    },
    File {
        id: u64,
        song: QueuedMidiSong,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedSpatialMidi {
    pub kind: QueuedSpatialMidiKind,
    pub range: f32,
    pub pos: QueuedSpatialAudioPos,
}

pub struct RuntimeResourceApi {
    pub(super) state: Mutex<RuntimeResourceState>,
    pub(super) localization: std::sync::RwLock<RuntimeLocalizationState>,
    pub(crate) bark: Mutex<Option<AudioController>>,
    pub(crate) spatial_audio_queue: Mutex<Vec<QueuedSpatialAudio>>,
    pub(crate) spatial_midi_queue: Mutex<Vec<QueuedSpatialMidi>>,
    pub(crate) next_spatial_midi_id: std::sync::atomic::AtomicU64,
    pub(crate) audio_listener_2d: Mutex<Option<perro_pawdio::AudioListener2D>>,
    pub(crate) audio_listener_3d: Mutex<Option<perro_pawdio::AudioListener3D>>,
    pub(crate) audio_listener_options_2d: Mutex<perro_structs::AudioListenerOptions>,
    pub(crate) audio_listener_options_3d: Mutex<perro_structs::AudioListenerOptions>,
    pub(super) static_material_lookup: Option<StaticMaterialLookup>,
    pub(super) static_skeleton_lookup: Option<StaticSkeletonLookup>,
    pub(super) static_animation_lookup: Option<StaticAnimationLookup>,
    pub(super) static_animation_tree_lookup: Option<StaticAnimationTreeLookup>,
    pub(super) static_localization_lookup: Option<StaticLocalizationLookup>,
    pub(super) static_csv_lookup: Option<StaticCsvLookup>,
    pub(super) csv_cache: Mutex<HashMap<u64, &'static perro_csv::PerroCsv>>,
    pub(super) skeleton_bones_2d_cache:
        Mutex<HashMap<String, Vec<perro_nodes::skeleton_2d::Bone2D>>>,
    pub(super) skeleton_bones_3d_cache:
        Mutex<HashMap<String, Vec<perro_nodes::skeleton_3d::Bone3D>>>,
    pub(super) viewport_size: Mutex<(u32, u32)>,
}

impl RuntimeResourceApi {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        static_material_lookup: Option<StaticMaterialLookup>,
        static_audio_lookup: Option<StaticAudioLookup>,
        static_skeleton_lookup: Option<StaticSkeletonLookup>,
        static_animation_lookup: Option<StaticAnimationLookup>,
        static_animation_tree_lookup: Option<StaticAnimationTreeLookup>,
        static_localization_lookup: Option<StaticLocalizationLookup>,
        static_csv_lookup: Option<StaticCsvLookup>,
        localization_config: Option<LocalizationConfig>,
    ) -> Arc<Self> {
        let api = Arc::new(Self {
            state: Mutex::new(RuntimeResourceState::new()),
            localization: std::sync::RwLock::new(RuntimeLocalizationState::new(
                localization_config.as_ref(),
            )),
            bark: Mutex::new(AudioController::new(static_audio_lookup).ok()),
            spatial_audio_queue: Mutex::new(Vec::new()),
            spatial_midi_queue: Mutex::new(Vec::new()),
            next_spatial_midi_id: std::sync::atomic::AtomicU64::new(1),
            audio_listener_2d: Mutex::new(None),
            audio_listener_3d: Mutex::new(None),
            audio_listener_options_2d: Mutex::new(perro_structs::AudioListenerOptions::new()),
            audio_listener_options_3d: Mutex::new(perro_structs::AudioListenerOptions::new()),
            static_material_lookup,
            static_skeleton_lookup,
            static_animation_lookup,
            static_animation_tree_lookup,
            static_localization_lookup,
            static_csv_lookup,
            csv_cache: Mutex::new(HashMap::new()),
            skeleton_bones_2d_cache: Mutex::new(HashMap::new()),
            skeleton_bones_3d_cache: Mutex::new(HashMap::new()),
            viewport_size: Mutex::new((1, 1)),
        });
        api.initialize_localization();
        api
    }

    pub(crate) fn set_viewport_size(&self, width: u32, height: u32) {
        let mut viewport = self
            .viewport_size
            .lock()
            .expect("resource api viewport mutex poisoned");
        *viewport = (width.max(1), height.max(1));
    }

    pub(crate) fn set_audio_listener_2d(
        &self,
        position: [f32; 2],
        rotation_radians: f32,
        options: perro_structs::AudioListenerOptions,
    ) {
        let mut listener = self
            .audio_listener_2d
            .lock()
            .expect("resource api audio 2d listener mutex poisoned");
        *listener = Some(perro_pawdio::AudioListener2D {
            position,
            rotation_radians,
        });
        let mut listener_options = self
            .audio_listener_options_2d
            .lock()
            .expect("resource api audio 2d listener options mutex poisoned");
        *listener_options = options;
    }

    pub(crate) fn set_audio_listener_3d(
        &self,
        position: [f32; 3],
        rotation: [f32; 4],
        options: perro_structs::AudioListenerOptions,
    ) {
        let mut listener = self
            .audio_listener_3d
            .lock()
            .expect("resource api audio 3d listener mutex poisoned");
        *listener = Some(perro_pawdio::AudioListener3D { position, rotation });
        let mut listener_options = self
            .audio_listener_options_3d
            .lock()
            .expect("resource api audio 3d listener options mutex poisoned");
        *listener_options = options;
    }

    pub(crate) fn drain_commands(&self, out: &mut Vec<RenderCommand>) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        out.append(&mut state.queued_commands);
    }

    pub(crate) fn apply_render_event(&self, event: &RenderEvent) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        match event {
            RenderEvent::TextureCreated { request, id } => {
                let _ = state.occupy_texture_id(*id);
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.texture_pending_by_source.remove(&source_hash);
                    let pending_id = state.texture_pending_id_by_request.remove(request);
                    if state.texture_drop_pending.remove(&source_hash) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropTexture { id: *id },
                        ));
                        state.texture_by_source.remove(&source_hash);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_texture_id(pending_id);
                        }
                    } else {
                        state.texture_by_source.insert(source_hash, *id);
                        if state.texture_reserve_pending.remove(&source_hash) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetTextureReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
            }
            RenderEvent::MeshCreated { request, id, mesh } => {
                if id.is_nil() {
                    if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                        let source_hash = string_to_u64(&source);
                        state.mesh_pending_by_source.remove(&source_hash);
                        if let Some(pending_id) = state.mesh_pending_id_by_request.remove(request) {
                            let _ = state.free_mesh_id(pending_id);
                            state.mesh_source_by_id.remove(&pending_id);
                        }
                        state.mesh_by_source.remove(&source_hash);
                        state.mesh_reserve_pending.remove(&source_hash);
                        state.mesh_drop_pending.remove(&source_hash);
                    }
                    return;
                }
                let _ = state.occupy_mesh_id(*id);
                if let Some(mesh) = mesh {
                    state.mesh_data_by_id.insert(*id, mesh.clone());
                }
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.mesh_pending_by_source.remove(&source_hash);
                    let pending_id = state.mesh_pending_id_by_request.remove(request);
                    if let Some(pending_id) = pending_id
                        && pending_id != *id
                    {
                        // Backend resolved this request to an existing mesh id.
                        // Free the temporary pending slot to avoid mesh-id leaks/churn.
                        let _ = state.free_mesh_id(pending_id);
                        state.mesh_id_alias.insert(pending_id, *id);
                        state.mesh_source_by_id.remove(&pending_id);
                    }
                    if state.mesh_drop_pending.remove(&source_hash) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropMesh { id: *id },
                        ));
                        state.mesh_by_source.remove(&source_hash);
                        state.mesh_source_by_id.remove(id);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_mesh_id(pending_id);
                        }
                    } else {
                        state.mesh_by_source.insert(source_hash, *id);
                        state.mesh_source_by_id.insert(*id, source);
                        if state.mesh_reserve_pending.remove(&source_hash) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetMeshReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
            }
            RenderEvent::MaterialCreated { request, id } => {
                let _ = state.occupy_material_id(*id);
                let pending_id = state.material_pending_id_by_request.remove(request);
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.material_pending_by_source.remove(&source_hash);
                    if state.material_drop_pending.remove(&source_hash) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropMaterial { id: *id },
                        ));
                        state.material_by_source.remove(&source_hash);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_material_id(pending_id);
                        }
                    } else {
                        state.material_by_source.insert(source_hash, *id);
                        if state.material_reserve_pending.remove(&source_hash) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetMaterialReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
                if let Some(pending_id) = pending_id
                    && pending_id != *id
                {
                    if let Some(data) = state.material_data_by_id.remove(&pending_id) {
                        state.material_data_by_id.insert(*id, data);
                    }
                    let _ = state.free_material_id(pending_id);
                }
            }
            RenderEvent::Failed { request, .. } => {
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.texture_pending_by_source.remove(&source_hash);
                    if let Some(pending_id) = state.texture_pending_id_by_request.remove(request) {
                        let _ = state.free_texture_id(pending_id);
                    }
                    state.texture_by_source.remove(&source_hash);
                    state.texture_reserve_pending.remove(&source_hash);
                    state.texture_drop_pending.remove(&source_hash);
                }
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.mesh_pending_by_source.remove(&source_hash);
                    if let Some(pending_id) = state.mesh_pending_id_by_request.remove(request) {
                        let _ = state.free_mesh_id(pending_id);
                        state.mesh_data_by_id.remove(&pending_id);
                        state.mesh_source_by_id.remove(&pending_id);
                    }
                    state.mesh_by_source.remove(&source_hash);
                    state.mesh_reserve_pending.remove(&source_hash);
                    state.mesh_drop_pending.remove(&source_hash);
                }
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.material_pending_by_source.remove(&source_hash);
                    if let Some(pending_id) = state.material_pending_id_by_request.remove(request) {
                        let _ = state.free_material_id(pending_id);
                        state.material_data_by_id.remove(&pending_id);
                    }
                    state.material_by_source.remove(&source_hash);
                    state.material_reserve_pending.remove(&source_hash);
                    state.material_drop_pending.remove(&source_hash);
                }
                if let Some(pending_id) = state.material_pending_id_by_request.remove(request) {
                    let _ = state.free_material_id(pending_id);
                    state.material_data_by_id.remove(&pending_id);
                }
            }
            RenderEvent::WaterSamples { .. } => {}
        }
    }
}
