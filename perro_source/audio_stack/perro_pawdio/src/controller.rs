use crossbeam_channel::{self, Sender, TrySendError};
use perro_ids::AudioBusID;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::internal::{AudioCommand, OwnedAudioPlaybackRequest};
use crate::internal::{OwnedMidiFileRequest, OwnedMidiNoteRequest};
use crate::mic::MicClip;
use crate::midi::{MidiFileRequest, MidiNoteHandle, MidiNoteRequest};
use crate::player::BarkPlayer;
use crate::types::{AudioPlaybackRequest, SpatialAudioParams};

const AUDIO_DISABLED_ENV: &str = "PERRO_AUDIO_DISABLED";

/// Error returned when an audio command cannot be enqueued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioEnqueueError {
    /// The bounded audio command queue has no free slot.
    Full,
    /// The audio worker has stopped and no longer receives commands.
    Disconnected,
}

impl fmt::Display for AudioEnqueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => formatter.write_str("audio command queue is full"),
            Self::Disconnected => formatter.write_str("audio command queue is disconnected"),
        }
    }
}

impl std::error::Error for AudioEnqueueError {}

/// Result of enqueueing an audio command.
pub type AudioEnqueueResult<T = ()> = Result<T, AudioEnqueueError>;

pub struct AudioController {
    tx: Sender<AudioCommand>,
    next_playback_id: Arc<AtomicU64>,
    source_pool: Mutex<HashMap<u64, Arc<str>>>,
    loaded: Arc<Mutex<AudioLoadedState>>,
}

#[derive(Default)]
struct AudioLoadedState {
    sources: HashSet<Arc<str>>,
    soundfonts: HashSet<perro_ids::SoundFontID>,
}

#[derive(Clone)]
pub struct AudioSourceHandle {
    source: Arc<str>,
}

impl AudioSourceHandle {
    pub fn as_str(&self) -> &str {
        &self.source
    }
}

impl AudioController {
    const COMMAND_QUEUE_CAPACITY: usize = 4096;

