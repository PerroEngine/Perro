use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

pub struct BarkPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    state: Mutex<AudioState>,
}

struct Playback {
    source: String,
    bus_id: u32,
    looped: bool,
    base_volume: f32,
    pitch: f32,
    sink: Sink,
}

#[derive(Clone, Copy)]
struct BusState {
    volume: f32,
    paused: bool,
}

struct AudioState {
    master_volume: f32,
    buses: HashMap<u32, BusState>,
    playbacks: Vec<Playback>,
}

enum AudioCommand {
    Play {
        source: String,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    },
    Stop { source: String },
    StopMatch {
        source: String,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    },
    StopAll,
    SetMasterVolume { volume: f32 },
    SetBusVolume { bus_id: u32, volume: f32 },
    PauseBus { bus_id: u32 },
    ResumeBus { bus_id: u32 },
    StopBus { bus_id: u32 },
}

#[derive(Clone)]
pub struct AudioController {
    tx: Sender<AudioCommand>,
}

impl BarkPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|err| format!("audio output init failed: {err}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            state: Mutex::new(AudioState {
                master_volume: 1.0,
                buses: HashMap::new(),
                playbacks: Vec::new(),
            }),
        })
    }

    pub fn play_source(
        &self,
        source: &str,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> Result<(), String> {
        let bytes = perro_io::load_asset(source)
            .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
        self.play_bytes(source, bytes, bus_id, looped, volume, pitch)
    }

    pub fn play_bytes(
        &self,
        source: &str,
        bytes: Vec<u8>,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> Result<(), String> {
        let cursor = Cursor::new(bytes);
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader)
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;

        let sink =
            Sink::try_new(&self.handle).map_err(|err| format!("failed to create sink: {err}"))?;
        sink.set_speed(pitch.max(0.01));

        if looped {
            sink.append(decoder.repeat_infinite());
        } else {
            sink.append(decoder);
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let requested_volume = volume.max(0.0);
        let master_volume = state.master_volume.max(0.0);
        let bus_state = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            paused: false,
        });
        sink.set_volume(requested_volume * master_volume * bus_state.volume.max(0.0));
        if bus_state.paused {
            sink.pause();
        } else {
            sink.play();
        }

        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].source == source {
                state.playbacks.remove(i).sink.stop();
            } else {
                i += 1;
            }
        }
        state.playbacks.push(Playback {
            source: source.to_string(),
            bus_id,
            looped,
            base_volume: requested_volume,
            pitch: pitch.max(0.01),
            sink,
        });
        Ok(())
    }

    pub fn stop_source(&self, source: &str) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let mut removed_any = false;
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].source == source {
                state.playbacks.remove(i).sink.stop();
                removed_any = true;
            } else {
                i += 1;
            }
        }
        removed_any
    }

    pub fn stop_match(
        &self,
        source: &str,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let target_volume = volume.max(0.0);
        let target_pitch = pitch.max(0.01);
        let mut i = 0usize;
        while i < state.playbacks.len() {
            let p = &state.playbacks[i];
            if p.source == source
                && p.bus_id == bus_id
                && p.looped == looped
                && (p.base_volume - target_volume).abs() < f32::EPSILON
                && (p.pitch - target_pitch).abs() < f32::EPSILON
            {
                state.playbacks.remove(i).sink.stop();
                return true;
            }
            i += 1;
        }
        false
    }

    pub fn stop_all(&self) {
        if let Ok(mut state) = self.state.lock() {
            for playback in state.playbacks.drain(..) {
                playback.sink.stop();
            }
        }
    }

    pub fn set_master_volume(&self, volume: f32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.master_volume = volume.max(0.0);
        Self::refresh_volumes(&mut state);
    }

    pub fn set_bus_volume(&self, bus_id: u32, volume: f32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            paused: false,
        });
        bus.volume = volume.max(0.0);
        Self::refresh_volumes(&mut state);
    }

    pub fn pause_bus(&self, bus_id: u32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            paused: false,
        });
        bus.paused = true;
        for playback in &state.playbacks {
            if playback.bus_id == bus_id {
                playback.sink.pause();
            }
        }
    }

    pub fn resume_bus(&self, bus_id: u32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            paused: false,
        });
        bus.paused = false;
        for playback in &state.playbacks {
            if playback.bus_id == bus_id {
                playback.sink.play();
            }
        }
    }

    pub fn stop_bus(&self, bus_id: u32) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let mut removed_any = false;
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].bus_id == bus_id {
                state.playbacks.remove(i).sink.stop();
                removed_any = true;
            } else {
                i += 1;
            }
        }
        removed_any
    }

    fn refresh_volumes(state: &mut AudioState) {
        for playback in &state.playbacks {
            let bus_volume = state
                .buses
                .get(&playback.bus_id)
                .map(|bus| bus.volume.max(0.0))
                .unwrap_or(1.0);
            playback
                .sink
                .set_volume(playback.base_volume * state.master_volume.max(0.0) * bus_volume);
        }
    }
}

