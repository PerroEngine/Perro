use perro_ids::BusID;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

pub struct BarkPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    state: Mutex<AudioState>,
}

struct Playback {
    source: String,
    bus_id: BusID,
    looped: bool,
    base_volume: f32,
    speed: f32,
    from_start: f32,
    from_end: f32,
    sink: Sink,
}

#[derive(Clone, Copy)]
struct BusState {
    volume: f32,
    speed: f32,
    paused: bool,
}

struct AudioState {
    master_volume: f32,
    buses: HashMap<BusID, BusState>,
    playbacks: Vec<Playback>,
}

enum AudioCommand {
    Play {
        source: String,
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    },
    Stop {
        source: String,
    },
    StopMatch {
        source: String,
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    },
    StopAll,
    SetMasterVolume {
        volume: f32,
    },
    SetBusVolume {
        bus_id: BusID,
        volume: f32,
    },
    SetBusSpeed {
        bus_id: BusID,
        speed: f32,
    },
    PauseBus {
        bus_id: BusID,
    },
    ResumeBus {
        bus_id: BusID,
    },
    StopBus {
        bus_id: BusID,
    },
}

#[derive(Clone)]
pub struct AudioController {
    tx: Sender<AudioCommand>,
}

impl BarkPlayer {
    fn decoded_total_duration_from_bytes(bytes: &[u8]) -> Option<Duration> {
        let cursor = Cursor::new(bytes.to_vec());
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader).ok()?;
        if let Some(duration) = decoder.total_duration() {
            return Some(duration);
        }
        let channels = decoder.channels() as f64;
        let sample_rate = decoder.sample_rate() as f64;
        if channels <= 0.0 || sample_rate <= 0.0 {
            return None;
        }
        let sample_count = decoder.count() as f64;
        if sample_count <= 0.0 {
            return None;
        }
        let seconds = sample_count / (channels * sample_rate);
        Some(Duration::from_secs_f64(seconds))
    }

    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|err| format!("audio output init failed: {err}"))?;
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
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> Result<(), String> {
        let bytes = perro_io::load_asset(source)
            .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
        self.play_bytes(
            source, bytes, bus_id, looped, volume, speed, from_start, from_end,
        )
    }

    pub fn play_bytes(
        &self,
        source: &str,
        bytes: Vec<u8>,
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> Result<(), String> {
        let bytes_for_duration = bytes.clone();
        let cursor = Cursor::new(bytes);
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader)
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;

        let sink =
            Sink::try_new(&self.handle).map_err(|err| format!("failed to create sink: {err}"))?;
        sink.set_speed(speed.max(0.01));

        let trim_start = Duration::from_secs_f32(from_start.max(0.0));
        let trim_end = Duration::from_secs_f32(from_end.max(0.0));
        let total_duration = decoder
            .total_duration()
            .or_else(|| Self::decoded_total_duration_from_bytes(&bytes_for_duration));
        if let Some(total_duration) = total_duration {
            let after_start = total_duration.saturating_sub(trim_start);
            let play_duration = after_start.saturating_sub(trim_end);
            if play_duration.is_zero() {
                return Err(format!(
                    "invalid trim for `{source}`: from_start + from_end removes full clip"
                ));
            }
            if looped {
                sink.append(
                    decoder
                        .skip_duration(trim_start)
                        .take_duration(play_duration)
                        .repeat_infinite(),
                );
            } else {
                sink.append(decoder.skip_duration(trim_start).take_duration(play_duration));
            }
        } else if looped {
            sink.append(decoder.skip_duration(trim_start).repeat_infinite());
        } else {
            sink.append(decoder.skip_duration(trim_start));
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let requested_volume = volume.max(0.0);
        let master_volume = state.master_volume.max(0.0);
        let bus_state = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            speed: 1.0,
            paused: false,
        });
        sink.set_speed(speed.max(0.01) * bus_state.speed.max(0.01));
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
            speed: speed.max(0.01),
            from_start: from_start.max(0.0),
            from_end: from_end.max(0.0),
            sink,
        });
        Ok(())
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let bytes = perro_io::load_asset(source).ok()?;
        Self::decoded_total_duration_from_bytes(&bytes).map(|duration| duration.as_secs_f32())
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
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let target_volume = volume.max(0.0);
        let target_speed = speed.max(0.01);
        let target_from_start = from_start.max(0.0);
        let target_from_end = from_end.max(0.0);
        let mut i = 0usize;
        while i < state.playbacks.len() {
            let p = &state.playbacks[i];
            if p.source == source
                && p.bus_id == bus_id
                && p.looped == looped
                && (p.base_volume - target_volume).abs() < f32::EPSILON
                && (p.speed - target_speed).abs() < f32::EPSILON
                && (p.from_start - target_from_start).abs() < f32::EPSILON
                && (p.from_end - target_from_end).abs() < f32::EPSILON
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

    pub fn set_bus_volume(&self, bus_id: BusID, volume: f32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            speed: 1.0,
            paused: false,
        });
        bus.volume = volume.max(0.0);
        Self::refresh_volumes(&mut state);
    }

    pub fn set_bus_speed(&self, bus_id: BusID, speed: f32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            speed: 1.0,
            paused: false,
        });
        bus.speed = speed.max(0.01);
        Self::refresh_speeds(&mut state);
    }

    pub fn pause_bus(&self, bus_id: BusID) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            speed: 1.0,
            paused: false,
        });
        bus.paused = true;
        for playback in &state.playbacks {
            if playback.bus_id == bus_id {
                playback.sink.pause();
            }
        }
    }

    pub fn resume_bus(&self, bus_id: BusID) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let bus = state.buses.entry(bus_id).or_insert(BusState {
            volume: 1.0,
            speed: 1.0,
            paused: false,
        });
        bus.paused = false;
        for playback in &state.playbacks {
            if playback.bus_id == bus_id {
                playback.sink.play();
            }
        }
    }

    pub fn stop_bus(&self, bus_id: BusID) -> bool {
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

    fn refresh_speeds(state: &mut AudioState) {
        for playback in &state.playbacks {
            let bus_speed = state
                .buses
                .get(&playback.bus_id)
                .map(|bus| bus.speed.max(0.01))
                .unwrap_or(1.0);
            playback
                .sink
                .set_speed(playback.speed.max(0.01) * bus_speed);
        }
    }
}

