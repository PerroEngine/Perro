use perro_ids::AudioBusID;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source, SpatialSink};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::codec::decode_static_pawdio;
use crate::dsp::{DspControl, DspParams, DspSource};
#[cfg(feature = "profile")]
use crate::internal::SourceLoadKind;
use crate::internal::{
    AudioState, BuiltInMidiMixerPlayback, BusState, CachedAudioAsset, CachedMidiFile, CachedPcm,
    CachedSoundFont, MidiMixerKey, MidiPlayback, Playback, SoundFontMidiMixerKey,
    SoundFontMidiMixerPlayback, SourceLoadStats,
};
use crate::mic::MicClip;
use crate::midi::{
    BuiltInMidiMixerSource, BuiltInMidiSource, MidiControl, MidiFileRequest, MidiMixerControl,
    MidiMixerNote, MidiNoteRequest, MidiSound, RustyFileSource, RustyNoteMixerSource,
    SoundFontMixerControl, SoundFontMixerNote, parse_built_in_midi_file,
};
use crate::types::{AudioPan, AudioPlaybackRequest, SpatialAudioParams};

type LoadedAudioAsset = (Arc<[u8]>, Arc<str>, u64, u64, bool, SourceLoadStats);

struct MidiSinkActivation {
    id: u64,
    source: Option<Arc<str>>,
    bus_id: Option<AudioBusID>,
    volume: f32,
    pan: AudioPan,
    control: crossbeam_channel::Sender<MidiControl>,
    dsp: Arc<DspControl>,
    sink: SpatialSink,
}

pub struct BarkPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    state: Mutex<AudioState>,
    static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
}

