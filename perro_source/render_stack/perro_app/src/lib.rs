use perro_graphics::GraphicsBackend;
use perro_input::{GamepadAxis, GamepadButton, JoyConButton, KeyCode, MouseButton};
use perro_render_bridge::RenderEvent;
use perro_runtime::Runtime;
use std::sync::Arc;
use std::time::Duration;
use winit::window::Window;

pub struct App<B: GraphicsBackend> {
    pub runtime: Runtime,
    pub graphics: B,
    command_buffer: Vec<perro_render_bridge::RenderCommand>,
    event_buffer: Vec<RenderEvent>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PresentTiming {
    pub gpu_present: Duration,
    pub total: Duration,
    #[cfg(feature = "profile_heavy")]
    pub extract_2d: Duration,
    #[cfg(feature = "profile_heavy")]
    pub extract_3d: Duration,
    #[cfg(feature = "profile_heavy")]
    pub drain_commands: Duration,
    #[cfg(feature = "profile_heavy")]
    pub submit_commands: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_process_commands: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_prepare_cpu: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_2d: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_3d: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_particles_3d: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_3d_frustum: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_3d_hiz: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_3d_indirect: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_prepare_3d_cull_inputs: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_acquire: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_acquire_surface: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_acquire_view: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_encode_main: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_submit_main: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_submit_finish_main: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_submit_queue_main: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_post_process: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_accessibility: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_gpu_present: Duration,
    #[cfg(feature = "profile_heavy")]
    pub draw_calls_2d: u32,
    #[cfg(feature = "profile_heavy")]
    pub draw_calls_3d: u32,
    #[cfg(feature = "profile_heavy")]
    pub draw_calls_total: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_2d: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_3d: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_particles_3d: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_3d_frustum: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_3d_hiz: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_3d_indirect: u32,
    #[cfg(feature = "profile_heavy")]
    pub skip_prepare_3d_cull_inputs: u32,
    #[cfg(feature = "profile_heavy")]
    pub drain_events: Duration,
    #[cfg(feature = "profile_heavy")]
    pub apply_events: Duration,
}

impl<B: GraphicsBackend> App<B> {
    pub fn new(runtime: Runtime, graphics: B) -> Self {
        Self {
            runtime,
            graphics,
            command_buffer: Vec::new(),
            event_buffer: Vec::new(),
        }
    }

    pub fn with_empty_runtime(graphics: B) -> Self {
        Self::new(Runtime::new(), graphics)
    }

    #[inline]
    pub fn set_elapsed_time(&mut self, elapsed_time: f32) {
        self.runtime.time.elapsed = elapsed_time;
    }

    #[inline]
    pub fn set_smoothing(&mut self, enabled: bool) {
        self.graphics.set_smoothing(enabled);
    }

    #[inline]
    pub fn set_smoothing_samples(&mut self, samples: u32) {
        self.graphics.set_smoothing_samples(samples);
    }

    #[inline]
    pub fn attach_window(&mut self, window: Arc<Window>) {
        self.graphics.attach_window(window);
    }

    #[inline]
    pub fn update_runtime(&mut self, delta_time: f32) -> perro_runtime::RuntimeUpdateTiming {
        self.runtime.update_timed(delta_time)
    }

    #[inline]
    pub fn begin_input_frame(&mut self) {
        self.runtime.begin_input_frame();
    }

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.runtime.set_key_state(key, is_down);
    }

