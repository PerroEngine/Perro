use crate::App;
use perro_graphics::GraphicsBackend;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{fs, sync::Arc};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    event::{DeviceEvent, ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    monitor::MonitorHandle,
    window::{CursorGrabMode, Icon, Window, WindowAttributes},
};

const DEFAULT_FPS_CAP: f32 = 60.0;
const DEFAULT_FIXED_TIMESTEP: Option<f32> = None;
const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;
const LOG_INTERVAL_SECONDS: f32 = 2.5;
const INITIAL_WINDOW_MONITOR_FRACTION: f32 = 0.75;

#[inline]
fn normalize_fixed_timestep_seconds(value: Option<f32>) -> Option<f32> {
    let raw = value?;
    if !raw.is_finite() || raw <= 0.0 {
        return None;
    }
    // New semantics: values >= 1.0 are treated as Hz.
    // Backward compatibility: sub-second values remain seconds-per-step.
    if raw < 1.0 {
        Some(raw)
    } else {
        Some(1.0 / raw)
    }
}

#[inline]
fn target_frame_duration(fps_cap: f32) -> Option<Duration> {
    if !fps_cap.is_finite() || fps_cap <= 0.0 {
        return None;
    }
    let secs = 1.0f64 / fps_cap as f64;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    let d = Duration::from_secs_f64(secs);
    if d.is_zero() { None } else { Some(d) }
}

pub struct WinitRunner;

impl WinitRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run<B: GraphicsBackend>(self, app: App<B>, title: &str) {
        self.run_with_fps_cap_and_timestep(app, title, DEFAULT_FPS_CAP, DEFAULT_FIXED_TIMESTEP);
    }

    pub fn run_with_fps_cap<B: GraphicsBackend>(self, app: App<B>, title: &str, fps_cap: f32) {
        self.run_with_fps_cap_and_timestep(app, title, fps_cap, DEFAULT_FIXED_TIMESTEP);
    }

    pub fn run_with_fps_cap_and_timestep<B: GraphicsBackend>(
        self,
        app: App<B>,
        title: &str,
        fps_cap: f32,
        fixed_timestep: Option<f32>,
    ) {
        let event_loop = EventLoop::new().expect("failed to create winit event loop");
        let mut state = RunnerState::new(app, title, fps_cap, fixed_timestep);
        event_loop
            .run_app(&mut state)
            .expect("winit event loop failed");
    }

    pub fn event_loop_type_name() -> &'static str {
        std::any::type_name::<EventLoop<()>>()
    }
}

impl Default for WinitRunner {
    fn default() -> Self {
        Self::new()
    }
}

struct RunnerState<B: GraphicsBackend> {
    app: App<B>,
    title: String,
    window: Option<Arc<Window>>,
    last_frame_start: Instant,
    last_frame_end: Instant,
    next_frame_deadline: Instant,
    run_start: Instant,
    batch_start: Instant,
    batch_work: Duration,
    batch_simulation: Duration,
    batch_runtime_update: Duration,
    batch_input_poll: Duration,
    batch_fixed_update: Duration,
    batch_runtime_start_schedule: Duration,
    batch_runtime_snapshot_update: Duration,
    batch_runtime_script_update: Duration,
    batch_runtime_internal_update: Duration,
    batch_runtime_slowest_script: Duration,
    batch_runtime_script_count: u64,
    batch_present: Duration,
    batch_present_extract_2d: Duration,
    batch_present_extract_3d: Duration,
    batch_present_drain_commands: Duration,
    batch_present_submit_commands: Duration,
    batch_present_draw_frame: Duration,
    batch_draw_process_commands: Duration,
    batch_draw_prepare_cpu: Duration,
    batch_draw_gpu_prepare_2d: Duration,
    batch_draw_gpu_prepare_3d: Duration,
    batch_draw_gpu_acquire: Duration,
    batch_draw_gpu_encode_main: Duration,
    batch_draw_gpu_submit_main: Duration,
    batch_draw_gpu_post_process: Duration,
    batch_draw_gpu_accessibility: Duration,
    batch_draw_gpu_present: Duration,
    batch_present_drain_events: Duration,
    batch_present_apply_events: Duration,
    batch_idle: Duration,
    batch_sim_delta_seconds: f64,
    fixed_timestep: Option<f32>,
    fps_cap: f32,
    fixed_accumulator: f32,
    batch_frames: u32,
    kbm_input: crate::input::KbmInput,
    gamepad_input: crate::input::GamepadInput,
    joycon_input: crate::input::JoyConInput,
    cursor_captured: bool,
    capture_uses_raw_motion: bool,
    cursor_inside_window: bool,
}

