use super::*;

impl BarkPlayer {
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
        let source_buffer = SamplesBuffer::new(clip.channels(), clip.sample_rate(), samples);
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

    /// Append a microphone packet to its named stream instead of starting it
    /// immediately. This preserves packet order and prevents arrival bursts
    /// from overlapping audio and changing its apparent pitch.
    pub fn play_stream_clip(
        &self,
        source: &str,
        clip: MicClip,
        bus_id: Option<AudioBusID>,
        volume: f32,
        pan: AudioPan,
    ) -> Result<(), String> {
        let pan = pan.clamped();
        let requested_volume = volume.max(0.0);
        let mut state = self
            .state
            .lock()
            .map_err(|_| "audio mutex poisoned".to_string())?;
        let now = Instant::now();
        Self::prune_finished_playbacks_locked(&mut state, now);

        let master_volume = state.master_volume.max(0.0);
        let (bus_volume, bus_speed, bus_paused) = match bus_id.and_then(|id| state.buses.get(&id)) {
            Some(bus_state) => (
                bus_state.volume.max(0.0),
                bus_state.speed.max(0.01),
                bus_state.paused,
            ),
            None => (1.0, 1.0, false),
        };
        if let Some(playback) = state
            .playbacks
            .iter_mut()
            .find(|playback| playback.source.as_ref() == source && playback.bus_id == bus_id)
        {
            let samples = clip.samples_f32();
            let source_buffer = SamplesBuffer::new(clip.channels(), clip.sample_rate(), samples);
            playback
                .sink
                .append(DspSource::new(source_buffer, Arc::clone(&playback.dsp)));
            playback.base_volume = requested_volume;
            playback.pan = pan;
            playback
                .sink
                .set_emitter_position(Self::pan_emitter_position(pan));
            playback.sink.set_speed(bus_speed);
            playback
                .sink
                .set_volume(requested_volume * master_volume * bus_volume);
            if bus_paused {
                playback.sink.pause();
            } else {
                playback.sink.play();
            }
            return Ok(());
        }
        drop(state);
        self.play_clip(source, clip, bus_id, volume, pan)
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

    pub(super) fn update_midi_spatial_locked(
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

    pub(super) fn update_midi_note_mixer_spatial_locked(
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
}
