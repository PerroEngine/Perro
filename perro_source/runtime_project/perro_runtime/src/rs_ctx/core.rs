use super::state::{RuntimeLocalizationState, RuntimeResourceState};
use crate::runtime_project::{
    StaticAnimationLookup, StaticAnimationTreeLookup, StaticAudioLookup, StaticCsvLookup,
    StaticLocalizationLookup, StaticMaterialLookup, StaticSkeletonLookup,
};
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
use perro_animation::{AnimationClip, AnimationTreeAsset};
use perro_ids::SoundFontID;
use perro_ids::{NodeID, TextureID, WebcamID};
use perro_pawdio::{AudioController, MicRecorder, MidiChannel, MidiProgram, MidiSound, Note};
use perro_project::LocalizationConfig;
#[cfg(not(target_arch = "wasm32"))]
use perro_render_bridge::Material3D;
use perro_render_bridge::{RenderCommand, RenderEvent};
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::sync::atomic::AtomicBool;

// bound the capture->runtime handoff; a couple frames across the few live
// webcams. try_send drops past this so a lagging runtime never grows memory.
pub(crate) const WEBCAM_FRAME_CHANNEL_CAP: usize = 8;

pub(crate) struct WebcamFrameMessage {
    pub(crate) id: WebcamID,
    pub(crate) frame: perro_resource_api::sub_apis::WebcamFrame,
}