    #[inline]
    pub fn set_mouse_button_state(&mut self, button: MouseButton, is_down: bool) {
        self.runtime.set_mouse_button_state(button, is_down);
    }

    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.runtime.add_mouse_delta(dx, dy);
    }

    #[inline]
    pub fn add_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.runtime.add_mouse_wheel(dx, dy);
    }

    #[inline]
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.runtime.set_mouse_position(x, y);
    }

    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.runtime.set_viewport_size(width, height);
    }

    #[inline]
    pub fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool) {
        self.runtime
            .set_gamepad_button_state(index, button, is_down);
    }

    #[inline]
    pub fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32) {
        self.runtime.set_gamepad_axis(index, axis, value);
    }

    #[inline]
    pub fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_gamepad_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_gamepad_accel(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_button_state(&mut self, index: usize, button: JoyConButton, is_down: bool) {
        self.runtime.set_joycon_button_state(index, button, is_down);
    }

    #[inline]
    pub fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        self.runtime.set_joycon_stick(index, x, y);
    }

    #[inline]
    pub fn set_joycon_side(&mut self, index: usize, side: perro_input::JoyConSide) {
        self.runtime.set_joycon_side(index, side);
    }

    #[inline]
    pub fn set_joycon_connected(&mut self, index: usize, connected: bool) {
        self.runtime.set_joycon_connected(index, connected);
    }

    #[inline]
    pub fn set_joycon_calibrated(&mut self, index: usize, calibrated: bool) {
        self.runtime.set_joycon_calibrated(index, calibrated);
    }

    #[inline]
    pub fn set_joycon_calibration_in_progress(&mut self, index: usize, in_progress: bool) {
        self.runtime
            .set_joycon_calibration_in_progress(index, in_progress);
    }

    #[inline]
    pub fn set_joycon_calibration_bias(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_joycon_calibration_bias(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_joycon_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_joycon_accel(index, x, y, z);
    }

    #[inline]
    pub fn take_joycon_calibration_requests(&mut self) -> Vec<usize> {
        self.runtime.take_joycon_calibration_requests()
    }

    #[inline]
    pub fn fixed_update_runtime(&mut self, fixed_delta_time: f32) {
        self.runtime.fixed_update(fixed_delta_time);
    }

    #[inline]
    pub fn present(&mut self) {
        let _ = self.present_timed();
    }

    #[inline]
    pub fn present_timed(&mut self) -> PresentTiming {
        self.present_with_overlay_timed(std::iter::empty::<perro_render_bridge::RenderCommand>())
    }

    pub fn present_with_overlay_timed<I>(&mut self, overlay_commands: I) -> PresentTiming
    where
        I: IntoIterator<Item = perro_render_bridge::RenderCommand>,
    {
        let total_start = std::time::Instant::now();

        #[cfg(feature = "profile_heavy")]
        let extract_2d_start = std::time::Instant::now();
        self.runtime.extract_render_2d_commands();
        #[cfg(feature = "profile_heavy")]
        let extract_2d = extract_2d_start.elapsed();

        #[cfg(feature = "profile_heavy")]
        let extract_3d_start = std::time::Instant::now();
        self.runtime.extract_render_3d_commands();
        #[cfg(feature = "profile_heavy")]
        let extract_3d = extract_3d_start.elapsed();

        #[cfg(feature = "profile_heavy")]
        let drain_commands_start = std::time::Instant::now();
        self.runtime.drain_render_commands(&mut self.command_buffer);
        #[cfg(feature = "profile_heavy")]
        let drain_commands = drain_commands_start.elapsed();

        #[cfg(feature = "profile_heavy")]
        let submit_start = std::time::Instant::now();
        self.graphics.submit_many(self.command_buffer.drain(..));
        self.graphics.submit_many(overlay_commands);
        #[cfg(feature = "profile_heavy")]
        let submit_commands = submit_start.elapsed();

        let draw_frame_start = std::time::Instant::now();
        #[cfg(feature = "profile_heavy")]
        let draw_timing = self.graphics.draw_frame_timed();
        #[cfg(not(feature = "profile_heavy"))]
        self.graphics.draw_frame();
        let gpu_present = draw_frame_start.elapsed();

        #[cfg(feature = "profile_heavy")]
        let drain_events_start = std::time::Instant::now();
        self.graphics.drain_events(&mut self.event_buffer);
        #[cfg(feature = "profile_heavy")]
        let drain_events = drain_events_start.elapsed();

        #[cfg(feature = "profile_heavy")]
        let apply_events_start = std::time::Instant::now();
        self.runtime
            .apply_render_events(self.event_buffer.drain(..));
        #[cfg(feature = "profile_heavy")]
        let apply_events = apply_events_start.elapsed();
        // Dirty markers are per-frame extraction hints; clear after a full frame.
        self.runtime.clear_dirty_flags();

        PresentTiming {
            gpu_present,
            total: total_start.elapsed(),
            #[cfg(feature = "profile_heavy")]
            extract_2d,
            #[cfg(feature = "profile_heavy")]
            extract_3d,
            #[cfg(feature = "profile_heavy")]
            drain_commands,
            #[cfg(feature = "profile_heavy")]
            submit_commands,
            #[cfg(feature = "profile_heavy")]
            draw_process_commands: draw_timing
                .as_ref()
                .map(|t| t.process_commands)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_prepare_cpu: draw_timing
                .as_ref()
                .map(|t| t.prepare_cpu)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_2d: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_2d)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_3d: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_3d)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_particles_3d: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_particles_3d)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_3d_frustum: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_3d_frustum)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_3d_hiz: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_3d_hiz)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_3d_indirect: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_3d_indirect)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_prepare_3d_cull_inputs: draw_timing
                .as_ref()
                .map(|t| t.gpu_prepare_3d_cull_inputs)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_acquire: draw_timing
                .as_ref()
                .map(|t| t.gpu_acquire)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_acquire_surface: draw_timing
                .as_ref()
                .map(|t| t.gpu_acquire_surface)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_acquire_view: draw_timing
                .as_ref()
                .map(|t| t.gpu_acquire_view)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_encode_main: draw_timing
                .as_ref()
                .map(|t| t.gpu_encode_main)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_submit_main: draw_timing
                .as_ref()
                .map(|t| t.gpu_submit_main)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_submit_finish_main: draw_timing
                .as_ref()
                .map(|t| t.gpu_submit_finish_main)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_submit_queue_main: draw_timing
                .as_ref()
                .map(|t| t.gpu_submit_queue_main)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_post_process: draw_timing
                .as_ref()
                .map(|t| t.gpu_post_process)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_accessibility: draw_timing
                .as_ref()
                .map(|t| t.gpu_accessibility)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_gpu_present: draw_timing
                .as_ref()
                .map(|t| t.gpu_present)
                .unwrap_or(Duration::ZERO),
            #[cfg(feature = "profile_heavy")]
            draw_calls_2d: draw_timing.as_ref().map(|t| t.draw_calls_2d).unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            draw_calls_3d: draw_timing.as_ref().map(|t| t.draw_calls_3d).unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            draw_calls_total: draw_timing
                .as_ref()
                .map(|t| t.draw_calls_2d.saturating_add(t.draw_calls_3d))
                .unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_2d: draw_timing.as_ref().map(|t| t.skip_prepare_2d).unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_3d: draw_timing.as_ref().map(|t| t.skip_prepare_3d).unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_particles_3d: draw_timing
                .as_ref()
                .map(|t| t.skip_prepare_particles_3d)
                .unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_3d_frustum: draw_timing
                .as_ref()
                .map(|t| t.skip_prepare_3d_frustum)
                .unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_3d_hiz: draw_timing
                .as_ref()
                .map(|t| t.skip_prepare_3d_hiz)
                .unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_3d_indirect: draw_timing
                .as_ref()
                .map(|t| t.skip_prepare_3d_indirect)
                .unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            skip_prepare_3d_cull_inputs: draw_timing
                .as_ref()
                .map(|t| t.skip_prepare_3d_cull_inputs)
                .unwrap_or(0),
            #[cfg(feature = "profile_heavy")]
            drain_events,
            #[cfg(feature = "profile_heavy")]
            apply_events,
        }
    }

    #[inline]
    pub fn resize_surface(&mut self, width: u32, height: u32) {
        self.graphics.resize(width, height);
        self.runtime.set_viewport_size(width, height);
    }

    pub fn frame(&mut self, delta_time: f32) {
        let _ = self.update_runtime(delta_time);
        self.present();
    }
}

pub mod entry;
pub mod input;
pub mod winit_runner;
