use crossbeam_channel::{self, Sender};
use perro_ids::AudioBusID;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::internal::{AudioCommand, OwnedAudioPlaybackRequest};
use crate::internal::{OwnedMidiFileRequest, OwnedMidiNoteRequest};
use crate::midi::{MidiFileRequest, MidiNoteHandle, MidiNoteRequest};
use crate::player::BarkPlayer;
use crate::types::{AudioPlaybackRequest, SpatialAudioParams};

const AUDIO_DISABLED_ENV: &str = "PERRO_AUDIO_DISABLED";

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
                        AudioCommand::LoadMidiFile { source } => {
                            let _ = player.load_midi_file(&source);
                        }
                        AudioCommand::MidiNote { request } => {
                            let _ = player.play_midi_note(request.as_request());
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
        let source = self.intern_source(request.source);
        self.play_source_arc(request, source)
    }

    pub fn play_source_handle(
        &self,
        handle: &AudioSourceHandle,
        request: AudioPlaybackRequest<'_>,
    ) -> bool {
        self.play_source_arc(request, handle.source.clone())
    }

    fn play_source_arc(&self, request: AudioPlaybackRequest<'_>, source: Arc<str>) -> bool {
        self.tx
            .try_send(AudioCommand::Play {
                request: OwnedAudioPlaybackRequest::from_request_with_source(request, source),
            })
            .is_ok()
    }

    pub fn play_spatial_source(&self, mut request: AudioPlaybackRequest<'_>) -> Option<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        let source = self.intern_source(request.source);
        self.play_spatial_source_arc(request, source, id)
    }

    pub fn play_spatial_source_handle(
        &self,
        handle: &AudioSourceHandle,
        mut request: AudioPlaybackRequest<'_>,
    ) -> Option<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.play_spatial_source_arc(request, handle.source.clone(), id)
    }

    fn play_spatial_source_arc(
        &self,
        request: AudioPlaybackRequest<'_>,
        source: Arc<str>,
        id: u64,
    ) -> Option<u64> {
        self.tx
            .try_send(AudioCommand::Play {
                request: OwnedAudioPlaybackRequest::from_request_with_source(request, source),
            })
            .is_ok()
            .then_some(id)
    }

    pub fn update_spatial(&self, id: u64, params: SpatialAudioParams) -> bool {
        self.tx
            .try_send(AudioCommand::UpdateSpatial { id, params })
            .is_ok()
    }

    pub fn stop_playback(&self, id: u64) -> bool {
        self.tx.try_send(AudioCommand::StopPlayback { id }).is_ok()
    }

    pub fn load_source(&self, source: &str) -> bool {
        let source = self.intern_source(source);
        self.tx
            .try_send(AudioCommand::Load {
                source,
                reserved: false,
            })
            .is_ok()
    }

    pub fn is_source_loaded(&self, source: &str) -> bool {
        let source = self.intern_source(source);
        self.loaded
            .lock()
            .map(|loaded| loaded.sources.contains(&source))
            .unwrap_or(false)
    }

    pub fn reserve_source(&self, source: &str) -> bool {
        let source = self.intern_source(source);
        self.tx
            .try_send(AudioCommand::Load {
                source,
                reserved: true,
            })
            .is_ok()
    }

    pub fn drop_source(&self, source: &str) -> bool {
        let source = self.intern_source(source);
        self.tx.try_send(AudioCommand::DropAsset { source }).is_ok()
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let source = self.intern_source(source);
        let (reply_tx, reply_rx) = crossbeam_channel::bounded::<Option<f32>>(1);
        if self
            .tx
            .try_send(AudioCommand::SourceLength {
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
        let source = self.intern_source(source);
        self.tx.try_send(AudioCommand::Stop { source }).is_ok()
    }

    pub fn stop_match(&self, request: AudioPlaybackRequest<'_>) -> bool {
        let source = self.intern_source(request.source);
        self.tx
            .try_send(AudioCommand::StopMatch {
                request: OwnedAudioPlaybackRequest::from_request_with_source(request, source),
            })
            .is_ok()
    }

    pub fn stop_all(&self) -> bool {
        self.tx.try_send(AudioCommand::StopAll).is_ok()
    }

    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.tx
            .try_send(AudioCommand::SetMasterVolume { volume })
            .is_ok()
    }

    pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool {
        self.tx
            .try_send(AudioCommand::SetBusVolume { bus_id, volume })
            .is_ok()
    }

    pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool {
        self.tx
            .try_send(AudioCommand::SetBusSpeed { bus_id, speed })
            .is_ok()
    }

    pub fn pause_bus(&self, bus_id: AudioBusID) -> bool {
        self.tx.try_send(AudioCommand::PauseBus { bus_id }).is_ok()
    }

    pub fn resume_bus(&self, bus_id: AudioBusID) -> bool {
        self.tx.try_send(AudioCommand::ResumeBus { bus_id }).is_ok()
    }

    pub fn stop_bus(&self, bus_id: AudioBusID) -> bool {
        self.tx.try_send(AudioCommand::StopBus { bus_id }).is_ok()
    }

    pub fn load_soundfont(&self, source: &str) -> perro_ids::SoundFontID {
        let id = perro_ids::SoundFontID::from_string(source);
        self.load_soundfont_with_id(id, source)
    }

    pub fn load_soundfont_with_id(
        &self,
        id: perro_ids::SoundFontID,
        source: &str,
    ) -> perro_ids::SoundFontID {
        let source = self.intern_source(source);
        let _ = self.tx.try_send(AudioCommand::LoadSoundFont { id, source });
        id
    }

    pub fn is_soundfont_loaded(&self, id: perro_ids::SoundFontID) -> bool {
        self.loaded
            .lock()
            .map(|loaded| loaded.soundfonts.contains(&id))
            .unwrap_or(false)
    }

    pub fn load_midi_file(&self, source: &str) -> bool {
        let source = self.intern_source(source);
        self.tx
            .try_send(AudioCommand::LoadMidiFile { source })
            .is_ok()
    }

    pub fn play_midi_note(&self, request: MidiNoteRequest) -> bool {
        self.tx
            .try_send(AudioCommand::MidiNote {
                request: OwnedMidiNoteRequest::from_request(request),
            })
            .is_ok()
    }

    pub fn start_midi_note(&self, mut request: MidiNoteRequest) -> Option<MidiNoteHandle> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        request.held = true;
        self.tx
            .try_send(AudioCommand::MidiNote {
                request: OwnedMidiNoteRequest::from_request(request),
            })
            .is_ok()
            .then_some(MidiNoteHandle(id))
    }

    pub fn play_spatial_midi_note(&self, mut request: MidiNoteRequest) -> Option<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.tx
            .try_send(AudioCommand::MidiNote {
                request: OwnedMidiNoteRequest::from_request(request),
            })
            .is_ok()
            .then_some(id)
    }

    pub fn play_midi_file(&self, request: MidiFileRequest<'_>) -> bool {
        self.tx
            .try_send(AudioCommand::MidiFile {
                request: OwnedMidiFileRequest::from_request(request),
            })
            .is_ok()
    }

    pub fn play_spatial_midi_file(&self, mut request: MidiFileRequest<'_>) -> Option<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.tx
            .try_send(AudioCommand::MidiFile {
                request: OwnedMidiFileRequest::from_request(request),
            })
            .is_ok()
            .then_some(id)
    }

    pub fn release_midi_note(&self, handle: MidiNoteHandle) -> bool {
        self.tx
            .try_send(AudioCommand::MidiRelease { id: handle.0 })
            .is_ok()
    }

    pub fn play_midi_notes<I>(&self, requests: I) -> bool
    where
        I: IntoIterator<Item = MidiNoteRequest>,
    {
        let requests = requests
            .into_iter()
            .map(OwnedMidiNoteRequest::from_request)
            .collect::<Vec<_>>();
        self.tx
            .try_send(AudioCommand::MidiNotes { requests })
            .is_ok()
    }

    pub fn play_midi_note_slice(&self, requests: &[MidiNoteRequest]) -> bool {
        if requests.len() == 1 {
            return self.play_midi_note(requests[0]);
        }
        let requests = requests
            .iter()
            .copied()
            .map(OwnedMidiNoteRequest::from_request)
            .collect::<Vec<_>>();
        self.tx
            .try_send(AudioCommand::MidiNotes { requests })
            .is_ok()
    }
}

fn audio_disabled_by_env() -> bool {
    std::env::var_os(AUDIO_DISABLED_ENV).is_some_and(|value| {
        let value = value.to_string_lossy();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}