impl<B: GraphicsBackend> RunnerState<B> {
    fn new(app: App<B>, title: &str, fps_cap: f32, fixed_timestep: Option<f32>) -> Self {
        let now = Instant::now();
        let normalized_fixed_timestep = normalize_fixed_timestep_seconds(fixed_timestep);
        if let Some(raw) = fixed_timestep
            && raw >= 1.0
            && let Some(step) = normalized_fixed_timestep
        {
            println!(
                "[runtime] target_fixed_update interpreted as Hz: {raw} -> step {:.6}s",
                step
            );
        }
        Self {
            app,
            title: title.to_owned(),
            window: None,
            fps_cap,
            fixed_timestep: normalized_fixed_timestep,
            fixed_accumulator: 0.0,
            last_frame_start: now,
            last_frame_end: now,
            next_frame_deadline: now,
            run_start: now,
            batch_frames: 0,
            batch_start: now,
            batch_work: Duration::ZERO,
            batch_simulation: Duration::ZERO,
            batch_runtime_update: Duration::ZERO,
            batch_input_poll: Duration::ZERO,
            batch_fixed_update: Duration::ZERO,
            batch_runtime_start_schedule: Duration::ZERO,
            batch_runtime_snapshot_update: Duration::ZERO,
            batch_runtime_script_update: Duration::ZERO,
            batch_runtime_internal_update: Duration::ZERO,
            batch_runtime_slowest_script: Duration::ZERO,
            batch_runtime_script_count: 0,
            batch_present: Duration::ZERO,
            batch_present_extract_2d: Duration::ZERO,
            batch_present_extract_3d: Duration::ZERO,
            batch_present_drain_commands: Duration::ZERO,
            batch_present_submit_commands: Duration::ZERO,
            batch_present_draw_frame: Duration::ZERO,
            batch_draw_process_commands: Duration::ZERO,
            batch_draw_prepare_cpu: Duration::ZERO,
            batch_draw_gpu_prepare_2d: Duration::ZERO,
            batch_draw_gpu_prepare_3d: Duration::ZERO,
            batch_draw_gpu_acquire: Duration::ZERO,
            batch_draw_gpu_encode_main: Duration::ZERO,
            batch_draw_gpu_submit_main: Duration::ZERO,
            batch_draw_gpu_post_process: Duration::ZERO,
            batch_draw_gpu_accessibility: Duration::ZERO,
            batch_draw_gpu_present: Duration::ZERO,
            batch_present_drain_events: Duration::ZERO,
            batch_present_apply_events: Duration::ZERO,
            batch_idle: Duration::ZERO,
            batch_sim_delta_seconds: 0.0,
            kbm_input: crate::input::KbmInput::new(),
            gamepad_input: crate::input::GamepadInput::new(),
            joycon_input: crate::input::JoyConInput::new(),
            cursor_captured: false,
            capture_uses_raw_motion: false,
            cursor_inside_window: false,
        }
    }

