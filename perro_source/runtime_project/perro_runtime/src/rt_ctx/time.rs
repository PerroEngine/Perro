use perro_runtime_api::sub_apis::TimeAPI;
use std::time::Duration;

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

    fn get_simulation_time(&self) -> Duration {
        self.time.simulation
    }

    fn get_graphics_time(&self) -> Duration {
        self.time.graphics
    }

    fn get_frame_time(&self) -> Duration {
        self.time.frame
    }

    fn get_fps(&self) -> f32 {
        self.time.fps
    }
}
