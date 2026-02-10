use perro_api::modules::TimeAPI;

use crate::Runtime;

impl TimeAPI for Runtime {
    fn get_delta(&self) -> f32 {
        self.delta_time.get()
    }
}
