use super::core::{
    QueuedMidiNoteOptions, QueuedMidiSong, QueuedSpatialAudio, QueuedSpatialAudioPos,
    QueuedSpatialMidi, QueuedSpatialMidiKind, RuntimeResourceApi,
};
use perro_ids::{AudioBusID, SoundFontID};
use perro_resource_api::sub_apis::{
    Audio, Audio2D, Audio3D, AudioAPI, AudioDirection, MidiNoteHandle, MidiNoteOptions, MidiSong,
    MidiSpatialPosition, Note,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::{Arc, atomic::Ordering},
};

fn bytes_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

impl AudioAPI for RuntimeResourceApi {
    fn load_audio_source(&self, source: &str) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.load_source(source)
    }

    fn create_audio_source_from_bytes(&self, bytes: &[u8]) -> Option<String> {
        if bytes.is_empty() {
            return None;
        }
        let Ok(guard) = self.bark.lock() else {
            return None;
        };
        let player = guard.as_ref()?;
        let source = format!("runtime://audio/{}", bytes_hash(bytes));
        player
            .load_source_bytes(&source, Arc::from(bytes))
            .then_some(source)
    }

    fn reserve_audio_source(&self, source: &str) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.reserve_source(source)
    }

    fn drop_audio_source(&self, source: &str) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.drop_source(source)
    }

    fn is_audio_source_loaded(&self, source: &str) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.is_source_loaded(source)
    }

    fn play_audio(
        &self,
        bus_id: Option<AudioBusID>,
        audio: Audio<'_>,
        pan: perro_resource_api::sub_apis::AudioPan,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(perro_pawdio::AudioPlaybackRequest {
            id: 0,
            source: audio.source,
            bus_id,
            looped: audio.looped,
            volume: audio.volume,
            speed: audio.effects.speed,
            pan: perro_pawdio::AudioPan {
                x: pan.x,
                y: pan.y,
                z: pan.z,
            },
            low_pass: audio.effects.low_pass,
            reverb_send: audio.effects.reverb_send,
            echo: audio.effects.echo,
            reflection: audio.effects.reflection,
            occlusion: audio.effects.occlusion,
            eq: perro_pawdio::AudioEq {
                low_gain: audio.effects.eq.low_gain,
                mid_gain: audio.effects.eq.mid_gain,
                high_gain: audio.effects.eq.high_gain,
            },
            compression: perro_pawdio::AudioCompression {
                threshold: audio.effects.compression.threshold,
                ratio: audio.effects.compression.ratio,
                attack: audio.effects.compression.attack,
                release: audio.effects.compression.release,
            },
            from_start: audio.from_start,
            from_end: audio.from_end,
        })
    }

    fn play_audio_2d(&self, bus_id: Option<AudioBusID>, audio: Audio2D<'_>) -> bool {
        let Ok(mut queue) = self.spatial_audio_queue.lock() else {
            return false;
        };
        queue.push(QueuedSpatialAudio {
            source: audio.audio.source.to_string(),
            bus_id,
            looped: audio.audio.looped,
            volume: audio.audio.volume,
            effects: audio.audio.effects,
            from_start: audio.audio.from_start,
            from_end: audio.audio.from_end,
            range: audio.range,
            audio_layer: audio.audio_layer,
            enable_propagation: audio.enable_propagation,
            pos: QueuedSpatialAudioPos::TwoD(audio.position),
            direction_2d: audio.direction.unwrap_or(AudioDirection::Omni),
            direction_3d: AudioDirection::Omni,
        });
        true
    }

    fn play_audio_3d(&self, bus_id: Option<AudioBusID>, audio: Audio3D<'_>) -> bool {
        let Ok(mut queue) = self.spatial_audio_queue.lock() else {
            return false;
        };
        queue.push(QueuedSpatialAudio {
            source: audio.audio.source.to_string(),
            bus_id,
            looped: audio.audio.looped,
            volume: audio.audio.volume,
            effects: audio.audio.effects,
            from_start: audio.audio.from_start,
            from_end: audio.audio.from_end,
            range: audio.range,
            audio_layer: audio.audio_layer,
            enable_propagation: audio.enable_propagation,
            pos: QueuedSpatialAudioPos::ThreeD(audio.position),
            direction_2d: AudioDirection::Omni,
            direction_3d: audio.direction.unwrap_or(AudioDirection::Omni),
        });
        true
    }

    fn stop_audio(
        &self,
        bus_id: Option<AudioBusID>,
        audio: Audio<'_>,
        pan: perro_resource_api::sub_apis::AudioPan,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_match(perro_pawdio::AudioPlaybackRequest {
            id: 0,
            source: audio.source,
            bus_id,
            looped: audio.looped,
            volume: audio.volume,
            speed: audio.effects.speed,
            pan: perro_pawdio::AudioPan {
                x: pan.x,
                y: pan.y,
                z: pan.z,
            },
            low_pass: audio.effects.low_pass,
            reverb_send: audio.effects.reverb_send,
            echo: audio.effects.echo,
            reflection: audio.effects.reflection,
            occlusion: audio.effects.occlusion,
            eq: perro_pawdio::AudioEq {
                low_gain: audio.effects.eq.low_gain,
                mid_gain: audio.effects.eq.mid_gain,
                high_gain: audio.effects.eq.high_gain,
            },
            compression: perro_pawdio::AudioCompression {
                threshold: audio.effects.compression.threshold,
                ratio: audio.effects.compression.ratio,
                attack: audio.effects.compression.attack,
                release: audio.effects.compression.release,
            },
            from_start: audio.from_start,
            from_end: audio.from_end,
        })
    }

    fn stop_audio_source(&self, source: &str) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_source(source)
    }

    fn audio_length_seconds(&self, source: &str) -> Option<f32> {
        let Ok(guard) = self.bark.lock() else {
            return None;
        };
        let player = guard.as_ref()?;
        player.source_length_seconds(source)
    }

    fn stop_all_audio(&self) {
        let Ok(guard) = self.bark.lock() else {
            return;
        };
        if let Some(player) = guard.as_ref() {
            player.stop_all();
        }
    }

    fn set_master_volume(&self, volume: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.set_master_volume(volume)
    }

    fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.set_bus_volume(bus_id, volume)
    }

    fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.set_bus_speed(bus_id, speed)
    }

    fn pause_bus(&self, bus_id: AudioBusID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.pause_bus(bus_id)
    }

    fn resume_bus(&self, bus_id: AudioBusID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.resume_bus(bus_id)
    }

    fn stop_bus(&self, bus_id: AudioBusID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_bus(bus_id)
    }

    fn load_midi_soundfont_hashed(&self, source_hash: u64, source: Option<&str>) -> SoundFontID {
        let Ok(guard) = self.bark.lock() else {
            return SoundFontID::nil();
        };
        let Some(player) = guard.as_ref() else {
            return SoundFontID::nil();
        };
        let id = SoundFontID::from_u64(source_hash);
        let Some(source) = source else {
            return id;
        };
        player.load_soundfont_with_id(id, source)
    }

    fn load_midi_soundfont_from_bytes(&self, bytes: &[u8]) -> SoundFontID {
        if bytes.is_empty() {
            return SoundFontID::nil();
        }
        let Ok(guard) = self.bark.lock() else {
            return SoundFontID::nil();
        };
        let Some(player) = guard.as_ref() else {
            return SoundFontID::nil();
        };
        let id = SoundFontID::from_u64(bytes_hash(bytes));
        let source = format!("runtime://soundfont/{}", id.as_u64());
        player.load_soundfont_bytes_with_id(id, &source, Arc::from(bytes))
    }

    fn is_midi_soundfont_loaded(&self, id: SoundFontID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.is_soundfont_loaded(id)
    }

    fn play_midi_note(&self, note: Note, options: MidiNoteOptions) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_midi_note(perro_pawdio::midi::MidiNoteRequest {
            id: 0,
            note,
            options,
            held: false,
        })
    }

    fn start_midi_note(&self, note: Note, options: MidiNoteOptions) -> Option<MidiNoteHandle> {
        let Ok(guard) = self.bark.lock() else {
            return None;
        };
        let player = guard.as_ref()?;
        player.start_midi_note(perro_pawdio::midi::MidiNoteRequest {
            id: 0,
            note,
            options,
            held: true,
        })
    }

    fn release_midi_note(&self, handle: MidiNoteHandle) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.release_midi_note(handle)
    }

    fn play_midi_file(&self, song: MidiSong) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_midi_file(perro_pawdio::midi::MidiFileRequest {
            id: 0,
            song,
            pan: perro_pawdio::AudioPan::CENTER,
        })
    }

    fn play_midi_note_at(
        &self,
        note: Note,
        position: MidiSpatialPosition,
        range: f32,
        options: MidiNoteOptions,
    ) -> bool {
        let id = self
            .next_spatial_midi_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let pos = match position {
            MidiSpatialPosition::TwoD(pos) => QueuedSpatialAudioPos::TwoD(pos),
            MidiSpatialPosition::ThreeD(pos) => QueuedSpatialAudioPos::ThreeD(pos),
        };
        let Ok(mut queue) = self.spatial_midi_queue.lock() else {
            return false;
        };
        queue.push(QueuedSpatialMidi {
            kind: QueuedSpatialMidiKind::Note {
                id,
                note,
                options: QueuedMidiNoteOptions::from_options(options),
                held: false,
            },
            range,
            pos,
        });
        true
    }

    fn start_midi_note_at(
        &self,
        note: Note,
        position: MidiSpatialPosition,
        range: f32,
        options: MidiNoteOptions,
    ) -> Option<MidiNoteHandle> {
        let id = self
            .next_spatial_midi_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let pos = match position {
            MidiSpatialPosition::TwoD(pos) => QueuedSpatialAudioPos::TwoD(pos),
            MidiSpatialPosition::ThreeD(pos) => QueuedSpatialAudioPos::ThreeD(pos),
        };
        let Ok(mut queue) = self.spatial_midi_queue.lock() else {
            return None;
        };
        queue.push(QueuedSpatialMidi {
            kind: QueuedSpatialMidiKind::Note {
                id,
                note,
                options: QueuedMidiNoteOptions::from_options(options),
                held: true,
            },
            range,
            pos,
        });
        Some(MidiNoteHandle(id))
    }

    fn play_midi_file_at(&self, song: MidiSong, position: MidiSpatialPosition, range: f32) -> bool {
        let id = self
            .next_spatial_midi_id
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let pos = match position {
            MidiSpatialPosition::TwoD(pos) => QueuedSpatialAudioPos::TwoD(pos),
            MidiSpatialPosition::ThreeD(pos) => QueuedSpatialAudioPos::ThreeD(pos),
        };
        let Ok(mut queue) = self.spatial_midi_queue.lock() else {
            return false;
        };
        queue.push(QueuedSpatialMidi {
            kind: QueuedSpatialMidiKind::File {
                id,
                song: QueuedMidiSong::from_song(song),
            },
            range,
            pos,
        });
        true
    }
}