pub(crate) struct WebcamErrorMessage {
    pub(crate) id: WebcamID,
    pub(crate) error: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeVideoFrame {
    pub(crate) rgba: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeVideoClip {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) fps: f32,
    pub(crate) frames: Arc<[RuntimeVideoFrame]>,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeVideoNode {
    pub(crate) source_hash: u64,
    pub(crate) texture: TextureID,
    pub(crate) frame_index: usize,
    pub(crate) accum: f32,
}

#[cfg(not(target_arch = "wasm32"))]
struct AsyncMaterialLoadResult {
    id: perro_ids::MaterialID,
    material: Option<Material3D>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn asset_ready_log_enabled() -> bool {
    std::env::var("PERRO_ASSET_READY_LOG")
        .ok()
        .is_some_and(|raw| matches!(raw.as_str(), "1" | "true" | "yes" | "on"))
}

#[cfg(target_arch = "wasm32")]
pub(super) fn asset_ready_log_enabled() -> bool {
    false
}

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
struct AsyncAnimationLoadResult {
    id: perro_ids::AnimationID,
    clip: Arc<AnimationClip>,
}

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
struct AsyncAnimationTreeLoadResult {
    id: perro_ids::AnimationTreeID,
    tree: Arc<AnimationTreeAsset>,
}

pub(super) struct AsyncSkeleton2DLoadResult {
    pub(super) source: String,
    pub(super) bones: Vec<perro_nodes::skeleton_2d::Bone2D>,
}

pub(super) struct AsyncSkeleton3DLoadResult {
    pub(super) source: String,
    pub(super) bones: Vec<perro_nodes::skeleton_3d::Bone3D>,
}

fn split_source_fragment_for_material(source: &str) -> &str {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return source;
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return source;
    }
    if selector.contains('[') && selector.ends_with(']') {
        return path;
    }
    source
}

fn load_animation_clip_from_source(source: &str) -> Arc<perro_animation::AnimationClip> {
    if source.ends_with(".panim")
        && let Ok(bytes) = perro_io::load_asset(source)
        && let Ok(text) = std::str::from_utf8(&bytes)
        && let Ok(clip) = perro_animation::parse_panim(text)
    {
        return Arc::new(clip);
    }
    Arc::new(perro_animation::AnimationClip {
        name: Cow::Borrowed("Animation"),
        fps: 60.0,
        total_frames: 1,
        objects: Cow::Borrowed(&[]),
        object_tracks: Cow::Borrowed(&[]),
        frame_events: Cow::Borrowed(&[]),
    })
}

fn load_animation_tree_from_source(source: &str) -> Arc<perro_animation::AnimationTreeAsset> {
    if source.ends_with(".panimtree")
        && let Ok(bytes) = perro_io::load_asset(source)
        && let Ok(text) = std::str::from_utf8(&bytes)
        && let Ok(tree) = perro_animation::parse_panimtree(text)
    {
        return Arc::new(tree);
    }
    Arc::new(perro_animation::AnimationTreeAsset {
        name: Cow::Borrowed("AnimationTree"),
        slots: Cow::Borrowed(&[]),
        nodes: Cow::Borrowed(&[]),
        output: Cow::Borrowed(""),
    })
}

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

/// Listener pose + options behind ONE mutex: writers set both together, and
/// audio solve reads them as a pair — a single lock keeps the pair coherent
/// (no torn read between separate listener/options locks) and halves the
/// per-solve lock traffic.
#[derive(Clone, Debug, Default)]
pub(crate) struct AudioListenerSlot<L> {
    pub(crate) listener: Option<L>,
    pub(crate) options: perro_structs::AudioListenerOptions,
}

pub struct RuntimeResourceApi {
    pub(super) state: Mutex<RuntimeResourceState>,
    pub(super) localization: std::sync::RwLock<RuntimeLocalizationState>,
    pub(crate) bark: Mutex<Option<AudioController>>,
    pub(crate) mic: Mutex<MicRecorder>,
    pub(crate) spatial_audio_queue: Mutex<Vec<QueuedSpatialAudio>>,
    pub(crate) spatial_midi_queue: Mutex<Vec<QueuedSpatialMidi>>,
    pub(crate) next_spatial_midi_id: std::sync::atomic::AtomicU64,
    pub(crate) audio_listener_2d: Mutex<AudioListenerSlot<perro_pawdio::AudioListener2D>>,
    pub(crate) audio_listener_3d: Mutex<AudioListenerSlot<perro_pawdio::AudioListener3D>>,
    pub(super) static_material_lookup: Option<StaticMaterialLookup>,
    pub(super) static_skeleton_lookup: Option<StaticSkeletonLookup>,
    pub(super) static_animation_lookup: Option<StaticAnimationLookup>,
    pub(super) static_animation_tree_lookup: Option<StaticAnimationTreeLookup>,
    pub(super) static_localization_lookup: Option<StaticLocalizationLookup>,
    pub(super) static_csv_lookup: Option<StaticCsvLookup>,
    pub(super) csv_cache: Mutex<HashMap<u64, &'static perro_csv::Csv>>,
    pub(super) skeleton_bones_2d_cache: Mutex<HashMap<u64, Vec<perro_nodes::skeleton_2d::Bone2D>>>,
    pub(super) skeleton_bones_3d_cache: Mutex<HashMap<u64, Vec<perro_nodes::skeleton_3d::Bone3D>>>,
    pub(super) video_clip_cache: Mutex<HashMap<u64, Arc<RuntimeVideoClip>>>,
    pub(super) video_node_state: Mutex<HashMap<NodeID, RuntimeVideoNode>>,
    pub(super) skeleton_bones_2d_pending: Mutex<std::collections::HashSet<u64>>,
    pub(super) skeleton_bones_3d_pending: Mutex<std::collections::HashSet<u64>>,
    pub(super) skeleton_2d_load_tx: mpsc::Sender<AsyncSkeleton2DLoadResult>,
    pub(super) skeleton_2d_load_rx: Mutex<mpsc::Receiver<AsyncSkeleton2DLoadResult>>,
    pub(super) skeleton_3d_load_tx: mpsc::Sender<AsyncSkeleton3DLoadResult>,
    pub(super) skeleton_3d_load_rx: Mutex<mpsc::Receiver<AsyncSkeleton3DLoadResult>>,
    // width in the high 32 bits, height in the low 32: one relaxed atomic
    // instead of a mutex for a value written once per resize.
    pub(super) viewport_size: std::sync::atomic::AtomicU64,
    #[cfg_attr(any(test, target_arch = "wasm32"), allow(dead_code))]
    pub(crate) webcam_frame_tx: mpsc::SyncSender<WebcamFrameMessage>,
    pub(crate) webcam_frame_rx: Mutex<mpsc::Receiver<WebcamFrameMessage>>,
    pub(crate) webcam_error_tx: mpsc::Sender<WebcamErrorMessage>,
    pub(crate) webcam_error_rx: Mutex<mpsc::Receiver<WebcamErrorMessage>>,
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub(crate) webcam_stop_by_id: Mutex<HashMap<WebcamID, Arc<AtomicBool>>>,
    // Cache of the auto-picked default device slot so empty-device configs do
    // not re-query the OS camera backend on every render extraction.
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) webcam_default_slot: Mutex<Option<String>>,
    #[cfg(not(target_arch = "wasm32"))]
    material_load_tx: mpsc::Sender<AsyncMaterialLoadResult>,
    #[cfg(not(target_arch = "wasm32"))]
    material_load_rx: Mutex<mpsc::Receiver<AsyncMaterialLoadResult>>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    animation_load_tx: mpsc::Sender<AsyncAnimationLoadResult>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    animation_load_rx: Mutex<mpsc::Receiver<AsyncAnimationLoadResult>>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    animation_tree_load_tx: mpsc::Sender<AsyncAnimationTreeLoadResult>,
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    animation_tree_load_rx: Mutex<mpsc::Receiver<AsyncAnimationTreeLoadResult>>,
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
        #[cfg(not(target_arch = "wasm32"))]
        let (material_load_tx, material_load_rx) = mpsc::channel();
        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
        let (animation_load_tx, animation_load_rx) = mpsc::channel();
        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
        let (animation_tree_load_tx, animation_tree_load_rx) = mpsc::channel();
        let (skeleton_2d_load_tx, skeleton_2d_load_rx) = mpsc::channel();
        let (skeleton_3d_load_tx, skeleton_3d_load_rx) = mpsc::channel();
        // bounded latest-only handoff: capture thread drops stale frames when the
        // runtime lags instead of growing memory + latency unboundedly. small cap
        // holds a couple frames across the few live webcams; receiver coalesces.
        let (webcam_frame_tx, webcam_frame_rx) = mpsc::sync_channel(WEBCAM_FRAME_CHANNEL_CAP);
        let (webcam_error_tx, webcam_error_rx) = mpsc::channel();
        let api = Arc::new(Self {
            state: Mutex::new(RuntimeResourceState::new()),
            localization: std::sync::RwLock::new(RuntimeLocalizationState::new(
                localization_config.as_ref(),
            )),
            bark: Mutex::new(AudioController::new(static_audio_lookup).ok()),
            mic: Mutex::new(MicRecorder::new()),
            spatial_audio_queue: Mutex::new(Vec::new()),
            spatial_midi_queue: Mutex::new(Vec::new()),
            next_spatial_midi_id: std::sync::atomic::AtomicU64::new(1),
            audio_listener_2d: Mutex::new(AudioListenerSlot::default()),
            audio_listener_3d: Mutex::new(AudioListenerSlot::default()),
            static_material_lookup,
            static_skeleton_lookup,
            static_animation_lookup,
            static_animation_tree_lookup,
            static_localization_lookup,
            static_csv_lookup,
            csv_cache: Mutex::new(HashMap::new()),
            skeleton_bones_2d_cache: Mutex::new(HashMap::new()),
            skeleton_bones_3d_cache: Mutex::new(HashMap::new()),
            video_clip_cache: Mutex::new(HashMap::new()),
            video_node_state: Mutex::new(HashMap::new()),
            skeleton_bones_2d_pending: Mutex::new(std::collections::HashSet::new()),
            skeleton_bones_3d_pending: Mutex::new(std::collections::HashSet::new()),
            skeleton_2d_load_tx,
            skeleton_2d_load_rx: Mutex::new(skeleton_2d_load_rx),
            skeleton_3d_load_tx,
            skeleton_3d_load_rx: Mutex::new(skeleton_3d_load_rx),
            viewport_size: std::sync::atomic::AtomicU64::new((1u64 << 32) | 1),
            webcam_frame_tx,
            webcam_frame_rx: Mutex::new(webcam_frame_rx),
            webcam_error_tx,
            webcam_error_rx: Mutex::new(webcam_error_rx),
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
            webcam_stop_by_id: Mutex::new(HashMap::new()),
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
            webcam_default_slot: Mutex::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            material_load_tx,
            #[cfg(not(target_arch = "wasm32"))]
            material_load_rx: Mutex::new(material_load_rx),
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            animation_load_tx,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            animation_load_rx: Mutex::new(animation_load_rx),
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            animation_tree_load_tx,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            animation_tree_load_rx: Mutex::new(animation_tree_load_rx),
        });
        api.initialize_localization();
        api
    }

