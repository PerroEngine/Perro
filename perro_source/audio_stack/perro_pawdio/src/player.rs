use perro_ids::AudioBusID;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source, SpatialSink};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::codec::decode_static_pawdio;
#[cfg(feature = "profile")]
use crate::internal::SourceLoadKind;
use crate::internal::{AudioState, BusState, CachedAudioAsset, Playback, SourceLoadStats};
use crate::types::{AudioPan, AudioPlaybackRequest, SpatialAudioParams};

pub struct BarkPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    state: Mutex<AudioState>,
    static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
}

impl BarkPlayer {
    const CACHE_SOFT_LIMIT_BYTES: usize = 128 * 1024 * 1024;
    const CACHE_EVICT_SWEEP_INTERVAL: Duration = Duration::from_millis(100);
    const UNRESERVED_TTL_FACTOR: f32 = 2.0;
    const UNRESERVED_TTL_FALLBACK: Duration = Duration::from_secs(1);
    const UNRESERVED_TTL_MIN: Duration = Duration::from_millis(250);

    pub fn new(static_audio_lookup: Option<fn(u64) -> &'static [u8]>) -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|err| format!("audio output init failed: {err}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            static_audio_lookup,
            state: Mutex::new(AudioState {
                master_volume: 1.0,
                buses: HashMap::new(),
                playbacks: Vec::new(),
                cache: HashMap::new(),
                cache_bytes: 0,
                next_cache_epoch: 1,
                last_evict_sweep: Instant::now(),
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

    pub fn play_source(&self, request: AudioPlaybackRequest<'_>) -> Result<(), String> {
        let AudioPlaybackRequest {
            id,
            source,
            bus_id,
            looped,
            volume,
            speed,
            pan,
            low_pass,
            reverb_send,
            echo,
            reflection,
            occlusion,
            eq,
            compression,
            from_start,
            from_end,
        } = request;
        #[cfg(feature = "profile")]
        let play_begin = Instant::now();
        let (bytes, source_key, source_hash, asset_epoch, cache_hit, load_stats) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "audio mutex poisoned".to_string())?;
            let now = Instant::now();
            Self::prune_finished_playbacks_locked(&mut state, now);
            let (bytes, source_key, source_hash, asset_epoch, cache_hit, load_stats) =
                Self::get_or_load_asset_locked(&mut state, source, false, self.static_audio_lookup)
                    .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
            (
                bytes,
                source_key,
                source_hash,
                asset_epoch,
                cache_hit,
                load_stats,
            )
        };

        #[cfg(feature = "profile")]
        let decode_begin = Instant::now();
        let cursor = Cursor::new(bytes.clone());
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader)
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;
        #[cfg(feature = "profile")]
        let decode_elapsed = decode_begin.elapsed();

        #[cfg(feature = "profile")]
        let duration_probe_begin = Instant::now();
        let total_duration = if from_end > 0.0 {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "audio mutex poisoned".to_string())?;
            let known = state
                .cache
                .get(&source_hash)
                .and_then(|entry| entry.duration)
                .or_else(|| decoder.total_duration());
            if let Some(entry) = state.cache.get_mut(&source_hash) {
                entry.duration = known;
                entry.duration_known = true;
            }
            known
        } else {
            None
        };
        #[cfg(feature = "profile")]
        let duration_probe_elapsed = duration_probe_begin.elapsed();

