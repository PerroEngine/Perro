use perro_ids::BusID;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

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
    cache: HashMap<String, CachedAudioAsset>,
    cache_bytes: usize,
}

enum AudioCommand {
    Load {
        source: String,
        reserved: bool,
    },
    DropAsset {
        source: String,
    },
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
    SourceLength {
        source: String,
        reply: Sender<Option<f32>>,
    },
}

#[derive(Clone)]
struct CachedAudioAsset {
    bytes: Arc<[u8]>,
    duration: Option<Duration>,
    reserved: bool,
    last_touched: Instant,
}

#[derive(Clone)]
pub struct AudioController {
    tx: Sender<AudioCommand>,
}

impl BarkPlayer {
    const CACHE_SOFT_LIMIT_BYTES: usize = 128 * 1024 * 1024;
    const UNRESERVED_TTL_FACTOR: f32 = 2.0;
    const UNRESERVED_TTL_FALLBACK: Duration = Duration::from_secs(1);
    const UNRESERVED_TTL_MIN: Duration = Duration::from_millis(250);

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
                cache: HashMap::new(),
                cache_bytes: 0,
            }),
        })
    }

    fn decode_duration_from_cached_bytes(bytes: Arc<[u8]>) -> Option<Duration> {
        let cursor = Cursor::new(bytes);
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
        let (bytes, total_duration) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "audio mutex poisoned".to_string())?;
            let now = Instant::now();
            Self::prune_finished_playbacks_locked(&mut state, now);
            let (bytes, duration) = Self::get_or_load_asset_locked(&mut state, source, false)
                .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
            (bytes, duration)
        };

        let cursor = Cursor::new(bytes.clone());
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader)
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;

        let sink =
            Sink::try_new(&self.handle).map_err(|err| format!("failed to create sink: {err}"))?;
        sink.set_speed(speed.max(0.01));

        let trim_start = Duration::from_secs_f32(from_start.max(0.0));
        let trim_end = Duration::from_secs_f32(from_end.max(0.0));
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
        Self::evict_unreserved_unused_locked(&mut state, Instant::now());
        Self::enforce_cache_soft_limit_locked(&mut state);
        Ok(())
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let (_, duration) = Self::get_or_load_asset_locked(&mut state, source, false).ok()?;
        duration.map(|d| d.as_secs_f32())
    }

    pub fn load_source(&self, source: &str, reserved: bool) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let _ = Self::get_or_load_asset_locked(&mut state, source, reserved)
            .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
        Self::evict_unreserved_unused_locked(&mut state, now);
        Self::enforce_cache_soft_limit_locked(&mut state);
        Ok(())
    }

    pub fn drop_source_asset(&self, source: &str) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let had_asset = if let Some(entry) = state.cache.remove(source) {
            state.cache_bytes = state.cache_bytes.saturating_sub(entry.bytes.len());
            true
        } else {
            false
        };
        Self::prune_finished_playbacks_locked(&mut state, Instant::now());
        had_asset
    }

    pub fn stop_source(&self, source: &str) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let mut removed_any = false;
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].source == source {
                let removed = state.playbacks.remove(i);
                removed.sink.stop();
                Self::mark_source_touched_now(&mut state, &removed.source, now);
                removed_any = true;
            } else {
                i += 1;
            }
        }
        Self::evict_unreserved_unused_locked(&mut state, now);
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
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
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
                let removed = state.playbacks.remove(i);
                removed.sink.stop();
                Self::mark_source_touched_now(&mut state, &removed.source, now);
                Self::evict_unreserved_unused_locked(&mut state, now);
                return true;
            }
            i += 1;
        }
        Self::evict_unreserved_unused_locked(&mut state, now);
        false
    }

    pub fn stop_all(&self) {
        if let Ok(mut state) = self.state.lock() {
            let now = Instant::now();
            let drained: Vec<_> = state.playbacks.drain(..).collect();
            for playback in drained {
                Self::mark_source_touched_now(&mut state, &playback.source, now);
                playback.sink.stop();
            }
            Self::evict_unreserved_unused_locked(&mut state, now);
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
                let removed = state.playbacks.remove(i);
                removed.sink.stop();
                Self::mark_source_touched_now(&mut state, &removed.source, Instant::now());
                removed_any = true;
            } else {
                i += 1;
            }
        }
        Self::evict_unreserved_unused_locked(&mut state, Instant::now());
        removed_any
    }

    fn get_or_load_asset_locked(
        state: &mut AudioState,
        source: &str,
        reserved: bool,
    ) -> Result<(Arc<[u8]>, Option<Duration>), String> {
        if let Some(existing) = state.cache.get_mut(source) {
            if reserved {
                existing.reserved = true;
            }
            existing.last_touched = Instant::now();
            return Ok((existing.bytes.clone(), existing.duration));
        }

        let bytes = perro_io::load_asset(source).map_err(|err| err.to_string())?;
        let shared: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let duration = Self::decode_duration_from_cached_bytes(shared.clone());
        state.cache_bytes = state.cache_bytes.saturating_add(shared.len());
        state.cache.insert(
            source.to_string(),
            CachedAudioAsset {
                bytes: shared.clone(),
                duration,
                reserved,
                last_touched: Instant::now(),
            },
        );
        Ok((shared, duration))
    }

    fn mark_source_touched_now(state: &mut AudioState, source: &str, now: Instant) {
        if let Some(entry) = state.cache.get_mut(source) {
            entry.last_touched = now;
        }
    }

    fn prune_finished_playbacks_locked(state: &mut AudioState, now: Instant) {
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].sink.empty() {
                let source = state.playbacks.remove(i).source;
                Self::mark_source_touched_now(state, &source, now);
            } else {
                i += 1;
            }
        }
        Self::evict_unreserved_unused_locked(state, now);
    }

    fn unreserved_ttl(entry: &CachedAudioAsset) -> Duration {
        if let Some(duration) = entry.duration {
            let scaled = Duration::from_secs_f32(duration.as_secs_f32() * Self::UNRESERVED_TTL_FACTOR);
            return scaled.max(Self::UNRESERVED_TTL_MIN);
        }
        Self::UNRESERVED_TTL_FALLBACK
    }

    fn evict_unreserved_unused_locked(state: &mut AudioState, now: Instant) {
        let mut in_use = HashMap::<&str, usize>::new();
        for playback in &state.playbacks {
            in_use
                .entry(playback.source.as_str())
                .and_modify(|v| *v += 1)
                .or_insert(1);
        }
        let mut to_remove = Vec::new();
        for (source, entry) in &state.cache {
            if entry.reserved || in_use.contains_key(source.as_str()) {
                continue;
            }
            if now.duration_since(entry.last_touched) >= Self::unreserved_ttl(entry) {
                to_remove.push(source.clone());
            }
        }
        for source in to_remove {
            if let Some(entry) = state.cache.remove(&source) {
                state.cache_bytes = state.cache_bytes.saturating_sub(entry.bytes.len());
            }
        }
    }

    fn enforce_cache_soft_limit_locked(state: &mut AudioState) {
        if state.cache_bytes <= Self::CACHE_SOFT_LIMIT_BYTES {
            return;
        }
        let mut to_remove = Vec::new();
        for (source, entry) in &state.cache {
            if entry.reserved {
                continue;
            }
            let in_use = state.playbacks.iter().any(|p| p.source == *source);
            if !in_use {
                to_remove.push(source.clone());
            }
        }
        for source in to_remove {
            if state.cache_bytes <= Self::CACHE_SOFT_LIMIT_BYTES {
                break;
            }
            if let Some(entry) = state.cache.remove(&source) {
                state.cache_bytes = state.cache_bytes.saturating_sub(entry.bytes.len());
            }
        }
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
                        AudioCommand::Load { source, reserved } => {
                            let _ = player.load_source(&source, reserved);
                        }
                        AudioCommand::DropAsset { source } => {
                            let _ = player.drop_source_asset(&source);
                        }
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
                        AudioCommand::SourceLength { source, reply } => {
                            let _ = reply.send(player.source_length_seconds(&source));
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

    pub fn load_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Load {
                source: source.to_string(),
                reserved: false,
            })
            .is_ok()
    }

    pub fn reserve_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Load {
                source: source.to_string(),
                reserved: true,
            })
            .is_ok()
    }

    pub fn drop_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::DropAsset {
                source: source.to_string(),
            })
            .is_ok()
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let (reply_tx, reply_rx) = mpsc::channel::<Option<f32>>();
        if self
            .tx
            .send(AudioCommand::SourceLength {
                source: source.to_string(),
                reply: reply_tx,
            })
            .is_err()
        {
            return None;
        }
        reply_rx.recv().ok().flatten()
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