    pub(crate) fn set_viewport_size(&self, width: u32, height: u32) {
        let packed = ((width.max(1) as u64) << 32) | height.max(1) as u64;
        self.viewport_size
            .store(packed, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn viewport_size(&self) -> (u32, u32) {
        let packed = self
            .viewport_size
            .load(std::sync::atomic::Ordering::Relaxed);
        ((packed >> 32) as u32, packed as u32)
    }

    pub(crate) fn set_audio_listener_2d(
        &self,
        position: [f32; 2],
        rotation_radians: f32,
        options: perro_structs::AudioListenerOptions,
    ) {
        let mut slot = self
            .audio_listener_2d
            .lock()
            .expect("resource api audio 2d listener mutex poisoned");
        slot.listener = Some(perro_pawdio::AudioListener2D {
            position,
            rotation_radians,
        });
        slot.options = options;
    }

    pub(crate) fn set_audio_listener_3d(
        &self,
        position: [f32; 3],
        rotation: [f32; 4],
        options: perro_structs::AudioListenerOptions,
    ) {
        let mut slot = self
            .audio_listener_3d
            .lock()
            .expect("resource api audio 3d listener mutex poisoned");
        slot.listener = Some(perro_pawdio::AudioListener3D { position, rotation });
        slot.options = options;
    }

    pub(crate) fn drain_commands(&self, out: &mut Vec<RenderCommand>) {
        self.poll_async_material_loads();
        self.poll_async_animation_loads();
        self.poll_async_animation_tree_loads();
        self.poll_webcam_messages();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        out.append(&mut state.queued_commands);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn queue_material_source_load(&self, id: perro_ids::MaterialID, source: String) {
        let tx = self.material_load_tx.clone();
        rayon::spawn(move || {
            let normalized = if source.contains('\\') {
                std::borrow::Cow::Owned(source.replace('\\', "/"))
            } else {
                std::borrow::Cow::Borrowed(source.as_str())
            };
            let path = split_source_fragment_for_material(source.as_str());
            let normalized_path = split_source_fragment_for_material(normalized.as_ref());
            let material = crate::material_schema::load_from_source(source.as_str())
                .or_else(|| crate::material_schema::load_from_source(path))
                .or_else(|| crate::material_schema::load_from_source(normalized.as_ref()))
                .or_else(|| crate::material_schema::load_from_source(normalized_path));
            let _ = tx.send(AsyncMaterialLoadResult { id, material });
        });
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn queue_material_source_load(&self, id: perro_ids::MaterialID, source: String) {
        let normalized = if source.contains('\\') {
            std::borrow::Cow::Owned(source.replace('\\', "/"))
        } else {
            std::borrow::Cow::Borrowed(source.as_str())
        };
        let path = split_source_fragment_for_material(source.as_str());
        let normalized_path = split_source_fragment_for_material(normalized.as_ref());
        if let Some(material) = crate::material_schema::load_from_source(source.as_str())
            .or_else(|| crate::material_schema::load_from_source(path))
            .or_else(|| crate::material_schema::load_from_source(normalized.as_ref()))
            .or_else(|| crate::material_schema::load_from_source(normalized_path))
        {
            let mut state = self.state.lock().expect("resource api mutex poisoned");
            state.material_load_pending_by_id.remove(&id);
            state.material_data_by_id.insert(id, material.clone());
            state.material_write_pending_by_id.insert(id);
            state.queued_commands.push(RenderCommand::Resource(
                perro_render_bridge::ResourceCommand::WriteMaterialData { id, material },
            ));
        } else {
            let mut state = self.state.lock().expect("resource api mutex poisoned");
            state.material_load_pending_by_id.remove(&id);
            if !state
                .material_pending_id_by_request
                .values()
                .any(|pending| *pending == id)
            {
                state.material_loaded_by_id.insert(id);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn poll_async_material_loads(&self) {
        let Ok(rx) = self.material_load_rx.lock() else {
            return;
        };
        let mut loaded = Vec::new();
        while let Ok(result) = rx.try_recv() {
            loaded.push(result);
        }
        drop(rx);
        if loaded.is_empty() {
            return;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        for result in loaded {
            if asset_ready_log_enabled() {
                eprintln!(
                    "[perro][asset-ready] material source task done id={:?} ok={}",
                    result.id,
                    result.material.is_some()
                );
            }
            state.material_load_pending_by_id.remove(&result.id);
            if let Some(material) = result.material {
                state
                    .material_data_by_id
                    .insert(result.id, material.clone());
                state.material_write_pending_by_id.insert(result.id);
                state.queued_commands.push(RenderCommand::Resource(
                    perro_render_bridge::ResourceCommand::WriteMaterialData {
                        id: result.id,
                        material,
                    },
                ));
                if !state
                    .material_pending_id_by_request
                    .values()
                    .any(|pending| *pending == result.id)
                {
                    state.material_loaded_by_id.insert(result.id);
                }
            } else if !state
                .material_pending_id_by_request
                .values()
                .any(|pending| *pending == result.id)
            {
                state.material_loaded_by_id.insert(result.id);
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn poll_async_material_loads(&self) {}

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(crate) fn queue_animation_source_load(&self, id: perro_ids::AnimationID, source: String) {
        let tx = self.animation_load_tx.clone();
        rayon::spawn(move || {
            let clip = load_animation_clip_from_source(source.as_str());
            let _ = tx.send(AsyncAnimationLoadResult { id, clip });
        });
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn queue_animation_source_load(&self, id: perro_ids::AnimationID, source: String) {
        let clip = load_animation_clip_from_source(source.as_str());
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_data_by_id.insert(id, clip);
        state.animation_loaded_by_id.insert(id);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(crate) fn poll_async_animation_loads(&self) {
        let Ok(rx) = self.animation_load_rx.lock() else {
            return;
        };
        let mut loaded = Vec::new();
        while let Ok(result) = rx.try_recv() {
            loaded.push(result);
        }
        drop(rx);
        if loaded.is_empty() {
            return;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        for result in loaded {
            state.animation_data_by_id.insert(result.id, result.clip);
            state.animation_loaded_by_id.insert(result.id);
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn poll_async_animation_loads(&self) {}

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(crate) fn queue_animation_tree_source_load(
        &self,
        id: perro_ids::AnimationTreeID,
        source: String,
    ) {
        let tx = self.animation_tree_load_tx.clone();
        rayon::spawn(move || {
            let tree = load_animation_tree_from_source(source.as_str());
            let _ = tx.send(AsyncAnimationTreeLoadResult { id, tree });
        });
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn queue_animation_tree_source_load(
        &self,
        id: perro_ids::AnimationTreeID,
        source: String,
    ) {
        let tree = load_animation_tree_from_source(source.as_str());
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_tree_data_by_id.insert(id, tree);
        state.animation_tree_loaded_by_id.insert(id);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(crate) fn poll_async_animation_tree_loads(&self) {
        let Ok(rx) = self.animation_tree_load_rx.lock() else {
            return;
        };
        let mut loaded = Vec::new();
        while let Ok(result) = rx.try_recv() {
            loaded.push(result);
        }
        drop(rx);
        if loaded.is_empty() {
            return;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        for result in loaded {
            state
                .animation_tree_data_by_id
                .insert(result.id, result.tree);
            state.animation_tree_loaded_by_id.insert(result.id);
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn poll_async_animation_tree_loads(&self) {}

    // Per-domain event logic lives with its domain (texture.rs / mesh.rs /
    // material.rs as RuntimeResourceState methods); this stays the single
    // lock + dispatch point so every event applies under one state guard.
    pub(crate) fn apply_render_event(&self, event: &RenderEvent) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        match event {
            RenderEvent::HdrStatusChanged(status) => {
                state.hdr_status = *status;
            }
            RenderEvent::TextureCreated { request, id } => {
                state.apply_texture_created(*request, *id);
            }
            RenderEvent::TextureLoaded { id } => {
                state.texture_loaded_by_id.insert(*id);
            }
            // texels changed on an already-loaded stream texture; load state
            // + pending resolution unchanged.
            RenderEvent::TextureTexelsUpdated { .. } => {}
            RenderEvent::MaterialLoaded { id } => {
                state.apply_material_loaded(*id);
            }
            RenderEvent::MeshCreated { request, id, mesh } => {
                state.apply_mesh_created(*request, *id, mesh.as_ref());
            }
            RenderEvent::MaterialCreated { request, id } => {
                state.apply_material_created(*request, *id);
            }
            RenderEvent::TextureDropped { id } => {
                state.apply_texture_dropped(*id);
            }
            RenderEvent::MeshDropped { id } => {
                state.apply_mesh_dropped(*id);
            }
            RenderEvent::MaterialDropped { id } => {
                state.apply_material_dropped(*id);
            }
            RenderEvent::Failed { request, .. } => {
                state.apply_texture_failed(*request);
                state.apply_mesh_failed(*request);
                state.apply_material_failed(*request);
            }
            RenderEvent::WaterSamples { .. } | RenderEvent::WaterBodySamples { .. } => {}
        }
    }
}
