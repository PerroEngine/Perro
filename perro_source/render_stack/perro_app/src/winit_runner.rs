use crate::App;
use perro_graphics::GraphicsBackend;
use perro_ids::{NodeID, TextureID, string_to_u64};
use perro_io::decompress_zlib;
use perro_render_bridge::{
    Camera2DState, Command2D, Rect2DCommand, RenderCommand, RenderRequestID, ResourceCommand,
    Sprite2DCommand,
};
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
const STARTUP_SPLASH_FADE_DURATION: Duration = Duration::from_millis(320);
const STARTUP_SPLASH_HOLD_DURATION: Duration = Duration::from_millis(2000);
const STARTUP_SPLASH_HARD_TIMEOUT: Duration = Duration::from_millis(8000);
const STARTUP_SPLASH_BG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const STARTUP_SPLASH_MAX_WIDTH_FRAC: f32 = 0.44;
const STARTUP_SPLASH_MAX_HEIGHT_FRAC: f32 = 0.34;
const STARTUP_SPLASH_TEXTURE_REQUEST: RenderRequestID = RenderRequestID::new(0x5350_4C41_5348_5F54);
const STARTUP_SPLASH_BG_NODE: NodeID = NodeID::from_u64(string_to_u64("__startup_splash_bg__"));
const STARTUP_SPLASH_IMAGE_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__startup_splash_image__"));
const STARTUP_SPLASH_BG_Z: i32 = 950;
const STARTUP_SPLASH_IMAGE_Z: i32 = 951;
const PTEX_MAGIC: &[u8; 4] = b"PTEX";
const PTEX_FLAG_FORMAT_MASK: u32 = 0b11;
const PTEX_FLAG_FORMAT_RGBA8: u32 = 0;
const PTEX_FLAG_FORMAT_RGB8: u32 = 1;
const PTEX_FLAG_FORMAT_R8: u32 = 2;

struct StartupSplashState {
    active: bool,
    source: Option<String>,
    source_hash: Option<u64>,
    image_size: Option<(u32, u32)>,
    texture_requested: bool,
    texture_id: Option<TextureID>,
    ready_streak: u32,
    shown_at: Instant,
    fade_started_at: Option<Instant>,
    first_frame_inflight: Vec<RenderRequestID>,
    first_frame_captured: bool,
    debug_frame_counter: u32,
}

impl StartupSplashState {
    fn from_project(project: Option<&perro_runtime::RuntimeProject>, now: Instant) -> Self {
        let mut source = None::<String>;
        let mut source_hash = None::<u64>;
        if let Some(p) = project {
            let splash = p.config.startup_splash.trim();
            if !splash.is_empty() {
                source = Some(splash.to_string());
                source_hash = p.config.startup_splash_hash;
            } else {
                let icon = p.config.icon.trim();
                if !icon.is_empty() {
                    source = Some(icon.to_string());
                    source_hash = p.config.icon_hash;
                }
            }
        }
        let image_size = project.and_then(|p| {
            source
                .as_deref()
                .and_then(|s| load_image_size(p, s, source_hash))
        });
        Self {
            active: true,
            source,
            source_hash,
            image_size,
            texture_requested: false,
            texture_id: None,
            ready_streak: 0,
            shown_at: now,
            fade_started_at: None,
            first_frame_inflight: Vec::new(),
            first_frame_captured: false,
            debug_frame_counter: 0,
        }
    }

    #[inline]
    fn blocks_input(&self) -> bool {
        self.active
    }