impl AudioController {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        std::thread::Builder::new()
            .name("perro_bark_audio".to_string())
            .spawn(move || {
                let Ok(player) = BarkPlayer::new() else {
                    return;
                };

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        AudioCommand::Play {
                            source,
                            bus_id,
                            looped,
                            volume,
                            pitch,
                        } => {
                            let _ = player.play_source(&source, bus_id, looped, volume, pitch);
                        }
                        AudioCommand::Stop { source } => {
                            let _ = player.stop_source(&source);
                        }
                        AudioCommand::StopMatch {
                            source,
                            bus_id,
                            looped,
                            volume,
                            pitch,
                        } => {
                            let _ = player.stop_match(&source, bus_id, looped, volume, pitch);
                        }
                        AudioCommand::StopAll => player.stop_all(),
                        AudioCommand::SetMasterVolume { volume } => player.set_master_volume(volume),
                        AudioCommand::SetBusVolume { bus_id, volume } => {
                            player.set_bus_volume(bus_id, volume)
                        }
                        AudioCommand::PauseBus { bus_id } => player.pause_bus(bus_id),
                        AudioCommand::ResumeBus { bus_id } => player.resume_bus(bus_id),
                        AudioCommand::StopBus { bus_id } => {
                            let _ = player.stop_bus(bus_id);
                        }
                    }
                }
            })
            .map_err(|err| format!("failed to spawn audio thread: {err}"))?;
        Ok(Self { tx })
    }

    pub fn play_source(
        &self,
        source: &str,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> bool {
        self.tx
            .send(AudioCommand::Play {
                source: source.to_string(),
                bus_id,
                looped,
                volume,
                pitch,
            })
            .is_ok()
    }

    pub fn stop_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Stop {
                source: source.to_string(),
            })
            .is_ok()
    }

    pub fn stop_match(
        &self,
        source: &str,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> bool {
        self.tx
            .send(AudioCommand::StopMatch {
                source: source.to_string(),
                bus_id,
                looped,
                volume,
                pitch,
            })
            .is_ok()
    }

    pub fn stop_all(&self) -> bool {
        self.tx.send(AudioCommand::StopAll).is_ok()
    }

    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.tx.send(AudioCommand::SetMasterVolume { volume }).is_ok()
    }

    pub fn set_bus_volume(&self, bus_id: u32, volume: f32) -> bool {
        self.tx
            .send(AudioCommand::SetBusVolume { bus_id, volume })
            .is_ok()
    }

    pub fn pause_bus(&self, bus_id: u32) -> bool {
        self.tx.send(AudioCommand::PauseBus { bus_id }).is_ok()
    }

    pub fn resume_bus(&self, bus_id: u32) -> bool {
        self.tx.send(AudioCommand::ResumeBus { bus_id }).is_ok()
    }

    pub fn stop_bus(&self, bus_id: u32) -> bool {
        self.tx.send(AudioCommand::StopBus { bus_id }).is_ok()
    }
}