    fn apply_cursor_capture(window: &Window, capture: bool) -> (bool, bool) {
        if capture {
            match window.set_cursor_grab(CursorGrabMode::Locked) {
                Ok(_) => {
                    window.set_cursor_visible(false);
                    (true, true)
                }
                Err(locked_err) => match window.set_cursor_grab(CursorGrabMode::Confined) {
                    Ok(_) => {
                        window.set_cursor_visible(false);
                        (true, false)
                    }
                    Err(confined_err) => {
                        println!(
                            "[runtime] failed to capture cursor (locked: {locked_err}; confined: {confined_err})"
                        );
                        window.set_cursor_visible(true);
                        (false, false)
                    }
                },
            }
        } else {
            if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                println!("[runtime] failed to release cursor capture: {err}");
            }
            window.set_cursor_visible(true);
            (false, false)
        }
    }

    fn set_cursor_capture(&mut self, capture: bool) {
        if self.cursor_captured == capture {
            return;
        }
        if let Some(window) = &self.window {
            let (captured, uses_raw_motion) = Self::apply_cursor_capture(window.as_ref(), capture);
            self.cursor_captured = captured;
            self.capture_uses_raw_motion = uses_raw_motion;
        } else {
            self.cursor_captured = false;
            self.capture_uses_raw_motion = false;
        }
    }

    fn step_frame(&mut self, now: Instant) {
        if let Some(target) = target_frame_duration(self.fps_cap) {
            if self.next_frame_deadline <= self.last_frame_start {
                self.next_frame_deadline = self.last_frame_start + target;
            }

            if now < self.next_frame_deadline {
                return;
            }

            self.next_frame_deadline = now + target;
        } else {
            // Uncapped mode for tiny/invalid frame targets.
            self.next_frame_deadline = now;
        }

        let frame_start = now;
        let frame_delta = frame_start.duration_since(self.last_frame_start);
        self.last_frame_start = frame_start;

        let elapsed_since_start = frame_start.duration_since(self.run_start);
        self.app.set_elapsed_time(elapsed_since_start.as_secs_f32());
        let simulated_delta_seconds;

        let idle_duration = frame_start.saturating_duration_since(self.last_frame_end);
        let work_start = Instant::now();
        let mut runtime_update_duration = Duration::ZERO;

        let simulation_start = Instant::now();
        let input_poll_start = Instant::now();
        // Poll device inputs before update so scripts see the latest state.
        self.gamepad_input.begin_frame(&mut self.app);
        self.joycon_input.begin_frame(&mut self.app);
        let input_poll_duration = input_poll_start.elapsed();
        let fixed_start = Instant::now();

        {
            if let Some(effective_fixed_step) = self.fixed_timestep {
                self.fixed_accumulator += frame_delta.as_secs_f32();
                let mut steps = 0u32;
                while self.fixed_accumulator >= effective_fixed_step
                    && steps < MAX_FIXED_STEPS_PER_FRAME
                {
                    let update_start = Instant::now();
                    self.app.fixed_update_runtime(effective_fixed_step);
                    runtime_update_duration += update_start.elapsed();
                    self.fixed_accumulator -= effective_fixed_step;
                    steps += 1;
                }
                if steps == MAX_FIXED_STEPS_PER_FRAME
                    && self.fixed_accumulator >= effective_fixed_step
                {
                    // Drop excess accumulated time to avoid spiral-of-death behavior.
                    self.fixed_accumulator = 0.0;
                }
                simulated_delta_seconds = effective_fixed_step as f64 * steps as f64;
            } else {
                let variable_step = frame_delta.as_secs_f32();
                let update_start = Instant::now();
                self.app.fixed_update_runtime(variable_step);
                runtime_update_duration += update_start.elapsed();
                simulated_delta_seconds = variable_step as f64;
            }
        }

        let fixed_duration = fixed_start.elapsed();
        let runtime_timing = self.app.update_runtime(frame_delta.as_secs_f32());
        runtime_update_duration += runtime_timing.total;
        let simulation_duration = simulation_start.elapsed();

        let present_timing = self.app.present_timed();
        let present_duration = present_timing.total;
        let work_duration = work_start.elapsed();

        let frame_end = Instant::now();
        self.last_frame_end = frame_end;

        self.batch_frames = self.batch_frames.saturating_add(1);
        self.batch_work += work_duration;
        self.batch_simulation += simulation_duration;
        self.batch_runtime_update += runtime_update_duration;
        self.batch_input_poll += input_poll_duration;
        self.batch_fixed_update += fixed_duration;
        self.batch_runtime_start_schedule += runtime_timing.start_schedule;
        self.batch_runtime_snapshot_update += runtime_timing.snapshot_update;
        self.batch_runtime_script_update += runtime_timing.update_schedule.scripts_total;
        self.batch_runtime_internal_update += runtime_timing.internal_update;
        self.batch_runtime_script_count += runtime_timing.update_schedule.script_count as u64;
        if runtime_timing.update_schedule.slowest_script > self.batch_runtime_slowest_script {
            self.batch_runtime_slowest_script = runtime_timing.update_schedule.slowest_script;
        }
        self.batch_present += present_duration;
        self.batch_present_extract_2d += present_timing.extract_2d;
        self.batch_present_extract_3d += present_timing.extract_3d;
        self.batch_present_drain_commands += present_timing.drain_commands;
        self.batch_present_submit_commands += present_timing.submit_commands;
        self.batch_present_draw_frame += present_timing.draw_frame;
        self.batch_draw_process_commands += present_timing.draw_process_commands;
        self.batch_draw_prepare_cpu += present_timing.draw_prepare_cpu;
        self.batch_draw_gpu_prepare_2d += present_timing.draw_gpu_prepare_2d;
        self.batch_draw_gpu_prepare_3d += present_timing.draw_gpu_prepare_3d;
        self.batch_draw_gpu_acquire += present_timing.draw_gpu_acquire;
        self.batch_draw_gpu_encode_main += present_timing.draw_gpu_encode_main;
        self.batch_draw_gpu_submit_main += present_timing.draw_gpu_submit_main;
        self.batch_draw_gpu_post_process += present_timing.draw_gpu_post_process;
        self.batch_draw_gpu_accessibility += present_timing.draw_gpu_accessibility;
        self.batch_draw_gpu_present += present_timing.draw_gpu_present;
        self.batch_present_drain_events += present_timing.drain_events;
        self.batch_present_apply_events += present_timing.apply_events;
        self.batch_idle += idle_duration;
        self.batch_sim_delta_seconds += simulated_delta_seconds;

        let batch_elapsed_secs = frame_end.duration_since(self.batch_start).as_secs_f32();
        if batch_elapsed_secs >= LOG_INTERVAL_SECONDS && self.batch_frames > 0 {
            let work_ms = self.batch_work.as_secs_f64() * 1_000.0;
            let avg_work_us = (work_ms * 1_000.0) / self.batch_frames as f64;
            let avg_simulation_us =
                self.batch_simulation.as_micros() as f64 / self.batch_frames as f64;
            let avg_runtime_update_us =
                self.batch_runtime_update.as_micros() as f64 / self.batch_frames as f64;
            let avg_input_poll_us =
                self.batch_input_poll.as_micros() as f64 / self.batch_frames as f64;
            let avg_fixed_update_us =
                self.batch_fixed_update.as_micros() as f64 / self.batch_frames as f64;
            let present_ms = self.batch_present.as_secs_f64() * 1_000.0;
            let avg_present_us = (present_ms * 1_000.0) / self.batch_frames as f64;
            let idle_ms = self.batch_idle.as_secs_f64() * 1_000.0;
            let avg_idle_us = (idle_ms * 1_000.0) / self.batch_frames as f64;
            let avg_runtime_script_update_us =
                self.batch_runtime_script_update.as_micros() as f64 / self.batch_frames as f64;
            let avg_runtime_script_count =
                self.batch_runtime_script_count as f64 / self.batch_frames as f64;

            let avg_present_extract_2d_us =
                self.batch_present_extract_2d.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_extract_3d_us =
                self.batch_present_extract_3d.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_drain_commands_us =
                self.batch_present_drain_commands.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_submit_commands_us =
                self.batch_present_submit_commands.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_draw_frame_us =
                self.batch_present_draw_frame.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_process_commands_us =
                self.batch_draw_process_commands.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_prepare_cpu_us =
                self.batch_draw_prepare_cpu.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_prepare_2d_us =
                self.batch_draw_gpu_prepare_2d.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_prepare_3d_us =
                self.batch_draw_gpu_prepare_3d.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_acquire_us =
                self.batch_draw_gpu_acquire.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_encode_main_us =
                self.batch_draw_gpu_encode_main.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_submit_main_us =
                self.batch_draw_gpu_submit_main.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_post_process_us =
                self.batch_draw_gpu_post_process.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_accessibility_us =
                self.batch_draw_gpu_accessibility.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_present_us =
                self.batch_draw_gpu_present.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_drain_events_us =
                self.batch_present_drain_events.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_apply_events_us =
                self.batch_present_apply_events.as_micros() as f64 / self.batch_frames as f64;
            println!(
                "update: ({:.3}us avg) | frame present: ({:.3}us avg) | total: ({:.3}us avg) | idle: ({:.3}us avg)",
                avg_simulation_us, avg_present_us, avg_work_us, avg_idle_us
            );
            println!(
                "simulation breakdown: input=({:.3}us) fixed=({:.3}us) runtime=({:.3}us)",
                avg_input_poll_us, avg_fixed_update_us, avg_runtime_update_us
            );
            println!(
                "user scripts: ({:.3}us avg) | script calls/frame: ({:.2}) | slowest script: ({:.3}us)",
                avg_runtime_script_update_us,
                avg_runtime_script_count,
                self.batch_runtime_slowest_script.as_micros() as f64
            );
            println!(
                "present breakdown: extract2d=({:.3}us) extract3d=({:.3}us) drain=({:.3}us) submit=({:.3}us) draw=({:.3}us) events_drain=({:.3}us) events_apply=({:.3}us)",
                avg_present_extract_2d_us,
                avg_present_extract_3d_us,
                avg_present_drain_commands_us,
                avg_present_submit_commands_us,
                avg_present_draw_frame_us,
                avg_present_drain_events_us,
                avg_present_apply_events_us
            );
            println!(
                "draw breakdown: process=({:.3}us) prep=({:.3}us) gpu_prepare2d=({:.3}us) gpu_prepare3d=({:.3}us) acquire=({:.3}us) encode=({:.3}us) gpu_submit=({:.3}us) post=({:.3}us) access=({:.3}us) present=({:.3}us)",
                avg_draw_process_commands_us,
                avg_draw_prepare_cpu_us,
                avg_draw_gpu_prepare_2d_us,
                avg_draw_gpu_prepare_3d_us,
                avg_draw_gpu_acquire_us,
                avg_draw_gpu_encode_main_us,
                avg_draw_gpu_submit_main_us,
                avg_draw_gpu_post_process_us,
                avg_draw_gpu_accessibility_us,
                avg_draw_gpu_present_us
            );
            println!("---");

            self.batch_frames = 0;
            self.batch_work = Duration::ZERO;
            self.batch_simulation = Duration::ZERO;
            self.batch_runtime_update = Duration::ZERO;
            self.batch_input_poll = Duration::ZERO;
            self.batch_fixed_update = Duration::ZERO;
            self.batch_runtime_start_schedule = Duration::ZERO;
            self.batch_runtime_snapshot_update = Duration::ZERO;
            self.batch_runtime_script_update = Duration::ZERO;
            self.batch_runtime_internal_update = Duration::ZERO;
            self.batch_runtime_slowest_script = Duration::ZERO;
            self.batch_runtime_script_count = 0;
            self.batch_present = Duration::ZERO;
            self.batch_present_extract_2d = Duration::ZERO;
            self.batch_present_extract_3d = Duration::ZERO;
            self.batch_present_drain_commands = Duration::ZERO;
            self.batch_present_submit_commands = Duration::ZERO;
            self.batch_present_draw_frame = Duration::ZERO;
            self.batch_draw_process_commands = Duration::ZERO;
            self.batch_draw_prepare_cpu = Duration::ZERO;
            self.batch_draw_gpu_prepare_2d = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d = Duration::ZERO;
            self.batch_draw_gpu_acquire = Duration::ZERO;
            self.batch_draw_gpu_encode_main = Duration::ZERO;
            self.batch_draw_gpu_submit_main = Duration::ZERO;
            self.batch_draw_gpu_post_process = Duration::ZERO;
            self.batch_draw_gpu_accessibility = Duration::ZERO;
            self.batch_draw_gpu_present = Duration::ZERO;
            self.batch_present_drain_events = Duration::ZERO;
            self.batch_present_apply_events = Duration::ZERO;
            self.batch_idle = Duration::ZERO;
            self.batch_sim_delta_seconds = 0.0;
            self.batch_start = frame_end;
        }

        // Keep a continuous redraw chain like the legacy runner.
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        // Clear per-frame pressed/released flags after update to preserve
        // window events that arrived since the last frame.
        self.app.begin_input_frame();
    }
}