    fn alpha(&self, now: Instant) -> f32 {
        let Some(started) = self.fade_started_at else {
            return 1.0;
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= STARTUP_SPLASH_FADE_DURATION {
            0.0
        } else {
            1.0 - (elapsed.as_secs_f32() / STARTUP_SPLASH_FADE_DURATION.as_secs_f32())
        }
    }

    fn should_finish(&self, now: Instant) -> bool {
        self.fade_started_at.is_some_and(|started| {
            now.saturating_duration_since(started) >= STARTUP_SPLASH_FADE_DURATION
        })
    }
}

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
    batch_draw_gpu_prepare_particles_3d: Duration,
    batch_draw_gpu_prepare_3d_frustum: Duration,
    batch_draw_gpu_prepare_3d_hiz: Duration,
    batch_draw_gpu_prepare_3d_indirect: Duration,
    batch_draw_gpu_prepare_3d_cull_inputs: Duration,
    batch_draw_gpu_acquire: Duration,
    batch_draw_gpu_acquire_surface: Duration,
    batch_draw_gpu_acquire_view: Duration,
    batch_draw_gpu_encode_main: Duration,
    batch_draw_gpu_submit_main: Duration,
    batch_draw_gpu_submit_finish_main: Duration,
    batch_draw_gpu_submit_queue_main: Duration,
    batch_draw_gpu_post_process: Duration,
    batch_draw_gpu_accessibility: Duration,
    batch_draw_gpu_present: Duration,
    batch_draw_calls_2d: u64,
    batch_draw_calls_3d: u64,
    batch_draw_calls_total: u64,
    batch_skip_prepare_2d: u64,
    batch_skip_prepare_3d: u64,
    batch_skip_prepare_particles_3d: u64,
    batch_skip_prepare_3d_frustum: u64,
    batch_skip_prepare_3d_hiz: u64,
    batch_skip_prepare_3d_indirect: u64,
    batch_skip_prepare_3d_cull_inputs: u64,
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
    startup_splash: StartupSplashState,
}

