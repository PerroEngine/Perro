use super::core::RuntimeResourceApi;
use perro_ids::BusID;
use perro_resource_context::sub_apis::AudioAPI;

impl AudioAPI for RuntimeResourceApi {
    fn play_audio(
        &self,
        source: &str,
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(source, bus_id, looped, volume, speed)
    }

    fn stop_audio(
        &self,
        source: &str,
        bus_id: BusID,
        looped: bool,
        volume: f32,
        speed: f32,
    ) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_match(source, bus_id, looped, volume, speed)
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

    fn set_bus_volume(&self, bus_id: BusID, volume: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.set_bus_volume(bus_id, volume)
    }

    fn set_bus_speed(&self, bus_id: BusID, speed: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.set_bus_speed(bus_id, speed)
    }

    fn pause_bus(&self, bus_id: BusID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.pause_bus(bus_id)
    }

    fn resume_bus(&self, bus_id: BusID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.resume_bus(bus_id)
    }

    fn stop_bus(&self, bus_id: BusID) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.stop_bus(bus_id)
    }
}