        #[cfg(feature = "profile")]
        let sink_setup_begin = Instant::now();
        let pan = pan.clamped();
        let sink = SpatialSink::try_new(
            &self.handle,
            Self::pan_emitter_position(pan),
            [-1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        )
        .map_err(|err| format!("failed to create sink: {err}"))?;
        #[cfg(feature = "profile")]
        let sink_setup_elapsed = sink_setup_begin.elapsed();

        #[cfg(feature = "profile")]
        let append_begin = Instant::now();
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
                sink.append(
                    decoder
                        .skip_duration(trim_start)
                        .take_duration(play_duration),
                );
            }
        } else if looped {
            sink.append(decoder.skip_duration(trim_start).repeat_infinite());
        } else {
            sink.append(decoder.skip_duration(trim_start));
        }
        #[cfg(feature = "profile")]
        let append_elapsed = append_begin.elapsed();

        #[cfg(feature = "profile")]
        let activate_begin = Instant::now();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let requested_volume = volume.max(0.0);
        let master_volume = state.master_volume.max(0.0);
        let (bus_volume, bus_speed, bus_paused) = match bus_id.and_then(|id| state.buses.get(&id)) {
            Some(bus_state) => (
                bus_state.volume.max(0.0),
                bus_state.speed.max(0.01),
                bus_state.paused,
            ),
            None => (1.0, 1.0, false),
        };
        sink.set_speed(speed.max(0.01) * bus_speed);
        sink.set_volume(requested_volume * master_volume * bus_volume);
        if bus_paused {
            sink.pause();
        } else {
            sink.play();
        }

        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].source_hash == source_hash
                && state.playbacks[i].source.as_ref() == source
            {
                Self::remove_playback_locked(&mut state, i, Instant::now())
                    .sink
                    .stop();
            } else {
                i += 1;
            }
        }
        if let Some(entry) = state.cache.get_mut(&source_hash) {
            entry.active_uses = entry.active_uses.saturating_add(1);
            entry.last_touched = Instant::now();
        }
        state.playbacks.push(Playback {
            id,
            source: source_key,
            source_hash,
            asset_epoch,
            bus_id,
            looped,
            base_volume: requested_volume,
            speed: speed.max(0.01),
            pan,
            low_pass: low_pass.clamp(0.0, 1.0),
            reverb_send: reverb_send.clamp(0.0, 1.0),
            echo: echo.clamp(0.0, 1.0),
            reflection: reflection.clamp(0.0, 1.0),
            occlusion: occlusion.clamp(0.0, 1.0),
            eq,
            compression,
            from_start: from_start.max(0.0),
            from_end: from_end.max(0.0),
            sink,
        });
        Self::evict_unreserved_unused_locked(&mut state, Instant::now());
        Self::enforce_cache_soft_limit_locked(&mut state);
        #[cfg(feature = "profile")]
        {
            let activate_elapsed = activate_begin.elapsed();
            let total_elapsed = play_begin.elapsed();
            println!(
                "[audio_timing] play source={} cache_hit={} source={} static_lookup_us={:.3} pawdio_decompress_us={:.3} disk_read_us={:.3} decode_us={:.3} duration_probe_us={:.3} sink_setup_us={:.3} append_us={:.3} activate_us={:.3} total_us={:.3}",
                source,
                cache_hit,
                match load_stats.kind {
                    SourceLoadKind::Cache => "cache",
                    SourceLoadKind::Static => "static",
                    SourceLoadKind::Disk => "disk",
                },
                load_stats.static_lookup.as_secs_f64() * 1_000_000.0,
                load_stats.pawdio_decompress.as_secs_f64() * 1_000_000.0,
                load_stats.disk_read.as_secs_f64() * 1_000_000.0,
                decode_elapsed.as_secs_f64() * 1_000_000.0,
                duration_probe_elapsed.as_secs_f64() * 1_000_000.0,
                sink_setup_elapsed.as_secs_f64() * 1_000_000.0,
                append_elapsed.as_secs_f64() * 1_000_000.0,
                activate_elapsed.as_secs_f64() * 1_000_000.0,
                total_elapsed.as_secs_f64() * 1_000_000.0
            );
        }
        #[cfg(not(feature = "profile"))]
        {
            let _ = (cache_hit, load_stats);
        }
        Ok(())
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let Ok(mut state) = self.state.lock() else {
            return None;
        };
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let (bytes, _, _, _, _, _) =
            Self::get_or_load_asset_locked(&mut state, source, false, self.static_audio_lookup)
                .ok()?;
        Self::duration_for_source_locked(&mut state, source, bytes).map(|d| d.as_secs_f32())
    }

    pub fn load_source(&self, source: &str, reserved: bool) -> Result<(), String> {
        #[cfg(feature = "profile")]
        let load_begin = Instant::now();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let (_, _, _, _, cache_hit, load_stats) =
            Self::get_or_load_asset_locked(&mut state, source, reserved, self.static_audio_lookup)
                .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
        Self::evict_unreserved_unused_locked(&mut state, now);
        Self::enforce_cache_soft_limit_locked(&mut state);
        #[cfg(feature = "profile")]
        {
            let total_elapsed = load_begin.elapsed();
            println!(
                "[audio_timing] preload source={} reserved={} cache_hit={} source={} static_lookup_us={:.3} pawdio_decompress_us={:.3} disk_read_us={:.3} total_us={:.3}",
                source,
                reserved,
                cache_hit,
                match load_stats.kind {
                    SourceLoadKind::Cache => "cache",
                    SourceLoadKind::Static => "static",
                    SourceLoadKind::Disk => "disk",
                },
                load_stats.static_lookup.as_secs_f64() * 1_000_000.0,
                load_stats.pawdio_decompress.as_secs_f64() * 1_000_000.0,
                load_stats.disk_read.as_secs_f64() * 1_000_000.0,
                total_elapsed.as_secs_f64() * 1_000_000.0
            );
        }
        #[cfg(not(feature = "profile"))]
        {
            let _ = (cache_hit, load_stats);
        }
        Ok(())
    }

    pub fn drop_source_asset(&self, source: &str) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let source_hash = perro_ids::string_to_u64(source);
        if state
            .cache
            .get(&source_hash)
            .is_some_and(|entry| entry.source.as_ref() != source)
        {
            return false;
        }
        let had_asset = if let Some(entry) = state.cache.remove(&source_hash) {
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
        let source_hash = perro_ids::string_to_u64(source);
        let mut removed_any = false;
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].source_hash == source_hash
                && state.playbacks[i].source.as_ref() == source
            {
                let removed = Self::remove_playback_locked(&mut state, i, now);
                removed.sink.stop();
                removed_any = true;
            } else {
                i += 1;
            }
        }
        Self::evict_unreserved_unused_locked(&mut state, now);
        removed_any
    }

    pub fn stop_match(&self, request: AudioPlaybackRequest<'_>) -> bool {
        let AudioPlaybackRequest {
            id: _,
            source,
            bus_id,
            looped,
            volume,
            speed,
            pan,
            low_pass: _,
            reverb_send: _,
            echo: _,
            reflection: _,
            occlusion: _,
            eq: _,
            compression: _,
            from_start,
            from_end,
        } = request;
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let target_volume = volume.max(0.0);
        let target_speed = speed.max(0.01);
        let target_pan = pan.clamped();
        let target_from_start = from_start.max(0.0);
        let target_from_end = from_end.max(0.0);
        let source_hash = perro_ids::string_to_u64(source);
        let mut i = 0usize;
        while i < state.playbacks.len() {
            let p = &state.playbacks[i];
            if p.source_hash == source_hash
                && p.source.as_ref() == source
                && p.bus_id == bus_id
                && p.looped == looped
                && (p.base_volume - target_volume).abs() < f32::EPSILON
                && (p.speed - target_speed).abs() < f32::EPSILON
                && (p.pan.x - target_pan.x).abs() < f32::EPSILON
                && (p.pan.y - target_pan.y).abs() < f32::EPSILON
                && (p.pan.z - target_pan.z).abs() < f32::EPSILON
                && (p.from_start - target_from_start).abs() < f32::EPSILON
                && (p.from_end - target_from_end).abs() < f32::EPSILON
            {
                let removed = Self::remove_playback_locked(&mut state, i, now);
                removed.sink.stop();
                Self::evict_unreserved_unused_locked(&mut state, now);
                return true;
            }
            i += 1;
        }
        Self::evict_unreserved_unused_locked(&mut state, now);
        false
    }

    pub fn stop_playback(&self, id: u64) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].id == id {
                let removed = Self::remove_playback_locked(&mut state, i, now);
                removed.sink.stop();
                Self::evict_unreserved_unused_locked(&mut state, now);
                return true;
            }
            i += 1;
        }
        false
    }

    pub fn update_spatial(&self, id: u64, params: SpatialAudioParams) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let master_volume = state.master_volume.max(0.0);
        let Some(index) = state.playbacks.iter().position(|p| p.id == id) else {
            return false;
        };
        let playback_bus_id = state.playbacks[index].bus_id;
        let bus_volume = playback_bus_id
            .and_then(|bus_id| state.buses.get(&bus_id))
            .map(|bus| bus.volume.max(0.0))
            .unwrap_or(1.0);
        let playback = &mut state.playbacks[index];
        playback.base_volume = params.volume.max(0.0);
        playback.pan = params.pan.clamped();
        playback.low_pass = params.low_pass.clamp(0.0, 1.0);
        playback.reverb_send = params.reverb_send.clamp(0.0, 1.0);
        playback.echo = params.echo.clamp(0.0, 1.0);
        playback.reflection = params.reflection.clamp(0.0, 1.0);
        playback.occlusion = params.occlusion.clamp(0.0, 1.0);
        playback.eq = params.eq;
        playback.compression = params.compression;
        playback
            .sink
            .set_emitter_position(Self::pan_emitter_position(playback.pan));
        playback
            .sink
            .set_volume(playback.base_volume * master_volume * bus_volume);
        true
    }

    pub fn stop_all(&self) {
        if let Ok(mut state) = self.state.lock() {
            let now = Instant::now();
            while !state.playbacks.is_empty() {
                let playback = Self::remove_playback_locked(&mut state, 0, now);
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

    pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) {
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

    pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) {
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

    pub fn pause_bus(&self, bus_id: AudioBusID) {
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
            if playback.bus_id == Some(bus_id) {
                playback.sink.pause();
            }
        }
    }

    pub fn resume_bus(&self, bus_id: AudioBusID) {
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
            if playback.bus_id == Some(bus_id) {
                playback.sink.play();
            }
        }
    }

    pub fn stop_bus(&self, bus_id: AudioBusID) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let mut removed_any = false;
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].bus_id == Some(bus_id) {
                let removed = Self::remove_playback_locked(&mut state, i, Instant::now());
                removed.sink.stop();
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
        static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
    ) -> Result<(Arc<[u8]>, Arc<str>, u64, u64, bool, SourceLoadStats), String> {
        let source_hash = perro_ids::string_to_u64(source);
        if let Some(existing) = state.cache.get_mut(&source_hash) {
            if existing.source.as_ref() != source {
                return Err(format!(
                    "audio source hash collision: `{}` conflicts with `{source}`",
                    existing.source
                ));
            }
            if reserved {
                existing.reserved = true;
            }
            existing.last_touched = Instant::now();
            return Ok((
                existing.bytes.clone(),
                existing.source.clone(),
                existing.source_hash,
                existing.asset_epoch,
                true,
                SourceLoadStats::cache_hit(),
            ));
        }
        let (bytes, load_stats) = if let Some(lookup) = static_audio_lookup {
            #[cfg(feature = "profile")]
            let lookup_begin = Instant::now();
            let looked_up = lookup(source_hash);
            #[cfg(feature = "profile")]
            let lookup_elapsed = lookup_begin.elapsed();
            let (decoded, decompress_elapsed) = decode_static_pawdio(looked_up)?;
            #[cfg(not(feature = "profile"))]
            let _ = decompress_elapsed;
            #[cfg(feature = "profile")]
            let stats = SourceLoadStats {
                kind: SourceLoadKind::Static,
                static_lookup: lookup_elapsed,
                pawdio_decompress: decompress_elapsed,
                disk_read: Duration::ZERO,
            };
            #[cfg(not(feature = "profile"))]
            let stats = SourceLoadStats;
            (decoded, stats)
        } else {
            #[cfg(feature = "profile")]
            let disk_begin = Instant::now();
            let disk = perro_io::load_asset(source).map_err(|err| err.to_string())?;
            #[cfg(feature = "profile")]
            let stats = SourceLoadStats {
                kind: SourceLoadKind::Disk,
                static_lookup: Duration::ZERO,
                pawdio_decompress: Duration::ZERO,
                disk_read: disk_begin.elapsed(),
            };
            #[cfg(not(feature = "profile"))]
            let stats = SourceLoadStats;
            (disk, stats)
        };
        let shared: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let source_key: Arc<str> = Arc::from(source);
        let asset_epoch = state.next_cache_epoch.max(1);
        state.next_cache_epoch = state.next_cache_epoch.wrapping_add(1).max(1);
        state.cache_bytes = state.cache_bytes.saturating_add(shared.len());
        state.cache.insert(
            source_hash,
            CachedAudioAsset {
                source: source_key.clone(),
                source_hash,
                asset_epoch,
                bytes: shared.clone(),
                duration: None,
                duration_known: false,
                reserved,
                active_uses: 0,
                last_touched: Instant::now(),
            },
        );
        Ok((
            shared,
            source_key,
            source_hash,
            asset_epoch,
            false,
            load_stats,
        ))
    }

    fn duration_for_source_locked(
        state: &mut AudioState,
        source: &str,
        bytes: Arc<[u8]>,
    ) -> Option<Duration> {
        let source_hash = perro_ids::string_to_u64(source);
        let needs_decode = state
            .cache
            .get(&source_hash)
            .map(|entry| !entry.duration_known)
            .unwrap_or(true);

        if needs_decode {
            let decoded = Self::decode_duration_from_cached_bytes(bytes);
            if let Some(entry) = state.cache.get_mut(&source_hash) {
                entry.duration = decoded;
                entry.duration_known = true;
            }
        }

        state
            .cache
            .get(&source_hash)
            .and_then(|entry| entry.duration)
    }

    fn remove_playback_locked(state: &mut AudioState, index: usize, now: Instant) -> Playback {
        let removed = state.playbacks.swap_remove(index);
        if let Some(entry) = state.cache.get_mut(&removed.source_hash) {
            if entry.asset_epoch == removed.asset_epoch {
                entry.active_uses = entry.active_uses.saturating_sub(1);
                entry.last_touched = now;
            }
        }
        removed
    }

    fn prune_finished_playbacks_locked(state: &mut AudioState, now: Instant) {
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].sink.empty() {
                let _ = Self::remove_playback_locked(state, i, now);
            } else {
                i += 1;
            }
        }
    }

    fn unreserved_ttl(entry: &CachedAudioAsset) -> Duration {
        if let Some(duration) = entry.duration {
            let scaled =
                Duration::from_secs_f32(duration.as_secs_f32() * Self::UNRESERVED_TTL_FACTOR);
            return scaled.max(Self::UNRESERVED_TTL_MIN);
        }
        Self::UNRESERVED_TTL_FALLBACK
    }

    fn evict_unreserved_unused_locked(state: &mut AudioState, now: Instant) {
        if now.duration_since(state.last_evict_sweep) < Self::CACHE_EVICT_SWEEP_INTERVAL {
            return;
        }
        state.last_evict_sweep = now;
        let mut removed_bytes = 0usize;
        state.cache.retain(|_, entry| {
            if entry.reserved || entry.active_uses > 0 {
                return true;
            }
            if now.duration_since(entry.last_touched) >= Self::unreserved_ttl(entry) {
                removed_bytes = removed_bytes.saturating_add(entry.bytes.len());
                return false;
            }
            true
        });
        state.cache_bytes = state.cache_bytes.saturating_sub(removed_bytes);
    }

    fn enforce_cache_soft_limit_locked(state: &mut AudioState) {
        if state.cache_bytes <= Self::CACHE_SOFT_LIMIT_BYTES {
            return;
        }
        let mut cache_bytes = state.cache_bytes;
        state.cache.retain(|_, entry| {
            if cache_bytes <= Self::CACHE_SOFT_LIMIT_BYTES
                || entry.reserved
                || entry.active_uses > 0
            {
                return true;
            }
            cache_bytes = cache_bytes.saturating_sub(entry.bytes.len());
            false
        });
        state.cache_bytes = cache_bytes;
    }

    fn refresh_volumes(state: &mut AudioState) {
        for playback in &state.playbacks {
            let bus_volume = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.volume.max(0.0))
                .unwrap_or(1.0);
            playback
                .sink
                .set_volume(playback.base_volume * state.master_volume.max(0.0) * bus_volume);
        }
    }

    fn refresh_speeds(state: &mut AudioState) {
        for playback in &state.playbacks {
            let bus_speed = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.speed.max(0.01))
                .unwrap_or(1.0);
            playback
                .sink
                .set_speed(playback.speed.max(0.01) * bus_speed);
        }
    }

    fn pan_emitter_position(pan: AudioPan) -> [f32; 3] {
        [pan.x, pan.y, pan.z]
    }
}