impl<B: GraphicsBackend> RunnerState<B> {
    fn new(app: App<B>, title: &str, fps_cap: f32, fixed_timestep: Option<f32>) -> Self {
        let now = Instant::now();
        let startup_splash = StartupSplashState::from_project(app.runtime.project(), now);
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
            batch_draw_gpu_prepare_particles_3d: Duration::ZERO,
            batch_draw_gpu_prepare_3d_frustum: Duration::ZERO,
            batch_draw_gpu_prepare_3d_hiz: Duration::ZERO,
            batch_draw_gpu_prepare_3d_indirect: Duration::ZERO,
            batch_draw_gpu_prepare_3d_cull_inputs: Duration::ZERO,
            batch_draw_gpu_acquire: Duration::ZERO,
            batch_draw_gpu_acquire_surface: Duration::ZERO,
            batch_draw_gpu_acquire_view: Duration::ZERO,
            batch_draw_gpu_encode_main: Duration::ZERO,
            batch_draw_gpu_submit_main: Duration::ZERO,
            batch_draw_gpu_submit_finish_main: Duration::ZERO,
            batch_draw_gpu_submit_queue_main: Duration::ZERO,
            batch_draw_gpu_post_process: Duration::ZERO,
            batch_draw_gpu_accessibility: Duration::ZERO,
            batch_draw_gpu_present: Duration::ZERO,
            batch_draw_calls_2d: 0,
            batch_draw_calls_3d: 0,
            batch_draw_calls_total: 0,
            batch_skip_prepare_2d: 0,
            batch_skip_prepare_3d: 0,
            batch_skip_prepare_particles_3d: 0,
            batch_skip_prepare_3d_frustum: 0,
            batch_skip_prepare_3d_hiz: 0,
            batch_skip_prepare_3d_indirect: 0,
            batch_skip_prepare_3d_cull_inputs: 0,
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
            startup_splash,
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

    fn startup_splash_overlay_commands(&mut self, alpha: f32) -> Vec<RenderCommand> {
        let alpha = alpha.clamp(0.0, 1.0);
        if let Some(result) = self
            .app
            .runtime
            .take_render_result(STARTUP_SPLASH_TEXTURE_REQUEST)
        {
            match result {
                perro_runtime::RuntimeRenderResult::Texture(id) => {
                    self.startup_splash.texture_id = Some(id);
                }
                perro_runtime::RuntimeRenderResult::Failed(_) => {
                    self.startup_splash.texture_requested = false;
                }
                perro_runtime::RuntimeRenderResult::Mesh(_)
                | perro_runtime::RuntimeRenderResult::Material(_) => {}
            }
        }
        let virtual_width = self
            .app
            .runtime
            .project()
            .map(|project| project.config.virtual_width.max(1))
            .unwrap_or(1920) as f32;
        let virtual_height = self
            .app
            .runtime
            .project()
            .map(|project| project.config.virtual_height.max(1))
            .unwrap_or(1080) as f32;

        let mut commands = Vec::with_capacity(3);
        commands.push(RenderCommand::TwoD(Command2D::SetCamera {
            camera: Camera2DState::default(),
        }));
        commands.push(RenderCommand::TwoD(Command2D::UpsertRect {
            node: STARTUP_SPLASH_BG_NODE,
            rect: Rect2DCommand {
                center: [0.0, 0.0],
                size: [virtual_width, virtual_height],
                color: [
                    STARTUP_SPLASH_BG_COLOR[0],
                    STARTUP_SPLASH_BG_COLOR[1],
                    STARTUP_SPLASH_BG_COLOR[2],
                    STARTUP_SPLASH_BG_COLOR[3] * alpha,
                ],
                z_index: STARTUP_SPLASH_BG_Z,
            },
        }));

        if !self.startup_splash.texture_requested
            && let Some(source) = self.startup_splash.source.clone()
        {
            self.startup_splash.texture_requested = true;
            commands.push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request: STARTUP_SPLASH_TEXTURE_REQUEST,
                id: TextureID::nil(),
                source: self
                    .startup_splash
                    .source_hash
                    .map(|v| v.to_string())
                    .unwrap_or(source),
                reserved: true,
            }));
        }

        let Some(texture_id) = self.startup_splash.texture_id else {
            return commands;
        };
        let (image_w, image_h) = self.startup_splash.image_size.unwrap_or((512, 512));
        let max_w = virtual_width * STARTUP_SPLASH_MAX_WIDTH_FRAC;
        let max_h = virtual_height * STARTUP_SPLASH_MAX_HEIGHT_FRAC;
        let scale = (max_w / image_w as f32)
            .min(max_h / image_h as f32)
            .max(0.001);
        let sx = scale;
        let sy = scale;
        commands.push(RenderCommand::TwoD(Command2D::UpsertSprite {
            node: STARTUP_SPLASH_IMAGE_NODE,
            sprite: Sprite2DCommand {
                texture: texture_id,
                model: [[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, 1.0]],
                tint: [1.0, 1.0, 1.0, alpha],
                z_index: STARTUP_SPLASH_IMAGE_Z,
            },
        }));
        commands
    }

    fn end_startup_splash(&mut self) {
        self.app
            .graphics
            .submit(RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_BG_NODE,
            }));
        self.app
            .graphics
            .submit(RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_IMAGE_NODE,
            }));
        self.startup_splash.active = false;
    }

    fn step_startup_frame(
        &mut self,
        frame_start: Instant,
        frame_delta: Duration,
        idle_duration: Duration,
    ) {
        let work_start = Instant::now();
        let mut runtime_update_duration = Duration::ZERO;

        let simulation_start = Instant::now();
        let input_poll_start = Instant::now();
        self.gamepad_input.begin_frame(&mut self.app);
        self.joycon_input.begin_frame(&mut self.app);
        let input_poll_duration = input_poll_start.elapsed();
        let fixed_start = Instant::now();

        let simulated_delta_seconds = {
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
                    self.fixed_accumulator = 0.0;
                }
                effective_fixed_step as f64 * steps as f64
            } else {
                let variable_step = frame_delta.as_secs_f32();
                let update_start = Instant::now();
                self.app.fixed_update_runtime(variable_step);
                runtime_update_duration += update_start.elapsed();
                variable_step as f64
            }
        };

        let fixed_duration = fixed_start.elapsed();
        let runtime_timing = self.app.update_runtime(frame_delta.as_secs_f32());
        runtime_update_duration += runtime_timing.total;
        let simulation_duration = simulation_start.elapsed();

        let alpha = self.startup_splash.alpha(frame_start);
        let splash_overlay = self.startup_splash_overlay_commands(alpha);
        let present_timing = self.app.present_with_overlay_timed(splash_overlay);
        let mut inflight_now = Vec::<RenderRequestID>::new();
        self.app
            .runtime
            .copy_inflight_render_requests(&mut inflight_now);
        if !self.startup_splash.first_frame_captured {
            self.startup_splash
                .first_frame_inflight
                .extend(inflight_now.iter().copied());
            self.startup_splash.first_frame_captured = true;
            println!(
                "[splash] captured first-frame inflight requests: {}",
                self.startup_splash.first_frame_inflight.len()
            );
        }
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
        self.batch_present += present_timing.total;
        self.batch_present_extract_2d += present_timing.extract_2d;
        self.batch_present_extract_3d += present_timing.extract_3d;
        self.batch_present_drain_commands += present_timing.drain_commands;
        self.batch_present_submit_commands += present_timing.submit_commands;
        self.batch_present_draw_frame += present_timing.draw_frame;
        self.batch_draw_process_commands += present_timing.draw_process_commands;
        self.batch_draw_prepare_cpu += present_timing.draw_prepare_cpu;
        self.batch_draw_gpu_prepare_2d += present_timing.draw_gpu_prepare_2d;
        self.batch_draw_gpu_prepare_3d += present_timing.draw_gpu_prepare_3d;
        self.batch_draw_gpu_prepare_particles_3d += present_timing.draw_gpu_prepare_particles_3d;
        self.batch_draw_gpu_prepare_3d_frustum += present_timing.draw_gpu_prepare_3d_frustum;
        self.batch_draw_gpu_prepare_3d_hiz += present_timing.draw_gpu_prepare_3d_hiz;
        self.batch_draw_gpu_prepare_3d_indirect += present_timing.draw_gpu_prepare_3d_indirect;
        self.batch_draw_gpu_prepare_3d_cull_inputs +=
            present_timing.draw_gpu_prepare_3d_cull_inputs;
        self.batch_draw_gpu_acquire += present_timing.draw_gpu_acquire;
        self.batch_draw_gpu_acquire_surface += present_timing.draw_gpu_acquire_surface;
        self.batch_draw_gpu_acquire_view += present_timing.draw_gpu_acquire_view;
        self.batch_draw_gpu_encode_main += present_timing.draw_gpu_encode_main;
        self.batch_draw_gpu_submit_main += present_timing.draw_gpu_submit_main;
        self.batch_draw_gpu_submit_finish_main += present_timing.draw_gpu_submit_finish_main;
        self.batch_draw_gpu_submit_queue_main += present_timing.draw_gpu_submit_queue_main;
        self.batch_draw_gpu_post_process += present_timing.draw_gpu_post_process;
        self.batch_draw_gpu_accessibility += present_timing.draw_gpu_accessibility;
        self.batch_draw_gpu_present += present_timing.draw_gpu_present;
        self.batch_draw_calls_2d += present_timing.draw_calls_2d as u64;
        self.batch_draw_calls_3d += present_timing.draw_calls_3d as u64;
        self.batch_draw_calls_total += present_timing.draw_calls_total as u64;
        self.batch_skip_prepare_2d += present_timing.skip_prepare_2d as u64;
        self.batch_skip_prepare_3d += present_timing.skip_prepare_3d as u64;
        self.batch_skip_prepare_particles_3d += present_timing.skip_prepare_particles_3d as u64;
        self.batch_skip_prepare_3d_frustum += present_timing.skip_prepare_3d_frustum as u64;
        self.batch_skip_prepare_3d_hiz += present_timing.skip_prepare_3d_hiz as u64;
        self.batch_skip_prepare_3d_indirect += present_timing.skip_prepare_3d_indirect as u64;
        self.batch_skip_prepare_3d_cull_inputs += present_timing.skip_prepare_3d_cull_inputs as u64;
        self.batch_present_drain_events += present_timing.drain_events;
        self.batch_present_apply_events += present_timing.apply_events;
        self.batch_idle += idle_duration;
        self.batch_sim_delta_seconds += simulated_delta_seconds;

        let pending_first_frame = if self.startup_splash.first_frame_captured {
            self.startup_splash
                .first_frame_inflight
                .iter()
                .copied()
                .filter(|request| self.app.runtime.is_render_request_inflight(*request))
                .count()
        } else {
            usize::MAX
        };
        let shown_for = frame_start.saturating_duration_since(self.startup_splash.shown_at);
        let hard_timeout_hit = shown_for >= STARTUP_SPLASH_HARD_TIMEOUT;
        if self.startup_splash.fade_started_at.is_none() {
            if shown_for >= STARTUP_SPLASH_HOLD_DURATION || hard_timeout_hit {
                self.startup_splash.fade_started_at = Some(frame_start);
            }
        }
        self.startup_splash.debug_frame_counter =
            self.startup_splash.debug_frame_counter.wrapping_add(1);
        if self.startup_splash.debug_frame_counter % 10 == 1 {
            println!(
                "[splash] active={} captured={} inflight_now={} tracked={} pending={} alpha={:.2} fade={} timeout={}",
                self.startup_splash.active,
                self.startup_splash.first_frame_captured,
                inflight_now.len(),
                self.startup_splash.first_frame_inflight.len(),
                if pending_first_frame == usize::MAX {
                    -1
                } else {
                    pending_first_frame as i32
                },
                alpha,
                self.startup_splash.fade_started_at.is_some(),
                hard_timeout_hit
            );
        }
        if self.startup_splash.should_finish(frame_start) {
            self.end_startup_splash();
            println!("[splash] finished");
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
        self.app.begin_input_frame();
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

        if self.startup_splash.active {
            self.step_startup_frame(frame_start, frame_delta, idle_duration);
            return;
        }

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
        self.batch_draw_gpu_prepare_particles_3d += present_timing.draw_gpu_prepare_particles_3d;
        self.batch_draw_gpu_prepare_3d_frustum += present_timing.draw_gpu_prepare_3d_frustum;
        self.batch_draw_gpu_prepare_3d_hiz += present_timing.draw_gpu_prepare_3d_hiz;
        self.batch_draw_gpu_prepare_3d_indirect += present_timing.draw_gpu_prepare_3d_indirect;
        self.batch_draw_gpu_prepare_3d_cull_inputs +=
            present_timing.draw_gpu_prepare_3d_cull_inputs;
        self.batch_draw_gpu_acquire += present_timing.draw_gpu_acquire;
        self.batch_draw_gpu_acquire_surface += present_timing.draw_gpu_acquire_surface;
        self.batch_draw_gpu_acquire_view += present_timing.draw_gpu_acquire_view;
        self.batch_draw_gpu_encode_main += present_timing.draw_gpu_encode_main;
        self.batch_draw_gpu_submit_main += present_timing.draw_gpu_submit_main;
        self.batch_draw_gpu_submit_finish_main += present_timing.draw_gpu_submit_finish_main;
        self.batch_draw_gpu_submit_queue_main += present_timing.draw_gpu_submit_queue_main;
        self.batch_draw_gpu_post_process += present_timing.draw_gpu_post_process;
        self.batch_draw_gpu_accessibility += present_timing.draw_gpu_accessibility;
        self.batch_draw_gpu_present += present_timing.draw_gpu_present;
        self.batch_draw_calls_2d += present_timing.draw_calls_2d as u64;
        self.batch_draw_calls_3d += present_timing.draw_calls_3d as u64;
        self.batch_draw_calls_total += present_timing.draw_calls_total as u64;
        self.batch_skip_prepare_2d += present_timing.skip_prepare_2d as u64;
        self.batch_skip_prepare_3d += present_timing.skip_prepare_3d as u64;
        self.batch_skip_prepare_particles_3d += present_timing.skip_prepare_particles_3d as u64;
        self.batch_skip_prepare_3d_frustum += present_timing.skip_prepare_3d_frustum as u64;
        self.batch_skip_prepare_3d_hiz += present_timing.skip_prepare_3d_hiz as u64;
        self.batch_skip_prepare_3d_indirect += present_timing.skip_prepare_3d_indirect as u64;
        self.batch_skip_prepare_3d_cull_inputs += present_timing.skip_prepare_3d_cull_inputs as u64;
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
            let avg_draw_gpu_prepare_particles_3d_us =
                self.batch_draw_gpu_prepare_particles_3d.as_micros() as f64
                    / self.batch_frames as f64;
            let avg_draw_gpu_prepare_3d_frustum_us =
                self.batch_draw_gpu_prepare_3d_frustum.as_micros() as f64
                    / self.batch_frames as f64;
            let avg_draw_gpu_prepare_3d_hiz_us =
                self.batch_draw_gpu_prepare_3d_hiz.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_prepare_3d_indirect_us =
                self.batch_draw_gpu_prepare_3d_indirect.as_micros() as f64
                    / self.batch_frames as f64;
            let avg_draw_gpu_prepare_3d_cull_inputs_us =
                self.batch_draw_gpu_prepare_3d_cull_inputs.as_micros() as f64
                    / self.batch_frames as f64;
            let avg_draw_gpu_acquire_us =
                self.batch_draw_gpu_acquire.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_acquire_surface_us =
                self.batch_draw_gpu_acquire_surface.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_acquire_view_us =
                self.batch_draw_gpu_acquire_view.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_encode_main_us =
                self.batch_draw_gpu_encode_main.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_submit_main_us =
                self.batch_draw_gpu_submit_main.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_submit_finish_main_us =
                self.batch_draw_gpu_submit_finish_main.as_micros() as f64
                    / self.batch_frames as f64;
            let avg_draw_gpu_submit_queue_main_us =
                self.batch_draw_gpu_submit_queue_main.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_post_process_us =
                self.batch_draw_gpu_post_process.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_accessibility_us =
                self.batch_draw_gpu_accessibility.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_gpu_present_us =
                self.batch_draw_gpu_present.as_micros() as f64 / self.batch_frames as f64;
            let avg_draw_calls_2d = self.batch_draw_calls_2d as f64 / self.batch_frames as f64;
            let avg_draw_calls_3d = self.batch_draw_calls_3d as f64 / self.batch_frames as f64;
            let avg_draw_calls_total =
                self.batch_draw_calls_total as f64 / self.batch_frames as f64;
            let avg_present_drain_events_us =
                self.batch_present_drain_events.as_micros() as f64 / self.batch_frames as f64;
            let avg_present_apply_events_us =
                self.batch_present_apply_events.as_micros() as f64 / self.batch_frames as f64;
            let pct_skip_prepare_2d =
                (self.batch_skip_prepare_2d as f64 * 100.0) / self.batch_frames as f64;
            let pct_skip_prepare_3d =
                (self.batch_skip_prepare_3d as f64 * 100.0) / self.batch_frames as f64;
            let pct_skip_prepare_particles_3d =
                (self.batch_skip_prepare_particles_3d as f64 * 100.0) / self.batch_frames as f64;
            let pct_skip_prepare_3d_frustum =
                (self.batch_skip_prepare_3d_frustum as f64 * 100.0) / self.batch_frames as f64;
            let pct_skip_prepare_3d_hiz =
                (self.batch_skip_prepare_3d_hiz as f64 * 100.0) / self.batch_frames as f64;
            let pct_skip_prepare_3d_indirect =
                (self.batch_skip_prepare_3d_indirect as f64 * 100.0) / self.batch_frames as f64;
            let pct_skip_prepare_3d_cull_inputs =
                (self.batch_skip_prepare_3d_cull_inputs as f64 * 100.0) / self.batch_frames as f64;
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
                "draw breakdown: process=({:.3}us) prep=({:.3}us) gpu_prepare2d=({:.3}us) gpu_prepare3d=({:.3}us) acquire=({:.3}us) encode=({:.3}us) gpu_submit=({:.3}us) post=({:.3}us) access=({:.3}us) present=({:.3}us) calls2d=({:.2}) calls3d=({:.2}) calls=({:.2})",
                avg_draw_process_commands_us,
                avg_draw_prepare_cpu_us,
                avg_draw_gpu_prepare_2d_us,
                avg_draw_gpu_prepare_3d_us,
                avg_draw_gpu_acquire_us,
                avg_draw_gpu_encode_main_us,
                avg_draw_gpu_submit_main_us,
                avg_draw_gpu_post_process_us,
                avg_draw_gpu_accessibility_us,
                avg_draw_gpu_present_us,
                avg_draw_calls_2d,
                avg_draw_calls_3d,
                avg_draw_calls_total
            );
            println!(
                "draw substeps: prep_particles3d=({:.3}us) prep_frustum=({:.3}us) prep_hiz=({:.3}us) prep_indirect=({:.3}us) prep_cull_inputs=({:.3}us) acquire_surface=({:.3}us) acquire_view=({:.3}us) submit_finish=({:.3}us) submit_queue=({:.3}us)",
                avg_draw_gpu_prepare_particles_3d_us,
                avg_draw_gpu_prepare_3d_frustum_us,
                avg_draw_gpu_prepare_3d_hiz_us,
                avg_draw_gpu_prepare_3d_indirect_us,
                avg_draw_gpu_prepare_3d_cull_inputs_us,
                avg_draw_gpu_acquire_surface_us,
                avg_draw_gpu_acquire_view_us,
                avg_draw_gpu_submit_finish_main_us,
                avg_draw_gpu_submit_queue_main_us
            );
            println!(
                "draw skips: prep2d=({:.1}%) prep3d=({:.1}%) prep_particles3d=({:.1}%) frustum=({:.1}%) hiz=({:.1}%) indirect=({:.1}%) cull_inputs=({:.1}%)",
                pct_skip_prepare_2d,
                pct_skip_prepare_3d,
                pct_skip_prepare_particles_3d,
                pct_skip_prepare_3d_frustum,
                pct_skip_prepare_3d_hiz,
                pct_skip_prepare_3d_indirect,
                pct_skip_prepare_3d_cull_inputs
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
            self.batch_draw_gpu_prepare_particles_3d = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_frustum = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_hiz = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_indirect = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_cull_inputs = Duration::ZERO;
            self.batch_draw_gpu_acquire = Duration::ZERO;
            self.batch_draw_gpu_acquire_surface = Duration::ZERO;
            self.batch_draw_gpu_acquire_view = Duration::ZERO;
            self.batch_draw_gpu_encode_main = Duration::ZERO;
            self.batch_draw_gpu_submit_main = Duration::ZERO;
            self.batch_draw_gpu_submit_finish_main = Duration::ZERO;
            self.batch_draw_gpu_submit_queue_main = Duration::ZERO;
            self.batch_draw_gpu_post_process = Duration::ZERO;
            self.batch_draw_gpu_accessibility = Duration::ZERO;
            self.batch_draw_gpu_present = Duration::ZERO;
            self.batch_draw_calls_2d = 0;
            self.batch_draw_calls_3d = 0;
            self.batch_draw_calls_total = 0;
            self.batch_skip_prepare_2d = 0;
            self.batch_skip_prepare_3d = 0;
            self.batch_skip_prepare_particles_3d = 0;
            self.batch_skip_prepare_3d_frustum = 0;
            self.batch_skip_prepare_3d_hiz = 0;
            self.batch_skip_prepare_3d_indirect = 0;
            self.batch_skip_prepare_3d_cull_inputs = 0;
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
            if self.startup_splash.active {
                let splash_overlay = self.startup_splash_overlay_commands(1.0);
                let _ = self.app.present_with_overlay_timed(splash_overlay);
            } else {
                self.app.present();
            }
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
            if self.startup_splash.active {
                self.startup_splash.shown_at = now;
                self.startup_splash.ready_streak = 0;
                self.startup_splash.fade_started_at = None;
                self.startup_splash.first_frame_inflight.clear();
                self.startup_splash.first_frame_captured = false;
                self.startup_splash.debug_frame_counter = 0;
            }
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
            self.batch_draw_gpu_prepare_particles_3d = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_frustum = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_hiz = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_indirect = Duration::ZERO;
            self.batch_draw_gpu_prepare_3d_cull_inputs = Duration::ZERO;
            self.batch_draw_gpu_acquire = Duration::ZERO;
            self.batch_draw_gpu_acquire_surface = Duration::ZERO;
            self.batch_draw_gpu_acquire_view = Duration::ZERO;
            self.batch_draw_gpu_encode_main = Duration::ZERO;
            self.batch_draw_gpu_submit_main = Duration::ZERO;
            self.batch_draw_gpu_submit_finish_main = Duration::ZERO;
            self.batch_draw_gpu_submit_queue_main = Duration::ZERO;
            self.batch_draw_gpu_post_process = Duration::ZERO;
            self.batch_draw_gpu_accessibility = Duration::ZERO;
            self.batch_draw_gpu_present = Duration::ZERO;
            self.batch_draw_calls_2d = 0;
            self.batch_draw_calls_3d = 0;
            self.batch_draw_calls_total = 0;
            self.batch_skip_prepare_2d = 0;
            self.batch_skip_prepare_3d = 0;
            self.batch_skip_prepare_particles_3d = 0;
            self.batch_skip_prepare_3d_frustum = 0;
            self.batch_skip_prepare_3d_hiz = 0;
            self.batch_skip_prepare_3d_indirect = 0;
            self.batch_skip_prepare_3d_cull_inputs = 0;
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
                if self.startup_splash.blocks_input() {
                    return;
                }
                if key_event.state == ElementState::Pressed
                    && matches!(&key_event.physical_key, PhysicalKey::Code(KeyCode::Escape))
                {
                    self.set_cursor_capture(false);
                }
                self.kbm_input
                    .handle_window_event(&mut self.app, &keyboard_event);
            }
            mouse_event @ WindowEvent::MouseInput { state, .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                if state == ElementState::Pressed && !self.cursor_captured {
                    if self.cursor_inside_window {
                        self.set_cursor_capture(true);
                    }
                }
                self.kbm_input
                    .handle_window_event(&mut self.app, &mouse_event);
            }
            WindowEvent::CursorEntered { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.cursor_inside_window = true;
            }
            cursor_left @ WindowEvent::CursorLeft { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.cursor_inside_window = false;
                self.kbm_input
                    .handle_window_event(&mut self.app, &cursor_left);
            }
            cursor_moved @ WindowEvent::CursorMoved { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.cursor_inside_window = true;
                self.kbm_input
                    .handle_window_event(&mut self.app, &cursor_moved);
            }
            WindowEvent::MouseWheel { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.kbm_input.handle_window_event(&mut self.app, &event);
            }
            WindowEvent::Focused(true) => {
                if self.startup_splash.blocks_input() {
                    return;
                }
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
                if self.startup_splash.blocks_input() {
                    return;
                }
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
        if self.startup_splash.blocks_input() {
            return;
        }
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
    let (rgba, width, height) = decode_image_rgba(&bytes)?;
    Icon::from_rgba(rgba, width, height).ok()
}