impl AudioController {
    fn decode_length_seconds(source: &str) -> Option<f32> {
        let bytes = perro_io::load_asset(source).ok()?;
        BarkPlayer::decoded_total_duration_from_bytes(&bytes).map(|duration| duration.as_secs_f32())
    }

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
                            speed,
                            from_start,
                            from_end,
                        } => {
                            let _ = player.play_source(
                                &source, bus_id, looped, volume, speed, from_start, from_end,
                            );
                        }
                        AudioCommand::Stop { source } => {
                            let _ = player.stop_source(&source);
                        }
                        AudioCommand::StopMatch {
                            source,
                            bus_id,
                            looped,
                            volume,
                            speed,
                            from_start,
                            from_end,
                        } => {
                            let _ = player.stop_match(
                                &source, bus_id, looped, volume, speed, from_start, from_end,
                            );
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
                    }
                }
            })
            .map_err(|err| format!("failed to spawn audio thread: {err}"))?;
        Ok(Self { tx })
    }

    pub fn play_source(
        &self,
        source: &str,
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> bool {
        self.tx
            .send(AudioCommand::Play {
                source: source.to_string(),
                bus_id,
                looped,
                volume,
                speed,
                from_start,
                from_end,
            })
            .is_ok()
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        Self::decode_length_seconds(source)
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
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> bool {
        self.tx
            .send(AudioCommand::StopMatch {
                source: source.to_string(),
                bus_id,
                looped,
                volume,
                speed,
                from_start,
                from_end,
            })
            .is_ok()
    }

    pub fn stop_all(&self) -> bool {
        self.tx.send(AudioCommand::StopAll).is_ok()
    }

    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.tx
            .send(AudioCommand::SetMasterVolume { volume })
            .is_ok()
    }

    pub fn set_bus_volume(&self, bus_id: BusID, volume: f32) -> bool {
        self.tx
            .send(AudioCommand::SetBusVolume { bus_id, volume })
            .is_ok()
    }

    pub fn set_bus_speed(&self, bus_id: BusID, speed: f32) -> bool {
        self.tx
            .send(AudioCommand::SetBusSpeed { bus_id, speed })
            .is_ok()
    }

    pub fn pause_bus(&self, bus_id: BusID) -> bool {
        self.tx.send(AudioCommand::PauseBus { bus_id }).is_ok()
    }

    pub fn resume_bus(&self, bus_id: BusID) -> bool {
        self.tx.send(AudioCommand::ResumeBus { bus_id }).is_ok()
    }

    pub fn stop_bus(&self, bus_id: BusID) -> bool {
        self.tx.send(AudioCommand::StopBus { bus_id }).is_ok()
    }
}
