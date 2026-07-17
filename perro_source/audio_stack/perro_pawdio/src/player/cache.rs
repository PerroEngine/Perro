use super::*;

impl BarkPlayer {
    pub(super) fn get_or_load_asset_locked(
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

    pub(super) fn insert_audio_bytes_locked(
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
    pub(super) fn decode_and_cache_pcm(
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

    pub(super) fn duration_for_source_locked(
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

    pub(super) fn remove_playback_locked(
        state: &mut AudioState,
        index: usize,
        now: Instant,
    ) -> Playback {
        let removed = state.playbacks.swap_remove(index);
        if let Some(entry) = state.cache.get_mut(&removed.source_hash)
            && entry.asset_epoch == removed.asset_epoch
        {
            entry.active_uses = entry.active_uses.saturating_sub(1);
            entry.last_touched = now;
        }
        removed
    }

    pub(super) fn remove_built_in_midi_mixer_locked(
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

    pub(super) fn remove_soundfont_midi_mixer_locked(
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

    pub(super) fn prune_finished_playbacks_locked(state: &mut AudioState, now: Instant) {
        let mut i = 0usize;
        while i < state.playbacks.len() {
            if state.playbacks[i].sink.empty() {
                let _ = Self::remove_playback_locked(state, i, now);
            } else {
                i += 1;
            }
        }
    }

    pub(super) fn prune_finished_midi_locked(state: &mut AudioState) {
        let mut i = 0usize;
        while i < state.midi_playbacks.len() {
            if state.midi_playbacks[i].sink.empty() {
                state.midi_playbacks.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub(super) fn get_or_load_soundfont_locked(
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

    pub(super) fn get_soundfont_locked(
        state: &AudioState,
        id: perro_ids::SoundFontID,
    ) -> Result<(Arc<str>, Arc<rustysynth::SoundFont>), String> {
        state
            .soundfonts
            .get(&id)
            .map(|font| (font.source.clone(), font.font.clone()))
            .ok_or_else(|| format!("soundfont not loaded: {id}"))
    }

    pub(super) fn get_or_load_midi_file_bytes_locked(
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

    pub(super) fn get_or_parse_built_in_midi_locked(
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

    pub(super) fn unreserved_ttl(entry: &CachedAudioAsset) -> Duration {
        if let Some(duration) = entry.duration {
            let scaled =
                Duration::from_secs_f32(duration.as_secs_f32() * Self::UNRESERVED_TTL_FACTOR);
            return scaled.max(Self::UNRESERVED_TTL_MIN);
        }
        Self::UNRESERVED_TTL_FALLBACK
    }

    pub(super) fn evict_unreserved_unused_locked(state: &mut AudioState, now: Instant) {
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

    pub(super) fn enforce_cache_soft_limit_locked(state: &mut AudioState) {
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

    pub(super) fn refresh_volumes(state: &mut AudioState) {
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

    pub(super) fn refresh_speeds(state: &mut AudioState) {
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

    pub(super) fn pan_emitter_position(pan: AudioPan) -> [f32; 3] {
        [pan.x, pan.y, pan.z]
    }
}
