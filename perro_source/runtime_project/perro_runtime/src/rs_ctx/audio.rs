use super::core::RuntimeResourceApi;
use perro_ids::AudioBusID;
use perro_resource_context::sub_apis::{Audio, Audio2D, Audio3D, AudioAPI};

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

    fn play_audio(
        &self,
        bus_id: Option<AudioBusID>,
        audio: Audio<'_>,
        pan: perro_resource_context::sub_apis::AudioPan,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(perro_bark::AudioPlaybackRequest {
            source: audio.source,
            bus_id,
            looped: audio.looped,
            volume: audio.volume,
            speed: audio.speed,
            pan: perro_bark::AudioPan {
                x: pan.x,
                y: pan.y,
                z: pan.z,
            },
            from_start: audio.from_start,
            from_end: audio.from_end,
        })
    }

    fn play_audio_2d(&self, bus_id: Option<AudioBusID>, audio: Audio2D<'_>) -> bool {
        let listener = self
            .audio_listener_2d
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or_default();
        let request = perro_bark::Audio2D {
            source: audio.audio.source,
            bus_id,
            looped: audio.audio.looped,
            volume: audio.audio.volume,
            speed: audio.audio.speed,
            position: [audio.position.x, audio.position.y],
            range: audio.range,
            from_start: audio.audio.from_start,
            from_end: audio.audio.from_end,
        };
        let Some(playback) = request.to_playback(listener) else {
            return true;
        };
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(playback)
    }

    fn play_audio_3d(&self, bus_id: Option<AudioBusID>, audio: Audio3D<'_>) -> bool {
        let listener = self
            .audio_listener_3d
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or_default();
        let request = perro_bark::Audio3D {
            source: audio.audio.source,
            bus_id,
            looped: audio.audio.looped,
            volume: audio.audio.volume,
            speed: audio.audio.speed,
            position: [audio.position.x, audio.position.y, audio.position.z],
            range: audio.range,
            from_start: audio.audio.from_start,
            from_end: audio.audio.from_end,
        };
        let Some(playback) = request.to_playback(listener) else {
            return true;
        };
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(playback)
    }

    fn stop_audio(
        &self,
        bus_id: Option<AudioBusID>,
        audio: Audio<'_>,
        pan: perro_resource_context::sub_apis::AudioPan,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_match(perro_bark::AudioPlaybackRequest {
            source: audio.source,
            bus_id,
            looped: audio.looped,
            volume: audio.volume,
            speed: audio.speed,
            pan: perro_bark::AudioPan {
                x: pan.x,
                y: pan.y,
                z: pan.z,
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
}
