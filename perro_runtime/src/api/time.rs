use perro_context::sub_apis::TimeAPI;

use crate::Runtime;

impl TimeAPI for Runtime {
    fn get_delta(&self) -> f32 {
        self.time.delta
    }

    fn get_fixed_delta(&self) -> f32 {
        self.time.fixed_delta
    }

    fn get_elapsed(&self) -> f32 {
        self.time.elapsed
    }
}
