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

    fn get_draw_gpu_prepare_3d(&self) -> Duration {
        self.time.draw_gpu_prepare_3d
    }

    fn get_draw_gpu_prepare_3d_frustum(&self) -> Duration {
        self.time.draw_gpu_prepare_3d_frustum
    }

    fn get_draw_gpu_prepare_3d_hiz(&self) -> Duration {
        self.time.draw_gpu_prepare_3d_hiz
    }

    fn get_draw_gpu_prepare_3d_indirect(&self) -> Duration {
        self.time.draw_gpu_prepare_3d_indirect
    }

    fn get_draw_gpu_prepare_3d_cull_inputs(&self) -> Duration {
        self.time.draw_gpu_prepare_3d_cull_inputs
    }

    fn get_draw_calls_2d(&self) -> u32 {
        self.time.draw_calls_2d
    }

    fn get_draw_calls_3d(&self) -> u32 {
        self.time.draw_calls_3d
    }

    fn get_draw_calls_total(&self) -> u32 {
        self.time.draw_calls_total
    }

    fn get_draw_instances_3d(&self) -> u32 {
        self.time.draw_instances_3d
    }

    fn get_draw_material_refs_3d(&self) -> u32 {
        self.time.draw_material_refs_3d
    }

    fn get_skip_prepare_3d(&self) -> u32 {
        self.time.skip_prepare_3d
    }

    fn get_skip_prepare_3d_frustum(&self) -> u32 {
        self.time.skip_prepare_3d_frustum
    }

    fn get_skip_prepare_3d_hiz(&self) -> u32 {
        self.time.skip_prepare_3d_hiz
    }

    fn get_skip_prepare_3d_indirect(&self) -> u32 {
        self.time.skip_prepare_3d_indirect
    }

    fn get_skip_prepare_3d_cull_inputs(&self) -> u32 {
        self.time.skip_prepare_3d_cull_inputs
    }
}
