use super::core::RuntimeResourceApi;
use perro_resource_context::sub_apis::AudioAPI;

impl AudioAPI for RuntimeResourceApi {
    fn play_audio(
        &self,
        source: &str,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(source, bus_id, looped, volume, pitch)
    }

    fn stop_audio(
        &self,
        source: &str,
        bus_id: u32,
        looped: bool,
        volume: f32,
        pitch: f32,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_match(source, bus_id, looped, volume, pitch)
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

    fn set_bus_volume(&self, bus_id: u32, volume: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.set_bus_volume(bus_id, volume)
    }

    fn pause_bus(&self, bus_id: u32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.pause_bus(bus_id)
    }

    fn resume_bus(&self, bus_id: u32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.resume_bus(bus_id)
    }

    fn stop_bus(&self, bus_id: u32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_bus(bus_id)
    }
}
