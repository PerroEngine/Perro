use super::*;

impl BarkPlayer {
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

    pub(super) fn play_built_in_midi_note(&self, request: MidiNoteRequest) -> Result<(), String> {
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

    pub(super) fn play_soundfont_midi_note(&self, request: MidiNoteRequest) -> Result<(), String> {
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

    pub(super) fn activate_midi_sink(&self, activation: MidiSinkActivation) -> Result<(), String> {
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
}