impl BarkPlayer {
    const CACHE_SOFT_LIMIT_BYTES: usize = 128 * 1024 * 1024;
    // Clips at or under this length keep their decoded PCM cached so repeated
    // plays skip the decoder; longer clips stream-decode per play.
    const PCM_CACHE_MAX_SECONDS: usize = 12;
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
                midi_playbacks: Vec::new(),
                built_in_midi_mixers: Vec::new(),
                built_in_midi_mixer_index: HashMap::new(),
                built_in_midi_notes: HashMap::new(),
                soundfont_midi_mixers: Vec::new(),
                soundfont_midi_mixer_index: HashMap::new(),
                soundfont_midi_notes: HashMap::new(),
                cache: HashMap::new(),
                soundfonts: HashMap::new(),
                midi_files: HashMap::new(),
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
        let (bytes, source_key, source_hash, asset_epoch, cache_hit, load_stats, pcm, oversized) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "audio mutex poisoned".to_string())?;
            let now = Instant::now();
            Self::prune_finished_playbacks_locked(&mut state, now);
            let (bytes, source_key, source_hash, asset_epoch, cache_hit, load_stats) =
                Self::get_or_load_asset_locked(&mut state, source, false, self.static_audio_lookup)
                    .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
            let (pcm, oversized) = state
                .cache
                .get(&source_hash)
                .map(|entry| (entry.pcm.clone(), entry.pcm_oversized))
                .unwrap_or((None, false));
            (
                bytes,
                source_key,
                source_hash,
                asset_epoch,
                cache_hit,
                load_stats,
                pcm,
                oversized,
            )
        };

        #[cfg(feature = "profile")]
        let decode_begin = Instant::now();
        // Short clips play from cached decoded PCM; oversized/first-play misses
        // stream through a fresh decoder as before.
        let pcm = match pcm {
            Some(pcm) => Some(pcm),
            None if !oversized => self.decode_and_cache_pcm(&bytes, source_hash, source)?,
            None => None,
        };
        let decoder = if pcm.is_none() {
            let cursor = Cursor::new(bytes.clone());
            let reader = BufReader::new(cursor);
            Some(
                Decoder::new(reader)
                    .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?,
            )
        } else {
            None
        };
        #[cfg(feature = "profile")]
        let decode_elapsed = decode_begin.elapsed();

        #[cfg(feature = "profile")]
        let duration_probe_begin = Instant::now();
        let total_duration = if from_end > 0.0 {
            if let Some(pcm) = &pcm {
                Some(pcm.duration())
            } else {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| "audio mutex poisoned".to_string())?;
                let known = state
                    .cache
                    .get(&source_hash)
                    .and_then(|entry| entry.duration)
                    .or_else(|| {
                        decoder
                            .as_ref()
                            .and_then(|decoder| decoder.total_duration())
                    });
                if let Some(entry) = state.cache.get_mut(&source_hash) {
                    entry.duration = known;
                    entry.duration_known = true;
                }
                known
            }
        } else {
            None
        };
        #[cfg(feature = "profile")]
        let duration_probe_elapsed = duration_probe_begin.elapsed();

        #[cfg(feature = "profile")]
        let sink_setup_begin = Instant::now();
        let pan = pan.clamped();
        let dsp = DspControl::new(DspParams {
            low_pass,
            reverb_send,
            echo,
            reflection,
            occlusion,
            eq,
            compression,
        });
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
        let play_duration = if let Some(total_duration) = total_duration {
            let after_start = total_duration.saturating_sub(trim_start);
            let play_duration = after_start.saturating_sub(trim_end);
            if play_duration.is_zero() {
                return Err(format!(
                    "invalid trim for `{source}`: from_start + from_end removes full clip"
                ));
            }
            Some(play_duration)
        } else {
            None
        };
        match (pcm, decoder) {
            (Some(pcm), _) => append_with_trims(
                &sink,
                CachedPcmSource::new(pcm),
                dsp.clone(),
                trim_start,
                play_duration,
                looped,
            ),
            (None, Some(decoder)) => append_with_trims(
                &sink,
                decoder.convert_samples::<f32>(),
                dsp.clone(),
                trim_start,
                play_duration,
                looped,
            ),
            (None, None) => unreachable!("play source has neither pcm nor decoder"),
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
            dsp,
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

    pub fn play_clip(
        &self,
        source: &str,
        clip: MicClip,
        bus_id: Option<AudioBusID>,
        volume: f32,
        pan: AudioPan,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        drop(state);

        let pan = pan.clamped();
        let dsp = DspControl::new(DspParams::dry());
        let sink = SpatialSink::try_new(
            &self.handle,
            Self::pan_emitter_position(pan),
            [-1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        )
        .map_err(|err| format!("failed to create sink: {err}"))?;
        let samples = clip.samples_f32();
        let source_buffer = SamplesBuffer::new(clip.channels, clip.sample_rate, samples);
        sink.append(DspSource::new(source_buffer, dsp.clone()));

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
        sink.set_speed(bus_speed);
        sink.set_volume(requested_volume * master_volume * bus_volume);
        if bus_paused {
            sink.pause();
        } else {
            sink.play();
        }

        let source_hash = perro_ids::string_to_u64(source);
        let source_key: Arc<str> = Arc::from(source);
        state.playbacks.push(Playback {
            id: 0,
            source: source_key,
            source_hash,
            asset_epoch: 0,
            bus_id,
            looped: false,
            base_volume: requested_volume,
            speed: 1.0,
            pan,
            dsp,
            from_start: 0.0,
            from_end: 0.0,
            sink,
        });
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

    pub fn load_source_bytes(
        &self,
        source: &str,
        bytes: Arc<[u8]>,
        reserved: bool,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);
        Self::insert_audio_bytes_locked(&mut state, source, bytes, reserved)?;
        Self::evict_unreserved_unused_locked(&mut state, now);
        Self::enforce_cache_soft_limit_locked(&mut state);
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
            state.cache_bytes = state.cache_bytes.saturating_sub(entry.cache_len());
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
        let mut i = 0usize;
        while i < state.midi_playbacks.len() {
            if state.midi_playbacks[i]
                .source
                .as_ref()
                .is_some_and(|stored| stored.as_ref() == source)
            {
                let removed = state.midi_playbacks.swap_remove(i);
                let _ = removed.control.send(MidiControl::Stop);
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
        let mut i = 0usize;
        while i < state.midi_playbacks.len() {
            if state.midi_playbacks[i].id == id {
                let removed = state.midi_playbacks.swap_remove(i);
                let _ = removed.control.send(MidiControl::Stop);
                removed.sink.stop();
                Self::evict_unreserved_unused_locked(&mut state, now);
                return true;
            }
            i += 1;
        }
        if let Some(key) = state.built_in_midi_notes.remove(&id)
            && let Some(mixer) = state
                .built_in_midi_mixers
                .iter()
                .find(|mixer| mixer.key == key)
        {
            let _ = mixer.control.send(MidiMixerControl::Release { id });
            return true;
        }
        if let Some(key) = state.soundfont_midi_notes.remove(&id)
            && let Some(mixer) = state
                .soundfont_midi_mixers
                .iter()
                .find(|mixer| mixer.key == key)
        {
            let _ = mixer.control.send(SoundFontMixerControl::Release { id });
            return true;
        }
        false
    }

    pub fn update_spatial(&self, id: u64, params: SpatialAudioParams) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let master_volume = state.master_volume.max(0.0);
        let Some(index) = state.playbacks.iter().position(|p| p.id == id) else {
            return Self::update_midi_spatial_locked(&mut state, id, params);
        };
        let playback_bus_id = state.playbacks[index].bus_id;
        let bus_volume = playback_bus_id
            .and_then(|bus_id| state.buses.get(&bus_id))
            .map(|bus| bus.volume.max(0.0))
            .unwrap_or(1.0);
        let playback = &mut state.playbacks[index];
        playback.base_volume = params.volume.max(0.0);
        playback.pan = params.pan.clamped();
        playback.dsp.update_spatial(params);
        playback
            .sink
            .set_emitter_position(Self::pan_emitter_position(playback.pan));
        playback
            .sink
            .set_volume(playback.base_volume * master_volume * bus_volume);
        true
    }

    fn update_midi_spatial_locked(
        state: &mut AudioState,
        id: u64,
        params: SpatialAudioParams,
    ) -> bool {
        let master_volume = state.master_volume.max(0.0);
        let Some(index) = state.midi_playbacks.iter().position(|p| p.id == id) else {
            return Self::update_midi_note_mixer_spatial_locked(state, id, params, master_volume);
        };
        let playback_bus_id = state.midi_playbacks[index].bus_id;
        let bus_volume = playback_bus_id
            .and_then(|bus_id| state.buses.get(&bus_id))
            .map(|bus| bus.volume.max(0.0))
            .unwrap_or(1.0);
        let playback = &mut state.midi_playbacks[index];
        playback.base_volume = params.volume.max(0.0);
        playback.pan = params.pan.clamped();
        playback.dsp.update_spatial(params);
        playback
            .sink
            .set_emitter_position(Self::pan_emitter_position(playback.pan));
        playback
            .sink
            .set_volume(playback.base_volume * master_volume * bus_volume);
        true
    }

    fn update_midi_note_mixer_spatial_locked(
        state: &mut AudioState,
        id: u64,
        params: SpatialAudioParams,
        master_volume: f32,
    ) -> bool {
        if let Some(key) = state.built_in_midi_notes.get(&id).copied()
            && let Some(index) = state
                .built_in_midi_mixers
                .iter()
                .position(|mixer| mixer.key == key)
        {
            let bus_volume = state.built_in_midi_mixers[index]
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.volume.max(0.0))
                .unwrap_or(1.0);
            let mixer = &mut state.built_in_midi_mixers[index];
            mixer.base_volume = params.volume.max(0.0);
            mixer.dsp.update_spatial(params);
            mixer
                .sink
                .set_emitter_position(Self::pan_emitter_position(params.pan.clamped()));
            mixer
                .sink
                .set_volume(mixer.base_volume * master_volume * bus_volume);
            return true;
        }
        if let Some(key) = state.soundfont_midi_notes.get(&id).copied()
            && let Some(index) = state
                .soundfont_midi_mixers
                .iter()
                .position(|mixer| mixer.key == key)
        {
            let bus_volume = state.soundfont_midi_mixers[index]
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.volume.max(0.0))
                .unwrap_or(1.0);
            let mixer = &mut state.soundfont_midi_mixers[index];
            mixer.base_volume = params.volume.max(0.0);
            mixer.dsp.update_spatial(params);
            mixer
                .sink
                .set_emitter_position(Self::pan_emitter_position(params.pan.clamped()));
            mixer
                .sink
                .set_volume(mixer.base_volume * master_volume * bus_volume);
            return true;
        }
        false
    }

    pub fn stop_all(&self) {
        if let Ok(mut state) = self.state.lock() {
            let now = Instant::now();
            while !state.playbacks.is_empty() {
                let playback = Self::remove_playback_locked(&mut state, 0, now);
                playback.sink.stop();
            }
            while !state.midi_playbacks.is_empty() {
                let playback = state.midi_playbacks.swap_remove(0);
                let _ = playback.control.send(MidiControl::Stop);
                playback.sink.stop();
            }
            while !state.built_in_midi_mixers.is_empty() {
                let playback = state.built_in_midi_mixers.swap_remove(0);
                let _ = playback.control.send(MidiMixerControl::Stop);
                playback.sink.stop();
            }
            state.built_in_midi_mixer_index.clear();
            state.built_in_midi_notes.clear();
            while !state.soundfont_midi_mixers.is_empty() {
                let playback = state.soundfont_midi_mixers.swap_remove(0);
                let _ = playback.control.send(SoundFontMixerControl::Stop);
                playback.sink.stop();
            }
            state.soundfont_midi_mixer_index.clear();
            state.soundfont_midi_notes.clear();
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
        for playback in &state.midi_playbacks {
            if playback.bus_id == Some(bus_id) {
                playback.sink.pause();
            }
        }
        for playback in &state.built_in_midi_mixers {
            if playback.bus_id == Some(bus_id) {
                playback.sink.pause();
            }
        }
        for playback in &state.soundfont_midi_mixers {
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
        for playback in &state.midi_playbacks {
            if playback.bus_id == Some(bus_id) {
                playback.sink.play();
            }
        }
        for playback in &state.built_in_midi_mixers {
            if playback.bus_id == Some(bus_id) {
                playback.sink.play();
            }
        }
        for playback in &state.soundfont_midi_mixers {
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
        let mut i = 0usize;
        while i < state.midi_playbacks.len() {
            if state.midi_playbacks[i].bus_id == Some(bus_id) {
                let removed = state.midi_playbacks.swap_remove(i);
                let _ = removed.control.send(MidiControl::Stop);
                removed.sink.stop();
                removed_any = true;
            } else {
                i += 1;
            }
        }
        let mut i = 0usize;
        while i < state.built_in_midi_mixers.len() {
            if state.built_in_midi_mixers[i].bus_id == Some(bus_id) {
                let removed = Self::remove_built_in_midi_mixer_locked(&mut state, i);
                let _ = removed.control.send(MidiMixerControl::Stop);
                removed.sink.stop();
                state
                    .built_in_midi_notes
                    .retain(|_, key| key.bus_id != Some(bus_id));
                removed_any = true;
            } else {
                i += 1;
            }
        }
        let mut i = 0usize;
        while i < state.soundfont_midi_mixers.len() {
            if state.soundfont_midi_mixers[i].bus_id == Some(bus_id) {
                let removed = Self::remove_soundfont_midi_mixer_locked(&mut state, i);
                let _ = removed.control.send(SoundFontMixerControl::Stop);
                removed.sink.stop();
                state
                    .soundfont_midi_notes
                    .retain(|_, key| key.bus_id != Some(bus_id));
                removed_any = true;
            } else {
                i += 1;
            }
        }
        Self::evict_unreserved_unused_locked(&mut state, Instant::now());
        removed_any
    }

    pub fn load_soundfont(&self, id: perro_ids::SoundFontID, source: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let _ = Self::get_or_load_soundfont_locked(&mut state, id, source)?;
        Ok(())
    }

    pub fn load_soundfont_bytes(
        &self,
        id: perro_ids::SoundFontID,
        source: &str,
        bytes: Arc<[u8]>,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let mut cursor = Cursor::new(bytes);
        let font =
            Arc::new(rustysynth::SoundFont::new(&mut cursor).map_err(|err| err.to_string())?);
        state.soundfonts.insert(
            id,
            CachedSoundFont {
                source: Arc::from(source),
                font,
            },
        );
        Ok(())
    }

    pub fn load_midi_file(&self, source: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let _ = Self::get_or_load_midi_file_bytes_locked(&mut state, source)?;
        let _ = Self::get_or_parse_built_in_midi_locked(&mut state, source)?;
        Ok(())
    }

    pub fn play_midi_note(&self, request: MidiNoteRequest) -> Result<(), String> {
        if matches!(request.options.sound, MidiSound::BuiltIn) {
            return self.play_built_in_midi_note(request);
        }
        self.play_soundfont_midi_note(request)
    }

    /// Spatial notes get a dedicated sink + finite source registered under the
    /// request id, so `update_spatial`/`stop_playback`/`release_midi` address
    /// this note alone. Mixer-shared sinks can't pan per note.
    pub fn play_midi_note_spatial(&self, request: MidiNoteRequest) -> Result<(), String> {
        let (control, rx) = crossbeam_channel::unbounded();
        let pan = request.options.pan.clamped();
        let sink = SpatialSink::try_new(
            &self.handle,
            Self::pan_emitter_position(pan),
            [-1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        )
        .map_err(|err| format!("failed to create sink: {err}"))?;
        let dsp = DspControl::new(DspParams::dry());
        match request.options.sound {
            MidiSound::BuiltIn => {
                // Voice volume is carried by the sink (spatial updates rewrite
                // it); keep the source itself at unit gain to avoid squaring.
                let mut source_request = request;
                source_request.options.volume = 1.0;
                sink.append(DspSource::new(
                    BuiltInMidiSource::note(source_request, rx).convert_samples::<f32>(),
                    dsp.clone(),
                ));
            }
            MidiSound::SoundFont(id) => {
                let font = {
                    let state = self
                        .state
                        .lock()
                        .map_err(|_| "audio mutex poisoned".to_string())?;
                    Self::get_soundfont_locked(&state, id)?.1
                };
                sink.append(DspSource::new(
                    crate::midi::RustyNoteSource::new(font, &request, rx)?.convert_samples::<f32>(),
                    dsp.clone(),
                ));
            }
        }
        self.activate_midi_sink(MidiSinkActivation {
            id: request.id,
            source: None,
            bus_id: request.options.bus_id,
            volume: request.options.volume,
            pan,
            control,
            dsp,
            sink,
        })
    }

    fn play_built_in_midi_note(&self, request: MidiNoteRequest) -> Result<(), String> {
        let pan = request.options.pan.clamped();
        let key = MidiMixerKey::new(request.options.bus_id, pan);
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        Self::prune_finished_playbacks_locked(&mut state, Instant::now());
        Self::prune_finished_midi_locked(&mut state);

        let mixer_index = if let Some(index) = state.built_in_midi_mixer_index.get(&key).copied() {
            index
        } else {
            let (control, rx) = crossbeam_channel::unbounded();
            let sink = SpatialSink::try_new(
                &self.handle,
                Self::pan_emitter_position(key.pan()),
                [-1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )
            .map_err(|err| format!("failed to create sink: {err}"))?;
            let dsp = DspControl::new(DspParams::dry());
            sink.append(DspSource::new(
                BuiltInMidiMixerSource::new(rx).convert_samples::<f32>(),
                dsp.clone(),
            ));

            let master_volume = state.master_volume.max(0.0);
            let (bus_volume, bus_speed, bus_paused) =
                match request.options.bus_id.and_then(|id| state.buses.get(&id)) {
                    Some(bus_state) => (
                        bus_state.volume.max(0.0),
                        bus_state.speed.max(0.01),
                        bus_state.paused,
                    ),
                    None => (1.0, 1.0, false),
                };
            sink.set_speed(bus_speed);
            sink.set_volume(master_volume * bus_volume);
            if bus_paused {
                sink.pause();
            } else {
                sink.play();
            }
            state.built_in_midi_mixers.push(BuiltInMidiMixerPlayback {
                key,
                bus_id: request.options.bus_id,
                base_volume: 1.0,
                dsp,
                control,
                sink,
            });
            let index = state.built_in_midi_mixers.len() - 1;
            state.built_in_midi_mixer_index.insert(key, index);
            index
        };

        let note = MidiMixerNote {
            id: request.id,
            note: request.note,
            velocity: request.options.velocity,
            sustain: request.options.sustain,
            held: request.held,
            program: request.options.program,
            volume: request.options.volume,
        };
        state.built_in_midi_mixers[mixer_index]
            .control
            .send(MidiMixerControl::Note(note))
            .map_err(|_| "failed to queue midi note".to_string())?;
        if request.held {
            state.built_in_midi_notes.insert(request.id, key);
        }
        Ok(())
    }

    fn play_soundfont_midi_note(&self, request: MidiNoteRequest) -> Result<(), String> {
        let MidiSound::SoundFont(id) = request.options.sound else {
            return Ok(());
        };
        let pan = request.options.pan.clamped();
        let key = SoundFontMidiMixerKey::new(id, request.options.bus_id, pan);
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        Self::prune_finished_playbacks_locked(&mut state, Instant::now());
        Self::prune_finished_midi_locked(&mut state);
        let (_, font) = Self::get_soundfont_locked(&state, id)?;

        let mixer_index = if let Some(index) = state.soundfont_midi_mixer_index.get(&key).copied() {
            index
        } else {
            let (control, rx) = crossbeam_channel::unbounded();
            let sink = SpatialSink::try_new(
                &self.handle,
                Self::pan_emitter_position(key.pan()),
                [-1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )
            .map_err(|err| format!("failed to create sink: {err}"))?;
            let dsp = DspControl::new(DspParams::dry());
            sink.append(DspSource::new(
                RustyNoteMixerSource::new(font, rx)?.convert_samples::<f32>(),
                dsp.clone(),
            ));

            let master_volume = state.master_volume.max(0.0);
            let (bus_volume, bus_speed, bus_paused) =
                match request.options.bus_id.and_then(|id| state.buses.get(&id)) {
                    Some(bus_state) => (
                        bus_state.volume.max(0.0),
                        bus_state.speed.max(0.01),
                        bus_state.paused,
                    ),
                    None => (1.0, 1.0, false),
                };
            sink.set_speed(bus_speed);
            sink.set_volume(master_volume * bus_volume);
            if bus_paused {
                sink.pause();
            } else {
                sink.play();
            }
            state
                .soundfont_midi_mixers
                .push(SoundFontMidiMixerPlayback {
                    key,
                    bus_id: request.options.bus_id,
                    base_volume: 1.0,
                    dsp,
                    control,
                    sink,
                });
            let index = state.soundfont_midi_mixers.len() - 1;
            state.soundfont_midi_mixer_index.insert(key, index);
            index
        };

        let note = SoundFontMixerNote {
            id: request.id,
            note: request.note,
            velocity: request.options.velocity,
            sustain: request.options.sustain,
            held: request.held,
            channel: request.options.channel,
            program: request.options.program,
        };
        state.soundfont_midi_mixers[mixer_index]
            .control
            .send(SoundFontMixerControl::Note(note))
            .map_err(|_| "failed to queue soundfont midi note".to_string())?;
        state.soundfont_midi_notes.insert(request.id, key);
        Ok(())
    }

    pub fn play_midi_file(&self, request: MidiFileRequest<'_>) -> Result<(), String> {
        let (bytes, built_in_data, soundfont) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "audio mutex poisoned".to_string())?;
            Self::prune_finished_playbacks_locked(&mut state, Instant::now());
            Self::prune_finished_midi_locked(&mut state);
            let bytes = Self::get_or_load_midi_file_bytes_locked(&mut state, request.song.source)?;
            let built_in_data = match request.song.sound {
                MidiSound::BuiltIn => Some(Self::get_or_parse_built_in_midi_locked(
                    &mut state,
                    request.song.source,
                )?),
                MidiSound::SoundFont(_) => None,
            };
            let soundfont = match request.song.sound {
                MidiSound::BuiltIn => None,
                MidiSound::SoundFont(id) => Some(Self::get_soundfont_locked(&state, id)?.1),
            };
            (bytes, built_in_data, soundfont)
        };
        let (control, rx) = crossbeam_channel::unbounded();
        let pan = request.pan.clamped();
        let sink = SpatialSink::try_new(
            &self.handle,
            Self::pan_emitter_position(pan),
            [-1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        )
        .map_err(|err| format!("failed to create sink: {err}"))?;
        let dsp = DspControl::new(DspParams::dry());
        if let Some(font) = soundfont {
            sink.append(DspSource::new(
                RustyFileSource::new(font, &bytes, request.song.looped, rx)?
                    .convert_samples::<f32>(),
                dsp.clone(),
            ));
        } else if let Some(data) = built_in_data {
            sink.append(DspSource::new(
                BuiltInMidiSource::file_data(data, request.song, rx).convert_samples::<f32>(),
                dsp.clone(),
            ));
        } else {
            sink.append(DspSource::new(
                BuiltInMidiSource::file(&bytes, request.song, rx)?.convert_samples::<f32>(),
                dsp.clone(),
            ));
        }
        self.activate_midi_sink(MidiSinkActivation {
            id: request.id,
            source: Some(Arc::from(request.song.source)),
            bus_id: request.song.bus_id,
            volume: request.song.volume,
            pan,
            control,
            dsp,
            sink,
        })
    }

    pub fn release_midi(&self, id: u64) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        Self::prune_finished_midi_locked(&mut state);
        if let Some(playback) = state.midi_playbacks.iter().find(|p| p.id == id) {
            return playback.control.send(MidiControl::Release).is_ok();
        }
        if let Some(key) = state.built_in_midi_notes.remove(&id)
            && let Some(mixer) = state
                .built_in_midi_mixers
                .iter()
                .find(|mixer| mixer.key == key)
        {
            return mixer.control.send(MidiMixerControl::Release { id }).is_ok();
        }
        false
    }

    fn activate_midi_sink(&self, activation: MidiSinkActivation) -> Result<(), String> {
        let MidiSinkActivation {
            id,
            source,
            bus_id,
            volume,
            pan,
            control,
            dsp,
            sink,
        } = activation;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let master_volume = state.master_volume.max(0.0);
        let (bus_volume, bus_speed, bus_paused) = match bus_id.and_then(|id| state.buses.get(&id)) {
            Some(bus_state) => (
                bus_state.volume.max(0.0),
                bus_state.speed.max(0.01),
                bus_state.paused,
            ),
            None => (1.0, 1.0, false),
        };
        sink.set_speed(bus_speed);
        sink.set_volume(volume.max(0.0) * master_volume * bus_volume);
        if bus_paused {
            sink.pause();
        } else {
            sink.play();
        }
        state.midi_playbacks.push(MidiPlayback {
            id,
            bus_id,
            base_volume: volume.max(0.0),
            pan,
            dsp,
            source,
            control,
            sink,
        });
        Ok(())
    }

    fn get_or_load_asset_locked(
        state: &mut AudioState,
        source: &str,
        reserved: bool,
        static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
    ) -> Result<LoadedAudioAsset, String> {
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
                pcm: None,
                pcm_oversized: false,
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

    fn insert_audio_bytes_locked(
        state: &mut AudioState,
        source: &str,
        bytes: Arc<[u8]>,
        reserved: bool,
    ) -> Result<(), String> {
        let source_hash = perro_ids::string_to_u64(source);
        if let Some(existing) = state.cache.get(&source_hash)
            && existing.source.as_ref() != source
        {
            return Err(format!(
                "audio source hash collision: `{}` conflicts with `{source}`",
                existing.source
            ));
        }
        if let Some(old) = state.cache.remove(&source_hash) {
            state.cache_bytes = state.cache_bytes.saturating_sub(old.cache_len());
        }
        let asset_epoch = state.next_cache_epoch.max(1);
        state.next_cache_epoch = state.next_cache_epoch.wrapping_add(1).max(1);
        state.cache_bytes = state.cache_bytes.saturating_add(bytes.len());
        state.cache.insert(
            source_hash,
            CachedAudioAsset {
                source: Arc::from(source),
                source_hash,
                asset_epoch,
                bytes,
                duration: None,
                duration_known: false,
                reserved,
                active_uses: 0,
                last_touched: Instant::now(),
                pcm: None,
                pcm_oversized: false,
            },
        );
        Ok(())
    }

    // Decode the full clip to f32 PCM and cache it when it fits the cap.
    // Returns None (and marks the entry oversized) when the clip is too long,
    // so the caller falls back to streaming decode.
    fn decode_and_cache_pcm(
        &self,
        bytes: &Arc<[u8]>,
        source_hash: u64,
        source: &str,
    ) -> Result<Option<Arc<CachedPcm>>, String> {
        let cursor = Cursor::new(bytes.clone());
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader)
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;
        let channels = decoder.channels().max(1);
        let sample_rate = decoder.sample_rate().max(1);
        let cap = (sample_rate as usize)
            .saturating_mul(channels as usize)
            .saturating_mul(Self::PCM_CACHE_MAX_SECONDS);
        let mut samples: Vec<f32> = Vec::new();
        let mut oversized = false;
        for sample in decoder.convert_samples::<f32>() {
            if samples.len() >= cap {
                oversized = true;
                break;
            }
            samples.push(sample);
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        if oversized {
            if let Some(entry) = state.cache.get_mut(&source_hash) {
                entry.pcm_oversized = true;
            }
            return Ok(None);
        }
        let pcm = Arc::new(CachedPcm {
            channels,
            sample_rate,
            samples: Arc::from(samples.into_boxed_slice()),
        });
        let stored = if let Some(entry) = state.cache.get_mut(&source_hash) {
            entry.pcm = Some(pcm.clone());
            entry.duration = Some(pcm.duration());
            entry.duration_known = true;
            true
        } else {
            false
        };
        if stored {
            state.cache_bytes = state.cache_bytes.saturating_add(pcm.byte_len());
        }
        Ok(Some(pcm))
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
        if let Some(entry) = state.cache.get_mut(&removed.source_hash)
            && entry.asset_epoch == removed.asset_epoch
        {
            entry.active_uses = entry.active_uses.saturating_sub(1);
            entry.last_touched = now;
        }
        removed
    }

    fn remove_built_in_midi_mixer_locked(
        state: &mut AudioState,
        index: usize,
    ) -> BuiltInMidiMixerPlayback {
        let removed = state.built_in_midi_mixers.swap_remove(index);
        state.built_in_midi_mixer_index.remove(&removed.key);
        if index < state.built_in_midi_mixers.len() {
            let moved_key = state.built_in_midi_mixers[index].key;
            state.built_in_midi_mixer_index.insert(moved_key, index);
        }
        removed
    }

    fn remove_soundfont_midi_mixer_locked(
        state: &mut AudioState,
        index: usize,
    ) -> SoundFontMidiMixerPlayback {
        let removed = state.soundfont_midi_mixers.swap_remove(index);
        state.soundfont_midi_mixer_index.remove(&removed.key);
        if index < state.soundfont_midi_mixers.len() {
            let moved_key = state.soundfont_midi_mixers[index].key;
            state.soundfont_midi_mixer_index.insert(moved_key, index);
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

    fn prune_finished_midi_locked(state: &mut AudioState) {
        let mut i = 0usize;
        while i < state.midi_playbacks.len() {
            if state.midi_playbacks[i].sink.empty() {
                state.midi_playbacks.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn get_or_load_soundfont_locked(
        state: &mut AudioState,
        id: perro_ids::SoundFontID,
        source: &str,
    ) -> Result<Arc<rustysynth::SoundFont>, String> {
        if let Some(existing) = state.soundfonts.get(&id) {
            if existing.source.as_ref() != source {
                return Err(format!(
                    "soundfont source hash collision: `{}` conflicts with `{source}`",
                    existing.source
                ));
            }
            return Ok(existing.font.clone());
        }
        let bytes = perro_io::load_asset(source).map_err(|err| err.to_string())?;
        let mut cursor = Cursor::new(bytes);
        let font =
            Arc::new(rustysynth::SoundFont::new(&mut cursor).map_err(|err| err.to_string())?);
        state.soundfonts.insert(
            id,
            CachedSoundFont {
                source: Arc::from(source),
                font: font.clone(),
            },
        );
        Ok(font)
    }

    fn get_soundfont_locked(
        state: &AudioState,
        id: perro_ids::SoundFontID,
    ) -> Result<(Arc<str>, Arc<rustysynth::SoundFont>), String> {
        state
            .soundfonts
            .get(&id)
            .map(|font| (font.source.clone(), font.font.clone()))
            .ok_or_else(|| format!("soundfont not loaded: {id}"))
    }

    fn get_or_load_midi_file_bytes_locked(
        state: &mut AudioState,
        source: &str,
    ) -> Result<Arc<[u8]>, String> {
        let source_hash = perro_ids::string_to_u64(source);
        if let Some(existing) = state.midi_files.get(&source_hash) {
            if existing.source.as_ref() != source {
                return Err(format!(
                    "midi source hash collision: `{}` conflicts with `{source}`",
                    existing.source
                ));
            }
            return Ok(existing.bytes.clone());
        }
        let bytes: Arc<[u8]> =
            Arc::from(perro_io::load_asset(source).map_err(|err| err.to_string())?);
        state.midi_files.insert(
            source_hash,
            CachedMidiFile {
                source: Arc::from(source),
                bytes: bytes.clone(),
                built_in: None,
            },
        );
        Ok(bytes)
    }

    fn get_or_parse_built_in_midi_locked(
        state: &mut AudioState,
        source: &str,
    ) -> Result<Arc<crate::midi::BuiltInMidiFileData>, String> {
        let source_hash = perro_ids::string_to_u64(source);
        if !state.midi_files.contains_key(&source_hash) {
            let _ = Self::get_or_load_midi_file_bytes_locked(state, source)?;
        }
        let entry = state
            .midi_files
            .get_mut(&source_hash)
            .ok_or_else(|| format!("midi source missing after load: `{source}`"))?;
        if entry.source.as_ref() != source {
            return Err(format!(
                "midi source hash collision: `{}` conflicts with `{source}`",
                entry.source
            ));
        }
        if let Some(parsed) = &entry.built_in {
            return Ok(parsed.clone());
        }
        let parsed = parse_built_in_midi_file(&entry.bytes)?;
        entry.built_in = Some(parsed.clone());
        Ok(parsed)
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
                removed_bytes = removed_bytes.saturating_add(entry.cache_len());
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
            cache_bytes = cache_bytes.saturating_sub(entry.cache_len());
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
        for playback in &state.midi_playbacks {
            let bus_volume = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.volume.max(0.0))
                .unwrap_or(1.0);
            playback
                .sink
                .set_volume(playback.base_volume * state.master_volume.max(0.0) * bus_volume);
        }
        for playback in &state.built_in_midi_mixers {
            let bus_volume = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.volume.max(0.0))
                .unwrap_or(1.0);
            playback
                .sink
                .set_volume(playback.base_volume * state.master_volume.max(0.0) * bus_volume);
        }
        for playback in &state.soundfont_midi_mixers {
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
        for playback in &state.midi_playbacks {
            let bus_speed = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.speed.max(0.01))
                .unwrap_or(1.0);
            playback.sink.set_speed(bus_speed);
        }
        for playback in &state.built_in_midi_mixers {
            let bus_speed = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.speed.max(0.01))
                .unwrap_or(1.0);
            playback.sink.set_speed(bus_speed);
        }
        for playback in &state.soundfont_midi_mixers {
            let bus_speed = playback
                .bus_id
                .and_then(|bus_id| state.buses.get(&bus_id))
                .map(|bus| bus.speed.max(0.01))
                .unwrap_or(1.0);
            playback.sink.set_speed(bus_speed);
        }
    }

    fn pan_emitter_position(pan: AudioPan) -> [f32; 3] {
        [pan.x, pan.y, pan.z]
    }
}

// Zero-copy playback over cached decoded PCM.
struct CachedPcmSource {
    pcm: Arc<CachedPcm>,
    position: usize,
}

impl CachedPcmSource {
    fn new(pcm: Arc<CachedPcm>) -> Self {
        Self { pcm, position: 0 }
    }
}

impl Iterator for CachedPcmSource {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        let sample = self.pcm.samples.get(self.position).copied()?;
        self.position += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.pcm.samples.len().saturating_sub(self.position);
        (remaining, Some(remaining))
    }
}

impl Source for CachedPcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.pcm.samples.len().saturating_sub(self.position))
    }

    fn channels(&self) -> u16 {
        self.pcm.channels
    }

    fn sample_rate(&self) -> u32 {
        self.pcm.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.pcm.duration())
    }
}

// Shared append tail for both the PCM and streaming decode paths: apply trim
// (and optional take/loop) then route through the DSP chain into the sink.
fn append_with_trims<S>(
    sink: &SpatialSink,
    source: S,
    dsp: Arc<DspControl>,
    trim_start: Duration,
    play_duration: Option<Duration>,
    looped: bool,
) where
    S: Source<Item = f32> + Send + 'static,
{
    match (play_duration, looped) {
        (Some(duration), true) => sink.append(DspSource::new(
            source
                .skip_duration(trim_start)
                .take_duration(duration)
                .repeat_infinite(),
            dsp,
        )),
        (Some(duration), false) => sink.append(DspSource::new(
            source.skip_duration(trim_start).take_duration(duration),
            dsp,
        )),
        (None, true) => sink.append(DspSource::new(
            source.skip_duration(trim_start).repeat_infinite(),
            dsp,
        )),
        (None, false) => sink.append(DspSource::new(source.skip_duration(trim_start), dsp)),
    }
}