fn load_project_icon_bytes(project: &perro_runtime::RuntimeProject) -> Option<Vec<u8>> {
    load_project_image_bytes(project, project.config.icon.trim(), project.config.icon_hash)
}

fn load_project_image_bytes(
    project: &perro_runtime::RuntimeProject,
    source: &str,
    source_hash: Option<u64>,
) -> Option<Vec<u8>> {
    if let Some(path) = resolve_project_asset_path(project, source)
        && let Ok(bytes) = fs::read(path)
    {
        return Some(bytes);
    }
    if let Some(lookup) = project.static_icon_lookup {
        let hash = source_hash
            .or_else(|| perro_ids::parse_hashed_source_uri(source))
            .or_else(|| source.starts_with("res://").then(|| perro_ids::string_to_u64(source)));
        if let Some(hash) = hash {
            let bytes = lookup(hash);
            if !bytes.is_empty() {
                return Some(bytes.to_vec());
            }
        }
    }
    None
}

fn load_image_size(
    project: &perro_runtime::RuntimeProject,
    source: &str,
    source_hash: Option<u64>,
) -> Option<(u32, u32)> {
    let bytes = load_project_image_bytes(project, source, source_hash)?;
    decode_image_size(&bytes)
}

fn decode_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if let Some((width, height)) = decode_ptex_dimensions(bytes) {
        return Some((width.max(1), height.max(1)));
    }
    let decoded = image::load_from_memory(bytes).ok()?;
    Some((decoded.width().max(1), decoded.height().max(1)))
}