    pub fn new(static_audio_lookup: Option<fn(u64) -> &'static [u8]>) -> Result<Self, String> {
        if audio_disabled_by_env() {
            return Err(format!("audio disabled by {AUDIO_DISABLED_ENV}"));
        }

        let (tx, rx) = crossbeam_channel::bounded::<AudioCommand>(Self::COMMAND_QUEUE_CAPACITY);
        let next_playback_id = Arc::new(AtomicU64::new(1));
        let loaded = Arc::new(Mutex::new(AudioLoadedState::default()));
        let loaded_for_thread = Arc::clone(&loaded);
        std::thread::Builder::new()
            .name("perro_pawdio_audio".to_string())
            .spawn(move || {
                let Ok(player) = BarkPlayer::new(static_audio_lookup) else {
                    return;
                };

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        AudioCommand::Load { source, reserved } => {
                            if player.load_source(&source, reserved).is_ok() {
                                if let Ok(mut loaded) = loaded_for_thread.lock() {
                                    loaded.sources.insert(Arc::clone(&source));
                                }
                            } else if let Ok(mut loaded) = loaded_for_thread.lock() {
                                loaded.sources.remove(&source);
                            }
                        }
                        AudioCommand::LoadBytes {
                            source,
                            bytes,
                            reserved,
                        } => {
                            if player.load_source_bytes(&source, bytes, reserved).is_ok() {
                                if let Ok(mut loaded) = loaded_for_thread.lock() {
                                    loaded.sources.insert(Arc::clone(&source));
                                }
                            } else if let Ok(mut loaded) = loaded_for_thread.lock() {
                                loaded.sources.remove(&source);
                            }
                        }
                        AudioCommand::DropAsset { source } => {
                            let _ = player.drop_source_asset(&source);
                            if let Ok(mut loaded) = loaded_for_thread.lock() {
                                loaded.sources.remove(&source);
                            }
                        }
                        AudioCommand::Play { request } => {
                            let _ = player.play_source(AudioPlaybackRequest {
                                id: request.id,
                                source: request.source.as_ref(),
                                bus_id: request.bus_id,
                                looped: request.looped,
                                volume: request.volume,
                                speed: request.speed,
                                pan: request.pan,
                                low_pass: request.low_pass,
                                reverb_send: request.reverb_send,
                                echo: request.echo,
                                reflection: request.reflection,
                                occlusion: request.occlusion,
                                eq: request.eq,
                                compression: request.compression,
                                from_start: request.from_start,
                                from_end: request.from_end,
                            });
                        }
                        AudioCommand::PlayClip {
                            source,
                            clip,
                            bus_id,
                            volume,
                            pan,
                        } => {
                            let _ = player.play_clip(&source, clip, bus_id, volume, pan);
                        }
                        AudioCommand::Stop { source } => {
                            let _ = player.stop_source(&source);
                        }
                        AudioCommand::StopMatch { request } => {
                            let _ = player.stop_match(AudioPlaybackRequest {
                                id: request.id,
                                source: request.source.as_ref(),
                                bus_id: request.bus_id,
                                looped: request.looped,
                                volume: request.volume,
                                speed: request.speed,
                                pan: request.pan,
                                low_pass: request.low_pass,
                                reverb_send: request.reverb_send,
                                echo: request.echo,
                                reflection: request.reflection,
                                occlusion: request.occlusion,
                                eq: request.eq,
                                compression: request.compression,
                                from_start: request.from_start,
                                from_end: request.from_end,
                            });
                        }
                        AudioCommand::StopPlayback { id } => {
                            let _ = player.stop_playback(id);
                        }
                        AudioCommand::UpdateSpatial { id, params } => {
                            let _ = player.update_spatial(id, params);
                        }
                        AudioCommand::StopAll => player.stop_all(),
                        AudioCommand::SetMasterVolume { volume } => {
                            player.set_master_volume(volume)
                        }
                        AudioCommand::SetBusVolume { bus_id, volume } => {
                            player.set_bus_volume(bus_id, volume)
                        }
                        AudioCommand::SetBusSpeed { bus_id, speed } => {
                            player.set_bus_speed(bus_id, speed)
                        }
                        AudioCommand::PauseBus { bus_id } => player.pause_bus(bus_id),
                        AudioCommand::ResumeBus { bus_id } => player.resume_bus(bus_id),
                        AudioCommand::StopBus { bus_id } => {
                            let _ = player.stop_bus(bus_id);
                        }
                        AudioCommand::SourceLength { source, reply } => {
                            let _ = reply.send(player.source_length_seconds(&source));
                        }
                        AudioCommand::LoadSoundFont { id, source } => {
                            if player.load_soundfont(id, &source).is_ok() {
                                if let Ok(mut loaded) = loaded_for_thread.lock() {
                                    loaded.soundfonts.insert(id);
                                }
                            } else if let Ok(mut loaded) = loaded_for_thread.lock() {
                                loaded.soundfonts.remove(&id);
                            }
                        }
                        AudioCommand::LoadSoundFontBytes { id, source, bytes } => {
                            if player.load_soundfont_bytes(id, &source, bytes).is_ok() {
                                if let Ok(mut loaded) = loaded_for_thread.lock() {
                                    loaded.soundfonts.insert(id);
                                }
                            } else if let Ok(mut loaded) = loaded_for_thread.lock() {
                                loaded.soundfonts.remove(&id);
                            }
                        }
                        AudioCommand::LoadMidiFile { source } => {
                            let _ = player.load_midi_file(&source);
                        }
                        AudioCommand::MidiNote { request } => {
                            let _ = player.play_midi_note(request.as_request());
                        }
                        AudioCommand::MidiNoteSpatial { request } => {
                            let _ = player.play_midi_note_spatial(request.as_request());
                        }
                        AudioCommand::MidiNotes { requests } => {
                            for request in requests {
                                let _ = player.play_midi_note(request.as_request());
                            }
                        }
                        AudioCommand::MidiFile { request } => {
                            let _ = player.play_midi_file(request.as_request());
                        }
                        AudioCommand::MidiRelease { id } => {
                            let _ = player.release_midi(id);
                        }
                    }
                }
            })
            .map_err(|err| format!("failed to spawn audio thread: {err}"))?;
        Ok(Self {
            tx,
            next_playback_id,
            source_pool: Mutex::new(HashMap::new()),
            loaded,
        })
    }

    #[cfg(test)]
    fn from_test_sender(tx: Sender<AudioCommand>) -> Self {
        Self {
            tx,
            next_playback_id: Arc::new(AtomicU64::new(1)),
            source_pool: Mutex::new(HashMap::new()),
            loaded: Arc::new(Mutex::new(AudioLoadedState::default())),
        }
    }

    fn enqueue(&self, command: AudioCommand) -> AudioEnqueueResult {
        self.tx.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => AudioEnqueueError::Full,
            TrySendError::Disconnected(_) => AudioEnqueueError::Disconnected,
        })
    }

    fn intern_source(&self, source: &str) -> Arc<str> {
        let hash = perro_ids::string_to_u64(source);
        let Ok(mut pool) = self.source_pool.lock() else {
            return Arc::from(source);
        };
        if let Some(existing) = pool.get(&hash)
            && existing.as_ref() == source
        {
            return existing.clone();
        }
        let interned: Arc<str> = Arc::from(source);
        pool.insert(hash, interned.clone());
        interned
    }

    pub fn source_handle(&self, source: &str) -> AudioSourceHandle {
        AudioSourceHandle {
            source: self.intern_source(source),
        }
    }

    pub fn play_source(&self, request: AudioPlaybackRequest<'_>) -> bool {
        self.enqueue_play_source(request).is_ok()
    }

    /// Enqueue source playback; success does not mean playback has started.
    pub fn enqueue_play_source(&self, request: AudioPlaybackRequest<'_>) -> AudioEnqueueResult {
        let source = self.intern_source(request.source);
        self.enqueue_play_source_arc(request, source)
    }

    pub fn play_clip(
        &self,
        source: &str,
        clip: MicClip,
        bus_id: Option<AudioBusID>,
        volume: f32,
        pan: crate::types::AudioPan,
    ) -> bool {
        self.enqueue_play_clip(source, clip, bus_id, volume, pan)
            .is_ok()
    }

    /// Enqueue clip playback; success does not mean playback has started.
    pub fn enqueue_play_clip(
        &self,
        source: &str,
        clip: MicClip,
        bus_id: Option<AudioBusID>,
        volume: f32,
        pan: crate::types::AudioPan,
    ) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::PlayClip {
            source,
            clip,
            bus_id,
            volume,
            pan,
        })
    }

    pub fn play_source_handle(
        &self,
        handle: &AudioSourceHandle,
        request: AudioPlaybackRequest<'_>,
    ) -> bool {
        self.enqueue_play_source_handle(handle, request).is_ok()
    }

    /// Enqueue source-handle playback; success does not mean playback has started.
    pub fn enqueue_play_source_handle(
        &self,
        handle: &AudioSourceHandle,
        request: AudioPlaybackRequest<'_>,
    ) -> AudioEnqueueResult {
        self.enqueue_play_source_arc(request, handle.source.clone())
    }

    fn enqueue_play_source_arc(
        &self,
        request: AudioPlaybackRequest<'_>,
        source: Arc<str>,
    ) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::Play {
            request: OwnedAudioPlaybackRequest::from_request_with_source(request, source),
        })
    }

    pub fn play_spatial_source(&self, request: AudioPlaybackRequest<'_>) -> Option<u64> {
        self.enqueue_play_spatial_source(request).ok()
    }

    /// Enqueue spatial playback and return its id; success does not mean playback has started.
    pub fn enqueue_play_spatial_source(
        &self,
        mut request: AudioPlaybackRequest<'_>,
    ) -> AudioEnqueueResult<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        let source = self.intern_source(request.source);
        self.enqueue_play_spatial_source_arc(request, source, id)
    }

    pub fn play_spatial_source_handle(
        &self,
        handle: &AudioSourceHandle,
        request: AudioPlaybackRequest<'_>,
    ) -> Option<u64> {
        self.enqueue_play_spatial_source_handle(handle, request)
            .ok()
    }

    /// Enqueue spatial handle playback and return its id; success does not mean playback has started.
    pub fn enqueue_play_spatial_source_handle(
        &self,
        handle: &AudioSourceHandle,
        mut request: AudioPlaybackRequest<'_>,
    ) -> AudioEnqueueResult<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.enqueue_play_spatial_source_arc(request, handle.source.clone(), id)
    }

    fn enqueue_play_spatial_source_arc(
        &self,
        request: AudioPlaybackRequest<'_>,
        source: Arc<str>,
        id: u64,
    ) -> AudioEnqueueResult<u64> {
        self.enqueue(AudioCommand::Play {
            request: OwnedAudioPlaybackRequest::from_request_with_source(request, source),
        })
        .map(|()| id)
    }

    pub fn update_spatial(&self, id: u64, params: SpatialAudioParams) -> bool {
        self.enqueue_update_spatial(id, params).is_ok()
    }

    /// Enqueue a spatial update; success does not mean the update has been applied.
    pub fn enqueue_update_spatial(
        &self,
        id: u64,
        params: SpatialAudioParams,
    ) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::UpdateSpatial { id, params })
    }

    pub fn stop_playback(&self, id: u64) -> bool {
        self.enqueue_stop_playback(id).is_ok()
    }

    /// Enqueue a playback stop; success does not mean playback has stopped.
    pub fn enqueue_stop_playback(&self, id: u64) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::StopPlayback { id })
    }

    pub fn load_source(&self, source: &str) -> bool {
        self.enqueue_load_source(source).is_ok()
    }

    /// Enqueue a source load; success does not mean decoding or loading has completed.
    pub fn enqueue_load_source(&self, source: &str) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::Load {
            source,
            reserved: false,
        })
    }

    pub fn load_source_bytes(&self, source: &str, bytes: Arc<[u8]>) -> bool {
        self.enqueue_load_source_bytes(source, bytes).is_ok()
    }

    /// Enqueue a byte source load; success does not mean decoding or loading has completed.
    pub fn enqueue_load_source_bytes(&self, source: &str, bytes: Arc<[u8]>) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::LoadBytes {
            source,
            bytes,
            reserved: false,
        })
    }

    pub fn is_source_loaded(&self, source: &str) -> bool {
        let source = self.intern_source(source);
        self.loaded
            .lock()
            .map(|loaded| loaded.sources.contains(&source))
            .unwrap_or(false)
    }

    pub fn reserve_source(&self, source: &str) -> bool {
        self.enqueue_reserve_source(source).is_ok()
    }

    /// Enqueue a reserved source load; success does not mean loading has completed.
    pub fn enqueue_reserve_source(&self, source: &str) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::Load {
            source,
            reserved: true,
        })
    }

    pub fn reserve_source_bytes(&self, source: &str, bytes: Arc<[u8]>) -> bool {
        self.enqueue_reserve_source_bytes(source, bytes).is_ok()
    }

    /// Enqueue a reserved byte source load; success does not mean loading has completed.
    pub fn enqueue_reserve_source_bytes(
        &self,
        source: &str,
        bytes: Arc<[u8]>,
    ) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::LoadBytes {
            source,
            bytes,
            reserved: true,
        })
    }

    pub fn drop_source(&self, source: &str) -> bool {
        self.enqueue_drop_source(source).is_ok()
    }

    /// Enqueue a source drop; success does not mean the asset has been dropped.
    pub fn enqueue_drop_source(&self, source: &str) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::DropAsset { source })
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let source = self.intern_source(source);
        let (reply_tx, reply_rx) = crossbeam_channel::bounded::<Option<f32>>(1);
        if self
            .enqueue(AudioCommand::SourceLength {
                source,
                reply: reply_tx,
            })
            .is_err()
        {
            return None;
        }
        reply_rx.recv().ok().flatten()
    }

    pub fn stop_source(&self, source: &str) -> bool {
        self.enqueue_stop_source(source).is_ok()
    }

    /// Enqueue a source stop; success does not mean playback has stopped.
    pub fn enqueue_stop_source(&self, source: &str) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::Stop { source })
    }

    pub fn stop_match(&self, request: AudioPlaybackRequest<'_>) -> bool {
        self.enqueue_stop_match(request).is_ok()
    }

    /// Enqueue a matching-playback stop; success does not mean playback has stopped.
    pub fn enqueue_stop_match(&self, request: AudioPlaybackRequest<'_>) -> AudioEnqueueResult {
        let source = self.intern_source(request.source);
        self.enqueue(AudioCommand::StopMatch {
            request: OwnedAudioPlaybackRequest::from_request_with_source(request, source),
        })
    }

    pub fn stop_all(&self) -> bool {
        self.enqueue_stop_all().is_ok()
    }

    /// Enqueue a global stop; success does not mean playback has stopped.
    pub fn enqueue_stop_all(&self) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::StopAll)
    }

    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.enqueue_set_master_volume(volume).is_ok()
    }

    /// Enqueue a master-volume change; success does not mean it has been applied.
    pub fn enqueue_set_master_volume(&self, volume: f32) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::SetMasterVolume { volume })
    }

    pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool {
        self.enqueue_set_bus_volume(bus_id, volume).is_ok()
    }

    /// Enqueue a bus-volume change; success does not mean it has been applied.
    pub fn enqueue_set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::SetBusVolume { bus_id, volume })
    }

    pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool {
        self.enqueue_set_bus_speed(bus_id, speed).is_ok()
    }

    /// Enqueue a bus-speed change; success does not mean it has been applied.
    pub fn enqueue_set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::SetBusSpeed { bus_id, speed })
    }

    pub fn pause_bus(&self, bus_id: AudioBusID) -> bool {
        self.enqueue_pause_bus(bus_id).is_ok()
    }

    /// Enqueue a bus pause; success does not mean the bus has paused.
    pub fn enqueue_pause_bus(&self, bus_id: AudioBusID) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::PauseBus { bus_id })
    }

    pub fn resume_bus(&self, bus_id: AudioBusID) -> bool {
        self.enqueue_resume_bus(bus_id).is_ok()
    }

    /// Enqueue a bus resume; success does not mean the bus has resumed.
    pub fn enqueue_resume_bus(&self, bus_id: AudioBusID) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::ResumeBus { bus_id })
    }

    pub fn stop_bus(&self, bus_id: AudioBusID) -> bool {
        self.enqueue_stop_bus(bus_id).is_ok()
    }

    /// Enqueue a bus stop; success does not mean playback has stopped.
    pub fn enqueue_stop_bus(&self, bus_id: AudioBusID) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::StopBus { bus_id })
    }

    pub fn load_soundfont(&self, source: &str) -> perro_ids::SoundFontID {
        let id = perro_ids::SoundFontID::from_string(source);
        let _ = self.enqueue_load_soundfont_with_id(id, source);
        id
    }

    /// Enqueue a soundfont load; success does not mean parsing or loading has completed.
    pub fn enqueue_load_soundfont(
        &self,
        source: &str,
    ) -> AudioEnqueueResult<perro_ids::SoundFontID> {
        let id = perro_ids::SoundFontID::from_string(source);
        self.enqueue_load_soundfont_with_id(id, source)
    }

    pub fn load_soundfont_with_id(
        &self,
        id: perro_ids::SoundFontID,
        source: &str,
    ) -> perro_ids::SoundFontID {
        let _ = self.enqueue_load_soundfont_with_id(id, source);
        id
    }

    /// Enqueue an identified soundfont load; success does not mean loading has completed.
    pub fn enqueue_load_soundfont_with_id(
        &self,
        id: perro_ids::SoundFontID,
        source: &str,
    ) -> AudioEnqueueResult<perro_ids::SoundFontID> {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::LoadSoundFont { id, source })
            .map(|()| id)
    }

    pub fn load_soundfont_bytes_with_id(
        &self,
        id: perro_ids::SoundFontID,
        source: &str,
        bytes: Arc<[u8]>,
    ) -> perro_ids::SoundFontID {
        let _ = self.enqueue_load_soundfont_bytes_with_id(id, source, bytes);
        id
    }

    /// Enqueue an identified byte soundfont load; success does not mean loading has completed.
    pub fn enqueue_load_soundfont_bytes_with_id(
        &self,
        id: perro_ids::SoundFontID,
        source: &str,
        bytes: Arc<[u8]>,
    ) -> AudioEnqueueResult<perro_ids::SoundFontID> {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::LoadSoundFontBytes { id, source, bytes })
            .map(|()| id)
    }

    pub fn is_soundfont_loaded(&self, id: perro_ids::SoundFontID) -> bool {
        self.loaded
            .lock()
            .map(|loaded| loaded.soundfonts.contains(&id))
            .unwrap_or(false)
    }

    pub fn load_midi_file(&self, source: &str) -> bool {
        self.enqueue_load_midi_file(source).is_ok()
    }

    /// Enqueue a MIDI-file load; success does not mean parsing or loading has completed.
    pub fn enqueue_load_midi_file(&self, source: &str) -> AudioEnqueueResult {
        let source = self.intern_source(source);
        self.enqueue(AudioCommand::LoadMidiFile { source })
    }

    pub fn play_midi_note(&self, request: MidiNoteRequest) -> bool {
        self.enqueue_play_midi_note(request).is_ok()
    }

    /// Enqueue MIDI-note playback; success does not mean playback has started.
    pub fn enqueue_play_midi_note(&self, request: MidiNoteRequest) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::MidiNote {
            request: OwnedMidiNoteRequest::from_request(request),
        })
    }

    pub fn start_midi_note(&self, request: MidiNoteRequest) -> Option<MidiNoteHandle> {
        self.enqueue_start_midi_note(request).ok()
    }

    /// Enqueue a held MIDI note and return its handle; success does not mean playback has started.
    pub fn enqueue_start_midi_note(
        &self,
        mut request: MidiNoteRequest,
    ) -> AudioEnqueueResult<MidiNoteHandle> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        request.held = true;
        self.enqueue(AudioCommand::MidiNote {
            request: OwnedMidiNoteRequest::from_request(request),
        })
        .map(|()| MidiNoteHandle(id))
    }

    pub fn play_spatial_midi_note(&self, request: MidiNoteRequest) -> Option<u64> {
        self.enqueue_play_spatial_midi_note(request).ok()
    }

    /// Enqueue spatial MIDI-note playback and return its id; success does not mean playback has started.
    pub fn enqueue_play_spatial_midi_note(
        &self,
        mut request: MidiNoteRequest,
    ) -> AudioEnqueueResult<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.enqueue_play_midi_note_spatial(request).map(|()| id)
    }

    /// Play a note on a dedicated per-note sink addressable by `request.id`
    /// via `update_spatial`/`stop_playback`. Caller keeps the id.
    pub fn play_midi_note_spatial(&self, request: MidiNoteRequest) -> bool {
        self.enqueue_play_midi_note_spatial(request).is_ok()
    }

    /// Enqueue dedicated spatial MIDI-note playback; success does not mean playback has started.
    pub fn enqueue_play_midi_note_spatial(&self, request: MidiNoteRequest) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::MidiNoteSpatial {
            request: OwnedMidiNoteRequest::from_request(request),
        })
    }

    pub fn play_midi_file(&self, request: MidiFileRequest<'_>) -> bool {
        self.enqueue_play_midi_file(request).is_ok()
    }

    /// Enqueue MIDI-file playback; success does not mean playback has started.
    pub fn enqueue_play_midi_file(&self, request: MidiFileRequest<'_>) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::MidiFile {
            request: OwnedMidiFileRequest::from_request(request),
        })
    }

    pub fn play_spatial_midi_file(&self, request: MidiFileRequest<'_>) -> Option<u64> {
        self.enqueue_play_spatial_midi_file(request).ok()
    }

    /// Enqueue spatial MIDI-file playback and return its id; success does not mean playback has started.
    pub fn enqueue_play_spatial_midi_file(
        &self,
        mut request: MidiFileRequest<'_>,
    ) -> AudioEnqueueResult<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.enqueue(AudioCommand::MidiFile {
            request: OwnedMidiFileRequest::from_request(request),
        })
        .map(|()| id)
    }

    pub fn release_midi_note(&self, handle: MidiNoteHandle) -> bool {
        self.enqueue_release_midi_note(handle).is_ok()
    }

    /// Enqueue MIDI-note release; success does not mean release has been applied.
    pub fn enqueue_release_midi_note(&self, handle: MidiNoteHandle) -> AudioEnqueueResult {
        self.enqueue(AudioCommand::MidiRelease { id: handle.0 })
    }

    pub fn play_midi_notes<I>(&self, requests: I) -> bool
    where
        I: IntoIterator<Item = MidiNoteRequest>,
    {
        self.enqueue_play_midi_notes(requests).is_ok()
    }

    /// Enqueue MIDI-note playback as one command; success does not mean playback has started.
    pub fn enqueue_play_midi_notes<I>(&self, requests: I) -> AudioEnqueueResult
    where
        I: IntoIterator<Item = MidiNoteRequest>,
    {
        let requests = requests
            .into_iter()
            .map(OwnedMidiNoteRequest::from_request)
            .collect::<Vec<_>>();
        self.enqueue(AudioCommand::MidiNotes { requests })
    }

    pub fn play_midi_note_slice(&self, requests: &[MidiNoteRequest]) -> bool {
        self.enqueue_play_midi_note_slice(requests).is_ok()
    }

    /// Enqueue MIDI-note slice playback; success does not mean playback has started.
    pub fn enqueue_play_midi_note_slice(&self, requests: &[MidiNoteRequest]) -> AudioEnqueueResult {
        if requests.len() == 1 {
            return self.enqueue_play_midi_note(requests[0]);
        }
        let requests = requests
            .iter()
            .copied()
            .map(OwnedMidiNoteRequest::from_request)
            .collect::<Vec<_>>();
        self.enqueue(AudioCommand::MidiNotes { requests })
    }
}

fn audio_disabled_by_env() -> bool {
    std::env::var_os(AUDIO_DISABLED_ENV).is_some_and(|value| {
        let value = value.to_string_lossy();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

#[cfg(test)]
mod tests {
    use super::{AudioController, AudioEnqueueError};

    #[test]
    fn enqueue_reports_full_bounded_queue() {
        let (tx, _rx) = crossbeam_channel::bounded(1);
        let controller = AudioController::from_test_sender(tx);

        assert_eq!(controller.enqueue_stop_all(), Ok(()));
        assert_eq!(
            controller.enqueue_load_source("res://queued.wav"),
            Err(AudioEnqueueError::Full)
        );
        assert!(!controller.load_source("res://compat.wav"));
    }

    #[test]
    fn enqueue_reports_disconnected_queue() {
        let (tx, rx) = crossbeam_channel::bounded(1);
        drop(rx);
        let controller = AudioController::from_test_sender(tx);

        assert_eq!(
            controller.enqueue_load_source("res://queued.wav"),
            Err(AudioEnqueueError::Disconnected)
        );
        assert!(!controller.stop_all());
    }
}
