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
    pub extract_2d: Duration,
    pub extract_3d: Duration,
    pub drain_commands: Duration,
    pub submit_commands: Duration,
    pub draw_frame: Duration,
    pub drain_events: Duration,
    pub apply_events: Duration,
    pub total: Duration,
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
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_joycon_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.runtime.set_joycon_accel(index, x, y, z);
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
        let total_start = std::time::Instant::now();

        let extract_2d_start = std::time::Instant::now();
        self.runtime.extract_render_2d_commands();
        let extract_2d = extract_2d_start.elapsed();

        let extract_3d_start = std::time::Instant::now();
        self.runtime.extract_render_3d_commands();
        let extract_3d = extract_3d_start.elapsed();

        let drain_commands_start = std::time::Instant::now();
        self.runtime.drain_render_commands(&mut self.command_buffer);
        let drain_commands = drain_commands_start.elapsed();

        let submit_start = std::time::Instant::now();
        self.graphics.submit_many(self.command_buffer.drain(..));
        let submit_commands = submit_start.elapsed();

        let draw_frame_start = std::time::Instant::now();
        self.graphics.draw_frame();
        let draw_frame = draw_frame_start.elapsed();

        let drain_events_start = std::time::Instant::now();
        self.graphics.drain_events(&mut self.event_buffer);
        let drain_events = drain_events_start.elapsed();

        let apply_events_start = std::time::Instant::now();
        self.runtime
            .apply_render_events(self.event_buffer.drain(..));
        let apply_events = apply_events_start.elapsed();
        // Dirty markers are per-frame extraction hints; clear after a full frame.
        self.runtime.clear_dirty_flags();

        PresentTiming {
            extract_2d,
            extract_3d,
            drain_commands,
            submit_commands,
            draw_frame,
            drain_events,
            apply_events,
            total: total_start.elapsed(),
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