fn decode_image_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if let Some(decoded) = decode_ptex_rgba(bytes) {
        return Some(decoded);
    }
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), width, height))
}

fn decode_ptex_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 16 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != 2 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

fn decode_ptex_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 24 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != 2 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if flags & !PTEX_FLAG_FORMAT_MASK != 0 {
        return None;
    }
    let raw_len = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    let pixel_count = width.checked_mul(height)? as usize;
    let expected_raw_len = match flags & PTEX_FLAG_FORMAT_MASK {
        PTEX_FLAG_FORMAT_RGBA8 => pixel_count.checked_mul(4)?,
        PTEX_FLAG_FORMAT_RGB8 => pixel_count.checked_mul(3)?,
        PTEX_FLAG_FORMAT_R8 => pixel_count,
        _ => return None,
    };
    if raw_len as usize != expected_raw_len {
        return None;
    }
    let raw = decompress_zlib(&bytes[24..]).ok()?;
    if raw.len() != expected_raw_len {
        return None;
    }

    let rgba = match flags & PTEX_FLAG_FORMAT_MASK {
        PTEX_FLAG_FORMAT_RGBA8 => raw,
        PTEX_FLAG_FORMAT_RGB8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for px in raw.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        PTEX_FLAG_FORMAT_R8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &v in &raw {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        _ => return None,
    };
    Some((rgba, width, height))
}

fn resolve_project_asset_path(
    project: &perro_runtime::RuntimeProject,
    source: &str,
) -> Option<PathBuf> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }

    if let Some(rel) = source.strip_prefix("res://") {
        let rel = rel.trim_start_matches('/');
        return Some(project.root.join("res").join(rel));
    }

    let path = Path::new(source);
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

#[cfg(test)]
mod tests {
    use super::{decode_image_rgba, decode_image_size};

    #[test]
    fn decode_image_rgba_supports_ptex_v2_rgb() {
        let raw_rgb = vec![10u8, 20, 30, 40, 50, 60];
        let compressed = perro_io::compress_zlib_best(&raw_rgb).expect("compress");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PTEX");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes()); // rgb8
        bytes.extend_from_slice(&(raw_rgb.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let (rgba, width, height) = decode_image_rgba(&bytes).expect("decode rgba");
        assert_eq!((width, height), (2, 1));
        assert_eq!(rgba, vec![10u8, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn decode_image_size_supports_ptex() {
        let raw = vec![1u8, 2, 3, 4];
        let compressed = perro_io::compress_zlib_best(&raw).expect("compress");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PTEX");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // rgba8
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&compressed);

        assert_eq!(decode_image_size(&bytes), Some((1, 1)));
    }
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


