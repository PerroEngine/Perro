use super::*;

impl BarkPlayer {
    fn cached_asset_locked(
        state: &mut AudioState,
        source: &str,
        reserved: bool,
    ) -> Result<Option<LoadedAudioAsset>, String> {
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
            return Ok(Some((
                existing.bytes.clone(),
                existing.source.clone(),
                existing.source_hash,
                existing.asset_epoch,
                true,
                SourceLoadStats::cache_hit(),
            )));
        }
        Ok(None)
    }

    pub(super) fn get_or_load_asset(
        state: &Mutex<AudioState>,
        source: &str,
        reserved: bool,
        static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
    ) -> Result<LoadedAudioAsset, String> {
        Self::get_or_load_asset_with(state, source, reserved, |source_hash| {
            Self::load_audio_bytes(source, source_hash, static_audio_lookup)
        })
    }

    fn get_or_load_asset_with(
        state_mutex: &Mutex<AudioState>,
        source: &str,
        reserved: bool,
        load: impl FnOnce(u64) -> Result<(Arc<[u8]>, SourceLoadStats), String>,
    ) -> Result<LoadedAudioAsset, String> {
        let source_hash = perro_ids::string_to_u64(source);
        let (source_key, asset_epoch) = {
            let mut state = state_mutex
                .lock()
                .map_err(|_| "audio mutex poisoned".to_string())?;
            if let Some(hit) = Self::cached_asset_locked(&mut state, source, reserved)? {
                return Ok(hit);
            }
            let source_key: Arc<str> = Arc::from(source);
            let epoch = state.next_cache_epoch.max(1);
            state.next_cache_epoch = epoch.wrapping_add(1).max(1);
            let pending = state
                .pending_audio_loads
                .entry(source_key.clone())
                .or_insert((epoch, 0));
            pending.1 += 1;
            (source_key, pending.0)
        };

        // Disk/archive IO, static lookup/decompression and the byte copy all run
        // outside the player-state mutex. Concurrent misses may share a winner.
        let loaded = load(source_hash);
        let mut state = state_mutex
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let valid = if let Some(pending) = state.pending_audio_loads.get_mut(source)
            && pending.0 == asset_epoch
        {
            pending.1 -= 1;
            if pending.1 == 0 {
                state.pending_audio_loads.remove(source);
            }
            true
        } else {
            false
        };
        // Prefer a concurrent load/reload over stale bytes, including when this
        // IO attempt fails. Never resurrect a source dropped during this load.
        if let Some(hit) = Self::cached_asset_locked(&mut state, source, reserved)? {
            return Ok(hit);
        }
        if !valid {
            return Err(format!("audio asset `{source}` dropped during load"));
        }
        let (shared, load_stats) = loaded?;
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

    fn load_audio_bytes(
        source: &str,
        source_hash: u64,
        static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
    ) -> Result<(Arc<[u8]>, SourceLoadStats), String> {
        // Cow keeps uncompressed static blobs borrowed until the single
        // Arc::from below, so that path copies once instead of twice.
        let (bytes, load_stats): (std::borrow::Cow<'_, [u8]>, _) =
            if let Some(lookup) = static_audio_lookup {
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
                (std::borrow::Cow::Owned(disk), stats)
            };
        Ok((Arc::from(bytes), load_stats))
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
        asset_epoch: u64,
        source: &str,
    ) -> Result<Option<Arc<CachedPcm>>, String> {
        // The bytes are already resident, so `Cursor` is the reader; a
        // `BufReader` on top would copy every block twice.
        let decoder = Decoder::new(Cursor::new(bytes.clone()))
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;
        let channels = decoder.channels().max(1);
        // Resample once, here, into the device rate. Cached PCM used to be
        // stored at its native rate, so every play ran rodio's per-sample rate
        // converter over the whole clip again.
        let sample_rate = self.output_sample_rate.max(1);
        let cap = (sample_rate as usize)
            .saturating_mul(channels as usize)
            .saturating_mul(Self::PCM_CACHE_MAX_SECONDS);
        // Preallocate up to the cap (bounded by a sanity ceiling in case the
        // header reports an absurd rate/channel count) so the decode loop does
        // not realloc-grow through megabytes of PCM.
        const PCM_PREALLOC_CEILING_SAMPLES: usize = 1 << 21;
        let mut samples: Vec<f32> = Vec::with_capacity(cap.min(PCM_PREALLOC_CEILING_SAMPLES));
        let mut oversized = false;
        let resampled = UniformSourceIterator::<_, f32>::new(
            decoder.convert_samples::<f32>(),
            channels,
            sample_rate,
        );
        for sample in resampled {
            if samples.len() >= cap {
                oversized = true;
                break;
            }
            samples.push(sample);
        }
        // Keep whole frames: a rate conversion can end mid-frame, and the
        // duration/trim math assumes `len % channels == 0`.
        samples.truncate(samples.len() - samples.len() % channels as usize);

        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        if oversized {
            if let Some(entry) = state.cache.get_mut(&source_hash)
                && entry.asset_epoch == asset_epoch
            {
                entry.pcm_oversized = true;
            }
            return Ok(None);
        }
        let pcm = Arc::new(CachedPcm {
            channels,
            sample_rate,
            samples: Arc::from(samples.into_boxed_slice()),
        });
        let stored = if let Some(entry) = state.cache.get_mut(&source_hash)
            && entry.asset_epoch == asset_epoch
        {
            // A concurrent decoder may already publish this epoch's PCM.
            if let Some(existing) = &entry.pcm {
                return Ok(Some(existing.clone()));
            }
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
        Self::prune_idle_midi_mixers_locked(state, Instant::now());
    }

    // Note mixers are keyed by bus + quantized pan, so they outlive the notes
    // that created them. Drop the ones whose notes have all finished: each one
    // pins a sink, an unbounded control channel and a mixer source.
    pub(super) fn prune_idle_midi_mixers_locked(state: &mut AudioState, now: Instant) {
        let mut i = 0usize;
        while i < state.built_in_midi_mixers.len() {
            let key = state.built_in_midi_mixers[i].key;
            let tracked = state.built_in_midi_notes.values().any(|held| *held == key);
            if midi_mixer_is_idle(state.built_in_midi_mixers[i].busy_until, now, tracked) {
                let removed = Self::remove_built_in_midi_mixer_locked(state, i);
                let _ = removed.control.send(MidiMixerControl::Stop);
                removed.sink.stop();
            } else {
                i += 1;
            }
        }
        let mut i = 0usize;
        while i < state.soundfont_midi_mixers.len() {
            let key = state.soundfont_midi_mixers[i].key;
            let tracked = state.soundfont_midi_notes.values().any(|held| *held == key);
            if midi_mixer_is_idle(state.soundfont_midi_mixers[i].busy_until, now, tracked) {
                let removed = Self::remove_soundfont_midi_mixer_locked(state, i);
                let _ = removed.control.send(SoundFontMixerControl::Stop);
                removed.sink.stop();
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
        let source_bytes = bytes.len();
        let mut cursor = Cursor::new(bytes);
        let font =
            Arc::new(rustysynth::SoundFont::new(&mut cursor).map_err(|err| err.to_string())?);
        // Fonts are pinned until shutdown (no unload API, playback errors if
        // one goes missing), so they live on their own ledger instead of the
        // evictable clip budget: charging them to `cache_bytes` would
        // permanently shrink the clip cache and force-evict hot clips.
        let footprint_bytes = CachedSoundFont::estimate_footprint(&font, source_bytes);
        state.soundfont_bytes = state.soundfont_bytes.saturating_add(footprint_bytes);
        state.soundfonts.insert(
            id,
            CachedSoundFont {
                source: Arc::from(source),
                font: font.clone(),
                footprint_bytes,
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
        if let Some(existing) = state.midi_files.get_mut(&source_hash) {
            if existing.source.as_ref() != source {
                return Err(format!(
                    "midi source hash collision: `{}` conflicts with `{source}`",
                    existing.source
                ));
            }
            existing.last_touched = Instant::now();
            return Ok(existing.bytes.clone());
        }
        let bytes: Arc<[u8]> =
            Arc::from(perro_io::load_asset(source).map_err(|err| err.to_string())?);
        // Midi file bytes pin memory like audio assets do; count them against
        // the shared budget so audio-cache eviction compensates. They reload
        // lazily from disk, so the soft-limit pass may evict idle ones.
        state.cache_bytes = state.cache_bytes.saturating_add(bytes.len());
        state.midi_files.insert(
            source_hash,
            CachedMidiFile {
                source: Arc::from(source),
                bytes: bytes.clone(),
                built_in: None,
                last_touched: Instant::now(),
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
        entry.last_touched = Instant::now();
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
        // Evict least-recently-touched first. The old `retain` walked the map
        // in hash order, which could drop a hot clip while stale ones survived.
        // Clip entries and idle midi files share one LRU order; midi files
        // reload lazily from disk, so dropping them is safe as long as no
        // active midi playback still references the source.
        let mut candidates: Vec<(u64, Instant, usize, bool)> = state
            .cache
            .iter()
            .filter(|(_, entry)| !entry.reserved && entry.active_uses == 0)
            .map(|(key, entry)| (*key, entry.last_touched, entry.cache_len(), false))
            .collect();
        if !state.midi_files.is_empty() {
            let active_midi_sources: std::collections::HashSet<u64> = state
                .midi_playbacks
                .iter()
                .filter_map(|playback| playback.source.as_deref())
                .map(perro_ids::string_to_u64)
                .collect();
            candidates.extend(
                state
                    .midi_files
                    .iter()
                    .filter(|(key, _)| !active_midi_sources.contains(key))
                    .map(|(key, entry)| (*key, entry.last_touched, entry.cache_len(), true)),
            );
        }
        candidates.sort_by_key(|(_, last_touched, _, _)| *last_touched);
        let mut cache_bytes = state.cache_bytes;
        for (key, _, len, is_midi) in candidates {
            if cache_bytes <= Self::CACHE_SOFT_LIMIT_BYTES {
                break;
            }
            if is_midi {
                state.midi_files.remove(&key);
            } else {
                state.cache.remove(&key);
            }
            cache_bytes = cache_bytes.saturating_sub(len);
        }
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

#[cfg(test)]
mod load_tests {
    use super::*;

    fn bytes() -> Result<(Arc<[u8]>, SourceLoadStats), String> {
        Ok((Arc::from(&b"old"[..]), SourceLoadStats::cache_hit()))
    }

    #[test]
    fn cold_load_releases_mutex_and_reuses_concurrent_replacement() {
        let state = Mutex::new(AudioState::empty_for_test());
        let loaded = BarkPlayer::get_or_load_asset_with(&state, "clip", true, |_| {
            let mut guard = state.try_lock().expect("IO must run outside state lock");
            BarkPlayer::insert_audio_bytes_locked(
                &mut guard,
                "clip",
                Arc::from(&b"new"[..]),
                false,
            )?;
            bytes()
        })
        .expect("test setup/result must succeed");
        assert_eq!(loaded.0.as_ref(), b"new");
        let state = state.lock().expect("test setup/result must succeed");
        assert!(state.pending_audio_loads.is_empty());
        assert_eq!(state.cache_bytes, 3);
        assert!(state.cache[&loaded.2].reserved);
        assert_eq!(state.cache[&loaded.2].asset_epoch, loaded.3);
    }

    #[test]
    fn dropped_inflight_load_does_not_resurrect_cache() {
        let state = Mutex::new(AudioState::empty_for_test());
        let result = BarkPlayer::get_or_load_asset_with(&state, "clip", false, |_| {
            state
                .try_lock()
                .expect("test setup/result must succeed")
                .pending_audio_loads
                .remove("clip");
            bytes()
        });
        assert!(result.is_err());
        let state = state.lock().expect("test setup/result must succeed");
        assert!(state.cache.is_empty());
        assert!(state.pending_audio_loads.is_empty());
        assert_eq!(state.cache_bytes, 0);
    }

    #[test]
    fn concurrent_misses_share_published_epoch_and_account_once() {
        let state = Mutex::new(AudioState::empty_for_test());
        let mut inner_epoch = 0;
        let loaded = BarkPlayer::get_or_load_asset_with(&state, "clip", false, |_| {
            inner_epoch = BarkPlayer::get_or_load_asset_with(&state, "clip", false, |_| bytes())?.3;
            bytes()
        })
        .expect("test setup/result must succeed");
        assert_eq!(loaded.3, inner_epoch);
        assert_eq!(
            state
                .lock()
                .expect("test setup/result must succeed")
                .cache_bytes,
            3
        );
        assert!(
            state
                .lock()
                .expect("test setup/result must succeed")
                .pending_audio_loads
                .is_empty()
        );
        BarkPlayer::get_or_load_asset_with(&state, "clip", false, |_| {
            panic!("cache hit must skip IO")
        })
        .expect("test setup/result must succeed");
    }

    #[test]
    fn failed_load_clears_inflight_token_for_retry() {
        let state = Mutex::new(AudioState::empty_for_test());
        assert!(
            BarkPlayer::get_or_load_asset_with(&state, "clip", false, |_| Err("IO failed".into()))
                .is_err()
        );
        assert!(
            state
                .lock()
                .expect("test setup/result must succeed")
                .pending_audio_loads
                .is_empty()
        );
        assert!(BarkPlayer::get_or_load_asset_with(&state, "clip", false, |_| bytes()).is_ok());
    }
}
