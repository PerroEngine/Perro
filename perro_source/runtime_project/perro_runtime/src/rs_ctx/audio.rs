use super::core::RuntimeResourceApi;
use perro_resource_context::sub_apis::AudioAPI;

impl AudioAPI for RuntimeResourceApi {
    fn play_audio(&self, source: &str, looped: bool) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_source(source, looped)
    }

    fn stop_audio(&self, source: &str) -> bool {
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
}