impl<B: GraphicsBackend> winit::application::ApplicationHandler for RunnerState<B> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        if self.window.is_none() {
            let attrs = window_attributes(event_loop, self.app.runtime.project(), &self.title);
            let window = Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("failed to create winit window"),
            );
            self.app.attach_window(window.clone());
            let initial_size = window.inner_size();
            self.app
                .resize_surface(initial_size.width, initial_size.height);
            // Draw once before showing the window to avoid a white first-frame flash.
            self.app.present();
            window.set_visible(true);
            self.window = Some(window);
            self.cursor_captured = false;
            self.capture_uses_raw_motion = false;
            if self.cursor_inside_window {
                self.set_cursor_capture(true);
            }
            let now = Instant::now();
            self.last_frame_start = now;
            self.last_frame_end = now;
            self.next_frame_deadline = now;
            self.run_start = now;
            self.fixed_accumulator = 0.0;
            self.batch_start = now;
            self.batch_frames = 0;
            self.batch_work = Duration::ZERO;
            self.batch_simulation = Duration::ZERO;
            self.batch_runtime_update = Duration::ZERO;
            self.batch_input_poll = Duration::ZERO;
            self.batch_fixed_update = Duration::ZERO;
            self.batch_runtime_start_schedule = Duration::ZERO;
            self.batch_runtime_snapshot_update = Duration::ZERO;
            self.batch_runtime_script_update = Duration::ZERO;
            self.batch_runtime_internal_update = Duration::ZERO;
            self.batch_runtime_slowest_script = Duration::ZERO;
            self.batch_runtime_script_count = 0;
            self.batch_present = Duration::ZERO;
            self.batch_present_extract_2d = Duration::ZERO;
            self.batch_present_extract_3d = Duration::ZERO;
            self.batch_present_drain_commands = Duration::ZERO;
            self.batch_present_submit_commands = Duration::ZERO;
            self.batch_present_draw_frame = Duration::ZERO;
            self.batch_draw_process_commands = Duration::ZERO;
            self.batch_draw_prepare_cpu = Duration::ZERO;
            self.batch_draw_gpu_prepare_2d = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d = Duration::ZERO;
            self.batch_draw_gpu_acquire = Duration::ZERO;
            self.batch_draw_gpu_encode_main = Duration::ZERO;
            self.batch_draw_gpu_submit_main = Duration::ZERO;
            self.batch_draw_gpu_post_process = Duration::ZERO;
            self.batch_draw_gpu_accessibility = Duration::ZERO;
            self.batch_draw_gpu_present = Duration::ZERO;
            self.batch_present_drain_events = Duration::ZERO;
            self.batch_present_apply_events = Duration::ZERO;
            self.batch_idle = Duration::ZERO;
            self.batch_sim_delta_seconds = 0.0;
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_some_and(|current_window| current_window.id() != window)
        {
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                self.app.resize_surface(size.width, size.height);
            }
            WindowEvent::Moved(_) => {
                // On Windows title-bar drag can suppress redraw cadence; tick on move events too.
                self.step_frame(Instant::now());
            }
            ref keyboard_event @ WindowEvent::KeyboardInput {
                event: ref key_event,
                ..
            } => {
                if key_event.state == ElementState::Pressed
                    && matches!(&key_event.physical_key, PhysicalKey::Code(KeyCode::Escape))
                {
                    self.set_cursor_capture(false);
                }
                self.kbm_input
                    .handle_window_event(&mut self.app, &keyboard_event);
            }
            mouse_event @ WindowEvent::MouseInput { state, .. } => {
                if state == ElementState::Pressed && !self.cursor_captured {
                    if self.cursor_inside_window {
                        self.set_cursor_capture(true);
                    }
                }
                self.kbm_input
                    .handle_window_event(&mut self.app, &mouse_event);
            }
            WindowEvent::CursorEntered { .. } => {
                self.cursor_inside_window = true;
            }
            cursor_left @ WindowEvent::CursorLeft { .. } => {
                self.cursor_inside_window = false;
                self.kbm_input
                    .handle_window_event(&mut self.app, &cursor_left);
            }
            cursor_moved @ WindowEvent::CursorMoved { .. } => {
                self.cursor_inside_window = true;
                self.kbm_input
                    .handle_window_event(&mut self.app, &cursor_moved);
            }
            WindowEvent::MouseWheel { .. } => {
                self.kbm_input.handle_window_event(&mut self.app, &event);
            }
            WindowEvent::Focused(true) => {
                if self.cursor_captured {
                    if let Some(window) = &self.window {
                        let (captured, uses_raw_motion) =
                            Self::apply_cursor_capture(window.as_ref(), true);
                        self.cursor_captured = captured;
                        self.capture_uses_raw_motion = uses_raw_motion;
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.cursor_inside_window = false;
                self.set_cursor_capture(false);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    self.app.resize_surface(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.step_frame(Instant::now());
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if self.cursor_captured
            && self.capture_uses_raw_motion
            && let DeviceEvent::MouseMotion { delta } = event
        {
            self.kbm_input
                .handle_mouse_motion(&mut self.app, delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        if self.fixed_timestep.is_none() && target_frame_duration(self.fps_cap).is_none() {
            // Uncapped: tick here to reduce latency between redraw events.
            self.step_frame(Instant::now());
            return;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn window_attributes(
    event_loop: &ActiveEventLoop,
    project: Option<&perro_runtime::RuntimeProject>,
    fallback_title: &str,
) -> WindowAttributes {
    let title = project
        .map(|project| project.config.name.as_str())
        .unwrap_or(fallback_title)
        .to_string();

    let mut attrs = WindowAttributes::default()
        .with_title(title)
        .with_visible(false);
    let Some(project) = project else {
        return attrs;
    };

    if let Some(icon) = load_project_window_icon(project) {
        attrs = attrs.with_window_icon(Some(icon));
    }

    let desired = PhysicalSize::new(project.config.virtual_width, project.config.virtual_height);
    if desired.width == 0 || desired.height == 0 {
        return attrs;
    }

    let Some(monitor) = pick_monitor(event_loop) else {
        return attrs.with_inner_size(Size::Physical(desired));
    };

    let max_width =
        ((monitor.size().width as f32) * INITIAL_WINDOW_MONITOR_FRACTION).floor() as u32;
    let max_height =
        ((monitor.size().height as f32) * INITIAL_WINDOW_MONITOR_FRACTION).floor() as u32;
    let fitted = fit_aspect(desired, max_width.max(1), max_height.max(1));
    let centered = center_position(&monitor, fitted);

    attrs = attrs.with_inner_size(Size::Physical(fitted));
    attrs.with_position(Position::Physical(centered))
}

fn load_project_window_icon(project: &perro_runtime::RuntimeProject) -> Option<Icon> {
    let bytes = load_project_icon_bytes(project)?;
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).ok()
}

fn load_project_icon_bytes(project: &perro_runtime::RuntimeProject) -> Option<Vec<u8>> {
    if let Some(icon_path) = resolve_project_icon_path(project)
        && let Ok(bytes) = fs::read(icon_path)
    {
        return Some(bytes);
    }

    let icon = project.config.icon.trim();
    if icon.starts_with("res://")
        && let Some(lookup) = project.static_icon_lookup
        && let Some(bytes) = lookup(icon)
    {
        return Some(bytes.to_vec());
    }

    None
}

fn resolve_project_icon_path(project: &perro_runtime::RuntimeProject) -> Option<PathBuf> {
    let icon = project.config.icon.trim();
    if icon.is_empty() {
        return None;
    }

    if let Some(rel) = icon.strip_prefix("res://") {
        let rel = rel.trim_start_matches('/');
        return Some(project.root.join("res").join(rel));
    }

    let path = Path::new(icon);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }

    Some(project.root.join(path))
}

fn pick_monitor(event_loop: &ActiveEventLoop) -> Option<MonitorHandle> {
    event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
}

fn fit_aspect(desired: PhysicalSize<u32>, max_width: u32, max_height: u32) -> PhysicalSize<u32> {
    if desired.width <= max_width && desired.height <= max_height {
        return desired;
    }

    let scale = f32::min(
        max_width as f32 / desired.width as f32,
        max_height as f32 / desired.height as f32,
    );
    let width = ((desired.width as f32) * scale).floor().max(1.0) as u32;
    let height = ((desired.height as f32) * scale).floor().max(1.0) as u32;
    PhysicalSize::new(width, height)
}

fn center_position(
    monitor: &MonitorHandle,
    window_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();

    let x = monitor_pos.x + ((monitor_size.width as i32 - window_size.width as i32) / 2);
    let y = monitor_pos.y + ((monitor_size.height as i32 - window_size.height as i32) / 2);
    PhysicalPosition::new(x, y)
}
