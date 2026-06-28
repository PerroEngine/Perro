//! Winit app runner, frame loop, input bridge, profiling, and window setup.

use crate::App;
#[cfg(not(target_arch = "wasm32"))]
use image_helpers::{PreloadedProjectImages, preload_project_images};
use perro_graphics::GraphicsBackend;
use perro_ids::TextureID;
use perro_input_api::MouseMode;
use perro_render_bridge::{
    Camera2DState, Command2D, Rect2DCommand, RenderCommand, RenderRequestID, ResourceCommand,
    Sprite2DCommand,
};
use perro_runtime::{WindowMode, WindowRequest};
use perro_runtime_api::sub_apis::FrameRateCap as RuntimeFrameRateCap;
use std::io::Write;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use std::{fs, sync::Arc};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
#[cfg(target_arch = "wasm32")]
use winit::monitor::MonitorHandle;
#[cfg(target_os = "android")]
use winit::platform::android::{EventLoopBuilderExtAndroid, activity::AndroidApp};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(not(target_arch = "wasm32"))]
use winit::{dpi::Position, monitor::MonitorHandle};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize, Size},
    event::{DeviceEvent, ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, CursorIcon as WinitCursorIcon, Fullscreen, Window, WindowAttributes},
};

pub(crate) mod image_helpers;
mod startup_splash;

use startup_splash::{
    STARTUP_SPLASH_BG_COLOR, STARTUP_SPLASH_BG_NODE, STARTUP_SPLASH_BG_Z,
    STARTUP_SPLASH_HARD_TIMEOUT, STARTUP_SPLASH_HOLD_DURATION, STARTUP_SPLASH_IMAGE_NODE,
    STARTUP_SPLASH_IMAGE_Z, STARTUP_SPLASH_MAX_HEIGHT_FRAC, STARTUP_SPLASH_MAX_WIDTH_FRAC,
    STARTUP_SPLASH_TEXTURE_REQUEST, StartupSplashState,
};

const DEFAULT_FIXED_TIMESTEP: Option<f32> = None;
const MAX_FIXED_STEPS_PER_FRAME: u32 = 2;
const MAX_FRAME_DELTA_SECONDS: f32 = 0.250;
const MIN_FRAME_RATE_CAP_FPS: f32 = 1.0;
const MAX_FRAME_RATE_CAP_FPS: f32 = 1000.0;
const HIGH_RATE_FRAME_INTERVAL: Duration = Duration::from_millis(8);
const LOG_INTERVAL_SECONDS: f32 = 3.0;
#[cfg(not(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps")))]
const LOG_TIMING_SAMPLE_STRIDE: u32 = 20;
const TIMING_WARMUP_FRAMES: u32 = 8;
#[cfg(not(target_arch = "wasm32"))]
const INITIAL_WINDOW_MONITOR_FRACTION: f32 = 0.75;

pub(crate) fn map_cursor_icon(icon: perro_ui::CursorIcon) -> WinitCursorIcon {
    match icon {
        perro_ui::CursorIcon::Default => WinitCursorIcon::Default,
        perro_ui::CursorIcon::ContextMenu => WinitCursorIcon::ContextMenu,
        perro_ui::CursorIcon::Help => WinitCursorIcon::Help,
        perro_ui::CursorIcon::Pointer => WinitCursorIcon::Pointer,
        perro_ui::CursorIcon::Progress => WinitCursorIcon::Progress,
        perro_ui::CursorIcon::Wait => WinitCursorIcon::Wait,
        perro_ui::CursorIcon::Cell => WinitCursorIcon::Cell,
        perro_ui::CursorIcon::Crosshair => WinitCursorIcon::Crosshair,
        perro_ui::CursorIcon::Text => WinitCursorIcon::Text,
        perro_ui::CursorIcon::VerticalText => WinitCursorIcon::VerticalText,
        perro_ui::CursorIcon::Alias => WinitCursorIcon::Alias,
        perro_ui::CursorIcon::Copy => WinitCursorIcon::Copy,
        perro_ui::CursorIcon::Move => WinitCursorIcon::Move,
        perro_ui::CursorIcon::NoDrop => WinitCursorIcon::NoDrop,
        perro_ui::CursorIcon::NotAllowed => WinitCursorIcon::NotAllowed,
        perro_ui::CursorIcon::Grab => WinitCursorIcon::Grab,
        perro_ui::CursorIcon::Grabbing => WinitCursorIcon::Grabbing,
        perro_ui::CursorIcon::EResize => WinitCursorIcon::EResize,
        perro_ui::CursorIcon::NResize => WinitCursorIcon::NResize,
        perro_ui::CursorIcon::NeResize => WinitCursorIcon::NeResize,
        perro_ui::CursorIcon::NwResize => WinitCursorIcon::NwResize,
        perro_ui::CursorIcon::SResize => WinitCursorIcon::SResize,
        perro_ui::CursorIcon::SeResize => WinitCursorIcon::SeResize,
        perro_ui::CursorIcon::SwResize => WinitCursorIcon::SwResize,
        perro_ui::CursorIcon::WResize => WinitCursorIcon::WResize,
        perro_ui::CursorIcon::EwResize => WinitCursorIcon::EwResize,
        perro_ui::CursorIcon::NsResize => WinitCursorIcon::NsResize,
        perro_ui::CursorIcon::NeswResize => WinitCursorIcon::NeswResize,
        perro_ui::CursorIcon::NwseResize => WinitCursorIcon::NwseResize,
        perro_ui::CursorIcon::ColResize => WinitCursorIcon::ColResize,
        perro_ui::CursorIcon::RowResize => WinitCursorIcon::RowResize,
        perro_ui::CursorIcon::AllScroll => WinitCursorIcon::AllScroll,
        perro_ui::CursorIcon::ZoomIn => WinitCursorIcon::ZoomIn,
        perro_ui::CursorIcon::ZoomOut => WinitCursorIcon::ZoomOut,
        perro_ui::CursorIcon::DndAsk => WinitCursorIcon::DndAsk,
        perro_ui::CursorIcon::AllResize => WinitCursorIcon::AllResize,
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
fn normalize_frame_rate_cap(cap: RuntimeFrameRateCap) -> RuntimeFrameRateCap {
    match cap {
        RuntimeFrameRateCap::Fps(fps) if fps.is_finite() && fps > 0.0 => {
            RuntimeFrameRateCap::Fps(fps.clamp(MIN_FRAME_RATE_CAP_FPS, MAX_FRAME_RATE_CAP_FPS))
        }
        RuntimeFrameRateCap::Fps(_) => RuntimeFrameRateCap::Unlimited,
        other => other,
    }
}

#[inline]
fn project_frame_rate_cap(cap: perro_runtime::FrameRateCap) -> RuntimeFrameRateCap {
    match cap {
        perro_runtime::FrameRateCap::Unlimited => RuntimeFrameRateCap::Unlimited,
        perro_runtime::FrameRateCap::Fps(fps) => RuntimeFrameRateCap::Fps(fps),
        perro_runtime::FrameRateCap::RefreshRate => RuntimeFrameRateCap::RefreshRate,
    }
}

#[inline]
fn frame_interval_from_fps(fps: f32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(fps))
}

#[cfg(not(target_arch = "wasm32"))]
fn active_refresh_rate_hz(window: Option<&Window>) -> Option<f32> {
    let monitor = window.and_then(Window::current_monitor)?;
    let refresh_millihertz = monitor
        .video_modes()
        .map(|mode| mode.refresh_rate_millihertz())
        .max()?;
    if refresh_millihertz == 0 {
        return None;
    }
    Some(refresh_millihertz as f32 / 1000.0)
}

#[cfg(target_arch = "wasm32")]
fn active_refresh_rate_hz(_window: Option<&Window>) -> Option<f32> {
    Some(60.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn refresh_rate_interval(window: Option<&Window>) -> Option<Duration> {
    let refresh_hz = active_refresh_rate_hz(window)?;
    Some(Duration::from_secs_f64(1.0 / f64::from(refresh_hz)))
}

#[cfg(target_arch = "wasm32")]
fn refresh_rate_interval(_window: Option<&Window>) -> Option<Duration> {
    Some(frame_interval_from_fps(60.0))
}

#[inline]
fn avg_micros(total: Duration, samples: u32) -> u128 {
    if samples == 0 {
        return 0;
    }
    total.as_micros() / u128::from(samples)
}

#[derive(Clone, Copy, Debug, Default)]
struct FixedStepPlan {
    steps: u32,
    step_seconds: f32,
    accumulator_after: f32,
    dropped_catchup: bool,
}

#[inline]
fn plan_fixed_steps(
    frame_delta_seconds: f32,
    fixed_timestep: f32,
    accumulator: f32,
) -> FixedStepPlan {
    let mut next_accumulator =
        accumulator + frame_delta_seconds.clamp(0.0, MAX_FRAME_DELTA_SECONDS);
    let mut steps = 0u32;
    while next_accumulator >= fixed_timestep && steps < MAX_FIXED_STEPS_PER_FRAME {
        next_accumulator -= fixed_timestep;
        steps += 1;
    }
    let dropped_catchup = steps == MAX_FIXED_STEPS_PER_FRAME && next_accumulator >= fixed_timestep;
    if dropped_catchup {
        next_accumulator %= fixed_timestep;
    }
    FixedStepPlan {
        steps,
        step_seconds: fixed_timestep,
        accumulator_after: next_accumulator,
        dropped_catchup,
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CsvFrameSample {
    frame_index: u64,
    phase: &'static str,
    warmup: bool,
    sampled: bool,
    frame_delta_us: u128,
    idle_before_frame_us: u128,
    simulation_us: u128,
    render_active_us: u128,
    work_active_us: u128,
    present_wait_us: u128,
    fixed_steps: u32,
    fixed_step_us: u128,
    fixed_accum_before_us: u128,
    fixed_accum_after_us: u128,
    fixed_catchup_dropped: bool,
}

struct TimingCsvWriter {
    file: fs::File,
}

impl TimingCsvWriter {
    fn from_env() -> Option<Self> {
        let path = std::env::var("PERRO_TIMING_CSV").ok()?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let _ = writeln!(
            file,
            "frame,phase,warmup,sampled,frame_delta_us,idle_before_frame_us,simulation_us,render_active_us,work_active_us,present_wait_us,fixed_steps,fixed_step_us,fixed_accum_before_us,fixed_accum_after_us,fixed_catchup_dropped"
        );
        Some(Self { file })
    }

    fn write(&mut self, sample: CsvFrameSample) {
        let _ = writeln!(
            self.file,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            sample.frame_index,
            sample.phase,
            if sample.warmup { 1 } else { 0 },
            if sample.sampled { 1 } else { 0 },
            sample.frame_delta_us,
            sample.idle_before_frame_us,
            sample.simulation_us,
            sample.render_active_us,
            sample.work_active_us,
            sample.present_wait_us,
            sample.fixed_steps,
            sample.fixed_step_us,
            sample.fixed_accum_before_us,
            sample.fixed_accum_after_us,
            if sample.fixed_catchup_dropped { 1 } else { 0 }
        );
        let _ = self.file.flush();
    }
}

#[cfg(feature = "profile_heavy")]
struct ProfileCsvWriter {
    file: fs::File,
}

#[cfg(feature = "profile_heavy")]
struct ProfileCsvRow {
    batch_end_frame: u64,
    frames: u32,
    sampled_frames: u32,
    avg_draw_calls_2d: f64,
    avg_draw_calls_3d: f64,
    avg_draw_calls_total: f64,
    avg_draw_instances_3d: f64,
    avg_instances_per_draw_3d: f64,
    avg_draw_material_refs_3d: f64,
    avg_render_commands: f64,
    avg_dirty_nodes: f64,
    avg_extract2d_us: f64,
    avg_extract3d_us: f64,
    avg_extract_ui_us: f64,
    avg_drain_commands_us: f64,
    avg_submit_commands_us: f64,
    avg_draw_process_us: f64,
    avg_draw_prep_us: f64,
    avg_active_meshes: f64,
    avg_active_materials: f64,
    avg_active_textures: f64,
    avg_present_wait_us: f64,
    avg_frame_us: f64,
}

#[cfg(feature = "profile_heavy")]
impl ProfileCsvWriter {
    fn from_env() -> Option<Self> {
        let path = std::env::var("PERRO_PROFILE_CSV").ok()?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let _ = writeln!(
            file,
            "batch_end_frame,frames,sampled_frames,avg_draw_calls_2d,avg_draw_calls_3d,avg_draw_calls_total,avg_draw_instances_3d,avg_instances_per_draw_3d,avg_draw_material_refs_3d,avg_render_commands,avg_dirty_nodes,avg_extract2d_us,avg_extract3d_us,avg_extract_ui_us,avg_drain_commands_us,avg_submit_commands_us,avg_draw_process_us,avg_draw_prep_us,avg_active_meshes,avg_active_materials,avg_active_textures,avg_present_wait_us,avg_frame_us"
        );
        Some(Self { file })
    }

    fn write(&mut self, row: &ProfileCsvRow) {
        let _ = writeln!(
            self.file,
            "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            row.batch_end_frame,
            row.frames,
            row.sampled_frames,
            row.avg_draw_calls_2d,
            row.avg_draw_calls_3d,
            row.avg_draw_calls_total,
            row.avg_draw_instances_3d,
            row.avg_instances_per_draw_3d,
            row.avg_draw_material_refs_3d,
            row.avg_render_commands,
            row.avg_dirty_nodes,
            row.avg_extract2d_us,
            row.avg_extract3d_us,
            row.avg_extract_ui_us,
            row.avg_drain_commands_us,
            row.avg_submit_commands_us,
            row.avg_draw_process_us,
            row.avg_draw_prep_us,
            row.avg_active_meshes,
            row.avg_active_materials,
            row.avg_active_textures,
            row.avg_present_wait_us,
            row.avg_frame_us,
        );
        let _ = self.file.flush();
    }
}

#[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
#[derive(Clone, Copy, Debug, Default)]
struct ProcessMemorySample {
    physical_mem: usize,
    virtual_mem: usize,
}

#[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
#[inline]
fn process_memory_sample() -> Option<ProcessMemorySample> {
    let sample = memory_stats::memory_stats()?;
    Some(ProcessMemorySample {
        physical_mem: sample.physical_mem,
        virtual_mem: sample.virtual_mem,
    })
}

#[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
struct MemProfileCsvWriter {
    file: fs::File,
}

#[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
struct MemProfileCsvSample {
    batch_end_frame: u64,
    sample: ProcessMemorySample,
    avg_update_us: u128,
    avg_render_us: u128,
    avg_idle_us: u128,
    avg_present_wait_us: u128,
    avg_frame_us: u128,
    avg_fps: f64,
}

#[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
impl MemProfileCsvWriter {
    fn from_env() -> Option<Self> {
        let path = std::env::var("PERRO_MEM_PROFILE_CSV").ok()?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let _ = writeln!(
            file,
            "batch_end_frame,rss_bytes,virtual_bytes,rss_mib,virtual_mib,avg_update_us,avg_render_us,avg_idle_us,avg_present_wait_us,avg_frame_us,avg_fps"
        );
        Some(Self { file })
    }

    fn write(&mut self, row: MemProfileCsvSample) {
        let _ = writeln!(
            self.file,
            "{},{},{},{:.6},{:.6},{},{},{},{},{},{:.6}",
            row.batch_end_frame,
            row.sample.physical_mem,
            row.sample.virtual_mem,
            bytes_to_mib(row.sample.physical_mem),
            bytes_to_mib(row.sample.virtual_mem),
            row.avg_update_us,
            row.avg_render_us,
            row.avg_idle_us,
            row.avg_present_wait_us,
            row.avg_frame_us,
            row.avg_fps,
        );
        let _ = self.file.flush();
    }
}

#[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
#[inline]
fn bytes_to_mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[cfg(all(feature = "fps", not(perro_no_console)))]
#[inline]
fn log_avg_sampled(
    update_us: u128,
    render_us: u128,
    total_us: u128,
    idle_before_frame_us: u128,
    present_wait_us: u128,
) {
    let frame_us = total_us
        .saturating_add(idle_before_frame_us)
        .saturating_add(present_wait_us);
    let fps_x100 = 100_000_000u128.checked_div(frame_us).unwrap_or(0) as u64;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(
        out,
        "timings: sim=({update_us}us) | gfx=({render_us}us) | work=({total_us}us) | idle=({idle_before_frame_us}us) | present_wait=({present_wait_us}us) | delta=({frame_us}us) | fps=({}.{:02})",
        fps_x100 / 100,
        fps_x100 % 100
    );
}

#[cfg(any(not(feature = "fps"), perro_no_console))]
#[inline]
fn log_avg_sampled(
    _update_us: u128,
    _render_us: u128,
    _total_us: u128,
    _idle_before_frame_us: u128,
    _present_wait_us: u128,
) {
}

pub struct WinitRunner;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppExitKind {
    WindowClose,
    EventLoopExit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppExitResult {
    pub kind: AppExitKind,
}

impl AppExitResult {
    pub fn window_close() -> Self {
        Self {
            kind: AppExitKind::WindowClose,
        }
    }

    pub fn event_loop_exit() -> Self {
        Self {
            kind: AppExitKind::EventLoopExit,
        }
    }

    pub fn is_success(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppExitError {
    pub message: String,
}

impl std::fmt::Display for AppExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppExitError {}

impl WinitRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run<B: GraphicsBackend + 'static>(
        self,
        app: App<B>,
        title: &str,
    ) -> Result<AppExitResult, AppExitError> {
        self.run_with_timestep(app, title, DEFAULT_FIXED_TIMESTEP)
    }

    pub fn run_with_timestep<B: GraphicsBackend + 'static>(
        self,
        app: App<B>,
        title: &str,
        fixed_timestep: Option<f32>,
    ) -> Result<AppExitResult, AppExitError> {
        self.run_with_timestep_and_preload(app, title, fixed_timestep, None)
    }

    pub(crate) fn run_with_timestep_and_preload<B: GraphicsBackend + 'static>(
        self,
        app: App<B>,
        title: &str,
        fixed_timestep: Option<f32>,
        #[cfg(not(target_arch = "wasm32"))] preloaded_images: Option<PreloadedProjectImages>,
        #[cfg(target_arch = "wasm32")] _preloaded_images: Option<()>,
    ) -> Result<AppExitResult, AppExitError> {
        let event_loop = EventLoop::new().map_err(|err| AppExitError {
            message: format!("failed to create winit event loop: {err}"),
        })?;
        #[cfg(target_arch = "wasm32")]
        {
            let state = RunnerState::new(app, title, fixed_timestep);
            event_loop.spawn_app(state);
            Ok(AppExitResult::event_loop_exit())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut state = RunnerState::new(app, title, fixed_timestep, preloaded_images);
            event_loop.run_app(&mut state).map_err(|err| AppExitError {
                message: format!("winit event loop failed: {err}"),
            })?;
            Ok(state
                .exit_result
                .take()
                .unwrap_or_else(AppExitResult::event_loop_exit))
        }
    }

    #[cfg(target_os = "android")]
    pub fn run_with_timestep_android<B: GraphicsBackend + 'static>(
        self,
        app: App<B>,
        title: &str,
        fixed_timestep: Option<f32>,
        android_app: AndroidApp,
    ) -> Result<AppExitResult, AppExitError> {
        let mut builder = EventLoop::<()>::builder();
        builder.with_android_app(android_app);
        let event_loop = builder.build().map_err(|err| AppExitError {
            message: format!("failed to create android winit event loop: {err}"),
        })?;
        let mut state = RunnerState::new(app, title, fixed_timestep, None);
        event_loop.run_app(&mut state).map_err(|err| AppExitError {
            message: format!("winit event loop failed: {err}"),
        })?;
        Ok(state
            .exit_result
            .take()
            .unwrap_or_else(AppExitResult::event_loop_exit))
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
    run_start: Instant,
    batch_start: Instant,
    batch_work: Duration,
    batch_simulation: Duration,
    batch_present: Duration,
    batch_idle_before_frame: Duration,
    batch_present_wait: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_input_poll: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_snapshot_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_script_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_internal_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_pre_transforms: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_collect: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_sync_world: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_apply_forces_impulses: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_step: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_sync_nodes: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_post_transforms: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_fixed_physics_signals: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_start_schedule: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_snapshot_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_script_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_internal_update: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_slowest_script: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_runtime_script_count: u64,
    #[cfg(feature = "profile_heavy")]
    batch_present_extract_2d: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_present_extract_3d: Duration,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_present_extract_ui: Duration,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_layout: Duration,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_commands: Duration,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_dirty_nodes: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_affected_nodes: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_recalculated_rects: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_cached_rects: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_auto_layout_batches: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_command_nodes: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_command_emitted: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_command_skipped: u64,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui_removed_nodes: u64,
    #[cfg(feature = "profile_heavy")]
    batch_present_drain_commands: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_present_submit_commands: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_present_draw_frame: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_process_commands: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_prepare_cpu: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_2d: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_3d: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_particles_3d: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_3d_frustum: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_3d_hiz: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_3d_indirect: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_prepare_3d_cull_inputs: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_acquire: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_acquire_surface: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_acquire_view: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_encode_main: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_submit_main: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_submit_finish_main: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_submit_queue_main: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_post_process: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_accessibility: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_gpu_present: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_draw_calls_2d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_draw_calls_3d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_draw_calls_total: u64,
    #[cfg(feature = "profile_heavy")]
    batch_draw_instances_3d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_draw_material_refs_3d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_2d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_3d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_particles_3d: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_3d_frustum: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_3d_hiz: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_3d_indirect: u64,
    #[cfg(feature = "profile_heavy")]
    batch_skip_prepare_3d_cull_inputs: u64,
    #[cfg(feature = "profile_heavy")]
    batch_present_drain_events: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_present_apply_events: Duration,
    batch_idle: Duration,
    #[cfg(feature = "profile_heavy")]
    batch_sim_delta_seconds: f64,
    fixed_timestep: Option<f32>,
    fixed_accumulator: f32,
    frame_rate_cap: RuntimeFrameRateCap,
    next_frame_deadline: Option<Instant>,
    frame_index: u64,
    timing_csv: Option<TimingCsvWriter>,
    #[cfg(feature = "profile_heavy")]
    profile_csv: Option<ProfileCsvWriter>,
    #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
    mem_profile_enabled: bool,
    #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
    mem_profile_csv: Option<MemProfileCsvWriter>,
    batch_frames: u32,
    batch_timing_samples: u32,
    timing_warmup_frames_left: u32,
    #[cfg(feature = "profile_heavy")]
    batch_render_command_count: u64,
    #[cfg(feature = "profile_heavy")]
    batch_dirty_node_count: u64,
    #[cfg(feature = "profile_heavy")]
    batch_active_meshes: u64,
    #[cfg(feature = "profile_heavy")]
    batch_active_materials: u64,
    #[cfg(feature = "profile_heavy")]
    batch_active_textures: u64,
    kbm_input: crate::input::KbmInput,
    gamepad_input: crate::input::GamepadInput,
    joycon_input: crate::input::JoyConInput,
    mouse_mode: MouseMode,
    mouse_uses_raw_motion: bool,
    cursor_icon: perro_ui::CursorIcon,
    window_requests: Vec<WindowRequest>,
    cursor_inside_window: bool,
    #[cfg(not(target_arch = "wasm32"))]
    last_window_position: Option<PhysicalPosition<i32>>,
    #[cfg(not(target_arch = "wasm32"))]
    preloaded_images: PreloadedProjectImages,
    startup_splash: StartupSplashState,
    exit_result: Option<AppExitResult>,
}

impl<B: GraphicsBackend> RunnerState<B> {
    fn new(
        app: App<B>,
        title: &str,
        fixed_timestep: Option<f32>,
        #[cfg(not(target_arch = "wasm32"))] preloaded_images: Option<PreloadedProjectImages>,
    ) -> Self {
        let now = Instant::now();
        #[cfg(not(target_arch = "wasm32"))]
        let preloaded_images =
            preloaded_images.unwrap_or_else(|| preload_project_images(app.runtime.project()));
        #[cfg(not(target_arch = "wasm32"))]
        let startup_splash =
            StartupSplashState::from_preloaded(preloaded_images.startup_splash.clone(), now);
        #[cfg(target_arch = "wasm32")]
        let startup_splash = StartupSplashState::from_preloaded(now);
        let normalized_fixed_timestep = normalize_fixed_timestep_seconds(fixed_timestep);
        let frame_rate_cap = app
            .runtime
            .project()
            .map(|project| project_frame_rate_cap(project.config.frame_rate_cap))
            .unwrap_or(RuntimeFrameRateCap::Unlimited);
        eprintln!("[perro][runtime] frame_rate_cap=({frame_rate_cap:?})");
        Self {
            app,
            title: title.to_owned(),
            window: None,
            fixed_timestep: normalized_fixed_timestep,
            fixed_accumulator: 0.0,
            frame_rate_cap: normalize_frame_rate_cap(frame_rate_cap),
            next_frame_deadline: None,
            last_frame_start: now,
            last_frame_end: now,
            run_start: now,
            timing_csv: TimingCsvWriter::from_env(),
            #[cfg(feature = "profile_heavy")]
            profile_csv: ProfileCsvWriter::from_env(),
            #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
            mem_profile_enabled: std::env::var("PERRO_MEM_PROFILE").ok().is_some_and(|raw| {
                let normalized = raw.trim().to_ascii_lowercase();
                matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
            }),
            #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
            mem_profile_csv: MemProfileCsvWriter::from_env(),
            batch_frames: 0,
            batch_timing_samples: 0,
            timing_warmup_frames_left: TIMING_WARMUP_FRAMES,
            #[cfg(feature = "profile_heavy")]
            batch_render_command_count: 0,
            #[cfg(feature = "profile_heavy")]
            batch_dirty_node_count: 0,
            #[cfg(feature = "profile_heavy")]
            batch_active_meshes: 0,
            #[cfg(feature = "profile_heavy")]
            batch_active_materials: 0,
            #[cfg(feature = "profile_heavy")]
            batch_active_textures: 0,
            batch_start: now,
            batch_work: Duration::ZERO,
            batch_simulation: Duration::ZERO,
            batch_present: Duration::ZERO,
            batch_idle_before_frame: Duration::ZERO,
            batch_present_wait: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_input_poll: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_snapshot_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_script_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_internal_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_pre_transforms: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_collect: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_sync_world: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_apply_forces_impulses: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_step: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_sync_nodes: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_post_transforms: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_fixed_physics_signals: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_start_schedule: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_snapshot_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_script_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_internal_update: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_slowest_script: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_runtime_script_count: 0,
            #[cfg(feature = "profile_heavy")]
            batch_present_extract_2d: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_present_extract_3d: Duration::ZERO,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_present_extract_ui: Duration::ZERO,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_layout: Duration::ZERO,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_commands: Duration::ZERO,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_dirty_nodes: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_affected_nodes: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_recalculated_rects: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_cached_rects: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_auto_layout_batches: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_command_nodes: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_command_emitted: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_command_skipped: 0,
            #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
            batch_ui_removed_nodes: 0,
            #[cfg(feature = "profile_heavy")]
            batch_present_drain_commands: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_present_submit_commands: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_present_draw_frame: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_process_commands: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_prepare_cpu: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_2d: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_3d: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_particles_3d: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_3d_frustum: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_3d_hiz: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_3d_indirect: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_prepare_3d_cull_inputs: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_acquire: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_acquire_surface: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_acquire_view: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_encode_main: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_submit_main: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_submit_finish_main: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_submit_queue_main: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_post_process: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_accessibility: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_gpu_present: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_draw_calls_2d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_draw_calls_3d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_draw_calls_total: 0,
            #[cfg(feature = "profile_heavy")]
            batch_draw_instances_3d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_draw_material_refs_3d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_2d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_3d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_particles_3d: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_3d_frustum: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_3d_hiz: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_3d_indirect: 0,
            #[cfg(feature = "profile_heavy")]
            batch_skip_prepare_3d_cull_inputs: 0,
            #[cfg(feature = "profile_heavy")]
            batch_present_drain_events: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_present_apply_events: Duration::ZERO,
            batch_idle: Duration::ZERO,
            #[cfg(feature = "profile_heavy")]
            batch_sim_delta_seconds: 0.0,
            frame_index: 0,
            kbm_input: crate::input::KbmInput::new(),
            gamepad_input: crate::input::GamepadInput::new(),
            joycon_input: crate::input::JoyConInput::new(),
            mouse_mode: MouseMode::Visible,
            mouse_uses_raw_motion: false,
            cursor_icon: perro_ui::CursorIcon::Default,
            window_requests: Vec::new(),
            cursor_inside_window: false,
            #[cfg(not(target_arch = "wasm32"))]
            last_window_position: None,
            #[cfg(not(target_arch = "wasm32"))]
            preloaded_images,
            startup_splash,
            exit_result: None,
        }
    }

    fn apply_mouse_mode(window: &Window, mode: MouseMode) -> (MouseMode, bool) {
        match mode {
            MouseMode::Visible => {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
                (MouseMode::Visible, false)
            }
            MouseMode::Hidden => {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(false);
                (MouseMode::Hidden, false)
            }
            MouseMode::Captured => match window.set_cursor_grab(CursorGrabMode::Locked) {
                Ok(_) => {
                    window.set_cursor_visible(false);
                    (MouseMode::Captured, true)
                }
                Err(_locked_err) => match window.set_cursor_grab(CursorGrabMode::Confined) {
                    Ok(_) => {
                        window.set_cursor_visible(false);
                        (MouseMode::Captured, false)
                    }
                    Err(_confined_err) => {
                        window.set_cursor_visible(true);
                        (MouseMode::Visible, false)
                    }
                },
            },
            MouseMode::Confined => match window.set_cursor_grab(CursorGrabMode::Confined) {
                Ok(_) => {
                    window.set_cursor_visible(true);
                    (MouseMode::Confined, false)
                }
                Err(_err) => {
                    window.set_cursor_visible(true);
                    (MouseMode::Visible, false)
                }
            },
            MouseMode::ConfinedHidden => match window.set_cursor_grab(CursorGrabMode::Confined) {
                Ok(_) => {
                    window.set_cursor_visible(false);
                    (MouseMode::ConfinedHidden, false)
                }
                Err(_err) => {
                    window.set_cursor_visible(false);
                    (MouseMode::Hidden, false)
                }
            },
        }
    }

    fn set_mouse_mode(&mut self, mode: MouseMode) {
        if self.mouse_mode == mode {
            return;
        }
        if let Some(window) = &self.window {
            let (applied_mode, uses_raw_motion) = Self::apply_mouse_mode(window.as_ref(), mode);
            if matches!(
                applied_mode,
                MouseMode::Captured | MouseMode::Confined | MouseMode::ConfinedHidden
            ) {
                center_cursor(window.as_ref());
            }
            self.mouse_mode = applied_mode;
            self.mouse_uses_raw_motion = uses_raw_motion;
            self.app.set_mouse_mode_state(applied_mode);
            self.app.clear_mouse_delta();
            self.kbm_input.reset_cursor_position();
        } else {
            self.mouse_mode = MouseMode::Visible;
            self.mouse_uses_raw_motion = false;
            self.app.set_mouse_mode_state(MouseMode::Visible);
        }
    }

    fn reset_mouse_mode_for_exit(&mut self) {
        if let Some(window) = &self.window {
            release_mouse(window.as_ref());
            self.mouse_mode = MouseMode::Visible;
            self.mouse_uses_raw_motion = false;
            self.app.set_mouse_mode_state(MouseMode::Visible);
            self.app.clear_mouse_delta();
            self.kbm_input.reset_cursor_position();
        } else {
            self.mouse_mode = MouseMode::Visible;
            self.mouse_uses_raw_motion = false;
            self.app.set_mouse_mode_state(MouseMode::Visible);
            self.app.clear_mouse_delta();
            self.kbm_input.reset_cursor_position();
        }
    }

    fn clear_keyboard_mouse_focus_state(&mut self) {
        self.cursor_inside_window = false;
        self.app.clear_keyboard_mouse_state();
        self.kbm_input.clear_focus_state();
    }

    fn apply_mouse_mode_request(&mut self) {
        self.app.apply_input_commands();
        if let Some(mode) = self.app.take_mouse_mode_request() {
            self.set_mouse_mode(mode);
        }
    }

    fn set_cursor_icon(&mut self, icon: perro_ui::CursorIcon) {
        if self.cursor_icon == icon {
            return;
        }
        if let Some(window) = &self.window {
            window.set_cursor(map_cursor_icon(icon));
        }
        self.cursor_icon = icon;
    }

    fn apply_cursor_icon_request(&mut self) {
        if let Some(icon) = self.app.take_cursor_icon_request() {
            self.set_cursor_icon(icon);
        }
    }

    fn apply_window_requests(&mut self, event_loop: &ActiveEventLoop) {
        self.app.drain_window_requests(&mut self.window_requests);
        if self.window_requests.is_empty() {
            return;
        }

        let Some(window) = self.window.as_ref().cloned() else {
            self.window_requests.clear();
            return;
        };

        let requests = std::mem::take(&mut self.window_requests);
        for request in requests {
            match request {
                WindowRequest::SetTitle(title) => window.set_title(&title),
                WindowRequest::SetSize { width, height } => {
                    let _ = window.request_inner_size(PhysicalSize::new(width, height));
                }
                WindowRequest::SetMode(WindowMode::Windowed) => {
                    window.set_fullscreen(None);
                }
                WindowRequest::SetMode(WindowMode::BorderlessFullscreen) => {
                    let monitor = window
                        .current_monitor()
                        .or_else(|| pick_monitor(event_loop));
                    window.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
                }
                WindowRequest::SetFrameRateCap(cap) => {
                    self.frame_rate_cap = normalize_frame_rate_cap(cap);
                    eprintln!(
                        "[perro][runtime] frame_rate_cap=({:?})",
                        self.frame_rate_cap
                    );
                    self.next_frame_deadline = None;
                }
                WindowRequest::SetCursorIcon(icon) => {
                    self.set_cursor_icon(icon);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn sync_window_position(&mut self, position: PhysicalPosition<i32>) {
        if let Some(prev) = self.last_window_position
            && self.cursor_inside_window
        {
            let dx = f64::from(prev.x - position.x);
            let dy = f64::from(prev.y - position.y);
            self.kbm_input.translate_cursor_position(dx, dy);
            if let Some(cursor) = self.kbm_input.last_cursor_position() {
                self.app
                    .set_mouse_position(cursor.x as f32, cursor.y as f32);
            }
        }
        self.last_window_position = Some(position);
    }

    #[cfg(target_arch = "wasm32")]
    fn sync_window_position(&mut self, _position: PhysicalPosition<i32>) {}

    fn frame_cap_interval(&self) -> Option<Duration> {
        match self.frame_rate_cap {
            RuntimeFrameRateCap::Unlimited => None,
            RuntimeFrameRateCap::Fps(fps) => Some(frame_interval_from_fps(fps)),
            RuntimeFrameRateCap::RefreshRate => refresh_rate_interval(self.window.as_deref())
                .or_else(|| Some(frame_interval_from_fps(60.0))),
        }
    }

    fn cap_blocks_frame(&mut self, now: Instant) -> bool {
        let Some(deadline) = self.next_frame_deadline else {
            return false;
        };
        deadline > now
    }

    fn update_frame_deadline(&mut self, frame_start: Instant, frame_end: Instant) {
        let Some(interval) = self.frame_cap_interval() else {
            self.next_frame_deadline = None;
            return;
        };
        let next = self
            .next_frame_deadline
            .and_then(|deadline| deadline.checked_add(interval))
            .filter(|deadline| *deadline > frame_end)
            .unwrap_or_else(|| frame_start.checked_add(interval).unwrap_or(frame_end));
        self.next_frame_deadline = Some(next);
    }

    fn apply_frame_control_flow(&self, event_loop: &ActiveEventLoop, now: Instant) {
        if self.startup_splash.active
            || matches!(self.frame_rate_cap, RuntimeFrameRateCap::Unlimited)
        {
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }
        if let Some(deadline) = self.next_frame_deadline
            && deadline > now
        {
            if self
                .frame_cap_interval()
                .is_some_and(|interval| interval <= HIGH_RATE_FRAME_INTERVAL)
            {
                event_loop.set_control_flow(ControlFlow::Poll);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }

    #[inline]
    fn should_sample_timing(&self) -> bool {
        #[cfg(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps"))]
        {
            true
        }
        #[cfg(not(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps")))]
        {
            self.frame_index == 1
                || self
                    .frame_index
                    .is_multiple_of(LOG_TIMING_SAMPLE_STRIDE as u64)
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
        let fallback_width = self
            .app
            .runtime
            .project()
            .map(|project| project.config.virtual_width.max(1))
            .unwrap_or(1920) as f32;
        let fallback_height = self
            .app
            .runtime
            .project()
            .map(|project| project.config.virtual_height.max(1))
            .unwrap_or(1080) as f32;
        let (window_width, window_height) = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .map(|size| (size.width.max(1) as f32, size.height.max(1) as f32))
            .unwrap_or((fallback_width, fallback_height));

        let mut commands = Vec::with_capacity(3);
        commands.push(RenderCommand::TwoD(Command2D::SetCamera {
            camera: Camera2DState::default(),
        }));
        commands.push(RenderCommand::TwoD(Command2D::UpsertRect {
            node: STARTUP_SPLASH_BG_NODE,
            rect: Rect2DCommand {
                center: [0.0, 0.0],
                size: [window_width, window_height],
                color: [
                    STARTUP_SPLASH_BG_COLOR[0],
                    STARTUP_SPLASH_BG_COLOR[1],
                    STARTUP_SPLASH_BG_COLOR[2],
                    STARTUP_SPLASH_BG_COLOR[3] * alpha,
                ]
                .into(),
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
        let (texture_w, texture_h) = self
            .startup_splash
            .texture_size
            .unwrap_or((image_w, image_h));
        let max_w = window_width * STARTUP_SPLASH_MAX_WIDTH_FRAC;
        let max_h = window_height * STARTUP_SPLASH_MAX_HEIGHT_FRAC;
        let scale = (max_w / image_w as f32)
            .min(max_h / image_h as f32)
            .max(0.001);
        commands.push(RenderCommand::TwoD(Command2D::UpsertSprite {
            node: STARTUP_SPLASH_IMAGE_NODE,
            sprite: Sprite2DCommand {
                texture: texture_id,
                model: [[scale, 0.0, 0.0], [0.0, scale, 0.0], [0.0, 0.0, 1.0]],
                tint: [1.0, 1.0, 1.0, alpha].into(),
                z_index: STARTUP_SPLASH_IMAGE_Z,
                uv_min: [0.0, 0.0],
                uv_max: [texture_w as f32, texture_h as f32],
                size: [image_w as f32, image_h as f32],
            },
        }));
        commands
    }

    fn end_startup_splash(&mut self) {
        self.app.graphics.submit_late_overlay_many([
            RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_BG_NODE,
            }),
            RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_IMAGE_NODE,
            }),
        ]);
        self.startup_splash.active = false;
        self.timing_warmup_frames_left = TIMING_WARMUP_FRAMES;
        self.batch_start = Instant::now();
        self.batch_frames = 0;
        self.batch_timing_samples = 0;
        self.batch_work = Duration::ZERO;
        self.batch_simulation = Duration::ZERO;
        self.batch_present = Duration::ZERO;
        self.batch_idle_before_frame = Duration::ZERO;
        self.batch_present_wait = Duration::ZERO;
        self.batch_idle = Duration::ZERO;
    }

    fn step_startup_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        frame_index: u64,
        frame_start: Instant,
        frame_delta: Duration,
        idle_duration: Duration,
    ) {
        let should_sample_timing = self.should_sample_timing();
        #[cfg(feature = "profile_heavy")]
        let work_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let work_start = should_sample_timing.then(Instant::now);
        let mut runtime_update_duration = Duration::ZERO;

        #[cfg(feature = "profile_heavy")]
        let simulation_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let simulation_start = should_sample_timing.then(Instant::now);
        #[cfg(feature = "profile_heavy")]
        let input_poll_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let input_poll_start = should_sample_timing.then(Instant::now);
        self.gamepad_input.begin_frame(&mut self.app);
        self.joycon_input.begin_frame(&mut self.app);
        #[cfg(feature = "profile_heavy")]
        let input_poll_duration = input_poll_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let input_poll_duration = input_poll_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let fixed_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let fixed_start = should_sample_timing.then(Instant::now);

        let fixed_accumulator_before = self.fixed_accumulator;
        let mut fixed_steps = 1u32;
        let mut fixed_step_seconds = frame_delta.as_secs_f32();
        let mut fixed_catchup_dropped = false;
        let simulated_delta_seconds = {
            if let Some(effective_fixed_step) = self.fixed_timestep {
                let plan = plan_fixed_steps(
                    frame_delta.as_secs_f32(),
                    effective_fixed_step,
                    self.fixed_accumulator,
                );
                fixed_steps = plan.steps;
                fixed_step_seconds = plan.step_seconds;
                fixed_catchup_dropped = plan.dropped_catchup;
                for _ in 0..plan.steps {
                    #[cfg(feature = "profile_heavy")]
                    {
                        let timing = self.app.fixed_update_runtime_timed(effective_fixed_step);
                        runtime_update_duration += timing.total;
                        self.batch_fixed_snapshot_update += timing.snapshot_update;
                        self.batch_fixed_script_update += timing.script_fixed_update;
                        self.batch_fixed_physics_update += timing.physics;
                        self.batch_fixed_internal_update += timing.internal_fixed_update;
                        self.batch_fixed_physics_pre_transforms += timing.physics_pre_transforms;
                        self.batch_fixed_physics_collect += timing.physics_collect;
                        self.batch_fixed_physics_sync_world += timing.physics_sync_world;
                        self.batch_fixed_physics_apply_forces_impulses +=
                            timing.physics_apply_forces_impulses;
                        self.batch_fixed_physics_step += timing.physics_step;
                        self.batch_fixed_physics_sync_nodes += timing.physics_sync_nodes;
                        self.batch_fixed_physics_post_transforms += timing.physics_post_transforms;
                        self.batch_fixed_physics_signals += timing.physics_signals;
                    }
                    #[cfg(not(feature = "profile_heavy"))]
                    {
                        let update_start = Instant::now();
                        self.app.fixed_update_runtime(effective_fixed_step);
                        runtime_update_duration += update_start.elapsed();
                    }
                }
                self.fixed_accumulator = plan.accumulator_after;
                self.app.set_physics_render_alpha(
                    (self.fixed_accumulator / effective_fixed_step).clamp(0.0, 1.0),
                );
                effective_fixed_step as f64 * plan.steps as f64
            } else {
                let variable_step = frame_delta.as_secs_f32();
                #[cfg(feature = "profile_heavy")]
                {
                    let timing = self.app.fixed_update_runtime_timed(variable_step);
                    runtime_update_duration += timing.total;
                    self.batch_fixed_snapshot_update += timing.snapshot_update;
                    self.batch_fixed_script_update += timing.script_fixed_update;
                    self.batch_fixed_physics_update += timing.physics;
                    self.batch_fixed_internal_update += timing.internal_fixed_update;
                    self.batch_fixed_physics_pre_transforms += timing.physics_pre_transforms;
                    self.batch_fixed_physics_collect += timing.physics_collect;
                    self.batch_fixed_physics_sync_world += timing.physics_sync_world;
                    self.batch_fixed_physics_apply_forces_impulses +=
                        timing.physics_apply_forces_impulses;
                    self.batch_fixed_physics_step += timing.physics_step;
                    self.batch_fixed_physics_sync_nodes += timing.physics_sync_nodes;
                    self.batch_fixed_physics_post_transforms += timing.physics_post_transforms;
                    self.batch_fixed_physics_signals += timing.physics_signals;
                }
                #[cfg(not(feature = "profile_heavy"))]
                {
                    let update_start = Instant::now();
                    self.app.fixed_update_runtime(variable_step);
                    runtime_update_duration += update_start.elapsed();
                }
                self.app.set_physics_render_alpha(1.0);
                variable_step as f64
            }
        };

        #[cfg(feature = "profile_heavy")]
        let fixed_duration = fixed_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let fixed_duration = fixed_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        let runtime_timing = self.app.update_runtime(frame_delta.as_secs_f32());
        runtime_update_duration += runtime_timing.total;
        self.apply_mouse_mode_request();
        self.apply_cursor_icon_request();
        self.apply_window_requests(event_loop);
        #[cfg(feature = "profile_heavy")]
        let simulation_duration = simulation_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let simulation_duration = simulation_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(not(feature = "profile_heavy"))]
        let _ = (
            runtime_update_duration,
            input_poll_duration,
            fixed_duration,
            runtime_timing,
            simulated_delta_seconds,
        );

        let alpha = self.startup_splash.alpha(frame_start);
        let splash_overlay = self.startup_splash_overlay_commands(alpha);
        #[cfg(feature = "profile_heavy")]
        let present_timing = self.app.present_with_overlay_timed_no_ui(splash_overlay);
        #[cfg(not(feature = "profile_heavy"))]
        let present_timing = if should_sample_timing {
            Some(self.app.present_with_overlay_timed_no_ui(splash_overlay))
        } else {
            self.app.present_with_overlay_no_ui(splash_overlay);
            None
        };
        self.apply_cursor_icon_request();
        let mut inflight_now = Vec::<RenderRequestID>::new();
        self.app
            .runtime
            .copy_inflight_render_requests(&mut inflight_now);
        if !self.startup_splash.first_frame_captured {
            self.startup_splash
                .first_frame_inflight
                .extend(inflight_now.iter().copied());
            self.startup_splash.first_frame_captured = true;
        }
        #[cfg(feature = "profile_heavy")]
        let work_duration = work_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let work_duration = work_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let present_wait_duration = present_timing.gpu_present;
        #[cfg(not(feature = "profile_heavy"))]
        let present_wait_duration = present_timing
            .as_ref()
            .map(|timing| timing.gpu_present)
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let present_active_duration = present_timing.active;
        #[cfg(not(feature = "profile_heavy"))]
        let present_active_duration = present_timing
            .as_ref()
            .map(|timing| timing.active)
            .unwrap_or(Duration::ZERO);
        let active_work_duration = work_duration.saturating_sub(present_wait_duration);
        let measured_frame_duration = active_work_duration
            .saturating_add(idle_duration)
            .saturating_add(present_wait_duration);
        let frame_end = Instant::now();
        self.last_frame_end = frame_end;
        if should_sample_timing {
            self.app.set_frame_timing(
                simulation_duration,
                present_active_duration,
                measured_frame_duration,
                if measured_frame_duration.is_zero() {
                    0.0
                } else {
                    1.0 / measured_frame_duration.as_secs_f32()
                },
            );
            #[cfg(feature = "profile_heavy")]
            self.app.set_present_timing_profile(&present_timing);
        }

        let warmup_frame = self.timing_warmup_frames_left > 0;
        if !warmup_frame {
            self.batch_frames = self.batch_frames.saturating_add(1);
            if should_sample_timing {
                self.batch_timing_samples = self.batch_timing_samples.saturating_add(1);
                self.batch_work += active_work_duration;
                self.batch_simulation += simulation_duration;
                self.batch_present += present_active_duration;
                self.batch_idle_before_frame += idle_duration;
                self.batch_present_wait += present_wait_duration;
                self.batch_idle += idle_duration + present_wait_duration;
            }
        }
        #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
        if !warmup_frame && let Some(timing) = present_timing.as_ref() {
            self.batch_present_extract_ui += timing.extract_ui;
            self.batch_ui_layout += timing.ui_layout;
            self.batch_ui_commands += timing.ui_commands;
            self.batch_ui_dirty_nodes += timing.ui_dirty_nodes as u64;
            self.batch_ui_affected_nodes += timing.ui_affected_nodes as u64;
            self.batch_ui_recalculated_rects += timing.ui_recalculated_rects as u64;
            self.batch_ui_cached_rects += timing.ui_cached_rects as u64;
            self.batch_ui_auto_layout_batches += timing.ui_auto_layout_batches as u64;
            self.batch_ui_command_nodes += timing.ui_command_nodes as u64;
            self.batch_ui_command_emitted += timing.ui_command_emitted as u64;
            self.batch_ui_command_skipped += timing.ui_command_skipped as u64;
            self.batch_ui_removed_nodes += timing.ui_removed_nodes as u64;
        }
        if frame_index != 1
            && let Some(csv) = &mut self.timing_csv
        {
            csv.write(CsvFrameSample {
                frame_index,
                phase: "startup",
                warmup: true,
                sampled: should_sample_timing,
                frame_delta_us: frame_delta.as_micros(),
                idle_before_frame_us: idle_duration.as_micros(),
                simulation_us: simulation_duration.as_micros(),
                render_active_us: present_active_duration.as_micros(),
                work_active_us: active_work_duration.as_micros(),
                present_wait_us: present_wait_duration.as_micros(),
                fixed_steps,
                fixed_step_us: Duration::from_secs_f32(fixed_step_seconds).as_micros(),
                fixed_accum_before_us: Duration::from_secs_f32(fixed_accumulator_before)
                    .as_micros(),
                fixed_accum_after_us: Duration::from_secs_f32(self.fixed_accumulator).as_micros(),
                fixed_catchup_dropped,
            });
        }
        #[cfg(feature = "profile_heavy")]
        if !warmup_frame {
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
            self.batch_present_extract_2d += present_timing.extract_2d;
            self.batch_present_extract_3d += present_timing.extract_3d;
            self.batch_present_extract_ui += present_timing.extract_ui;
            self.batch_ui_layout += present_timing.ui_layout;
            self.batch_ui_commands += present_timing.ui_commands;
            self.batch_ui_dirty_nodes += present_timing.ui_dirty_nodes as u64;
            self.batch_ui_affected_nodes += present_timing.ui_affected_nodes as u64;
            self.batch_ui_recalculated_rects += present_timing.ui_recalculated_rects as u64;
            self.batch_ui_cached_rects += present_timing.ui_cached_rects as u64;
            self.batch_ui_auto_layout_batches += present_timing.ui_auto_layout_batches as u64;
            self.batch_ui_command_nodes += present_timing.ui_command_nodes as u64;
            self.batch_ui_command_emitted += present_timing.ui_command_emitted as u64;
            self.batch_ui_command_skipped += present_timing.ui_command_skipped as u64;
            self.batch_ui_removed_nodes += present_timing.ui_removed_nodes as u64;
            self.batch_present_drain_commands += present_timing.drain_commands;
            self.batch_present_submit_commands += present_timing.submit_commands;
            self.batch_present_draw_frame += present_timing.gpu_present;
            self.batch_draw_process_commands += present_timing.draw_process_commands;
            self.batch_draw_prepare_cpu += present_timing.draw_prepare_cpu;
            self.batch_draw_gpu_prepare_2d += present_timing.draw_gpu_prepare_2d;
            self.batch_draw_gpu_prepare_3d += present_timing.draw_gpu_prepare_3d;
            self.batch_draw_gpu_prepare_particles_3d +=
                present_timing.draw_gpu_prepare_particles_3d;
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
            self.batch_draw_instances_3d += present_timing.draw_instances_3d as u64;
            self.batch_draw_material_refs_3d += present_timing.draw_material_refs_3d as u64;
            self.batch_render_command_count += present_timing.render_command_count as u64;
            self.batch_dirty_node_count += present_timing.dirty_node_count as u64;
            self.batch_active_meshes += present_timing.active_meshes as u64;
            self.batch_active_materials += present_timing.active_materials as u64;
            self.batch_active_textures += present_timing.active_textures as u64;
            self.batch_skip_prepare_2d += present_timing.skip_prepare_2d as u64;
            self.batch_skip_prepare_3d += present_timing.skip_prepare_3d as u64;
            self.batch_skip_prepare_particles_3d += present_timing.skip_prepare_particles_3d as u64;
            self.batch_skip_prepare_3d_frustum += present_timing.skip_prepare_3d_frustum as u64;
            self.batch_skip_prepare_3d_hiz += present_timing.skip_prepare_3d_hiz as u64;
            self.batch_skip_prepare_3d_indirect += present_timing.skip_prepare_3d_indirect as u64;
            self.batch_skip_prepare_3d_cull_inputs +=
                present_timing.skip_prepare_3d_cull_inputs as u64;
            self.batch_present_drain_events += present_timing.drain_events;
            self.batch_present_apply_events += present_timing.apply_events;
            self.batch_sim_delta_seconds += simulated_delta_seconds;
        }

        let shown_for = frame_start.saturating_duration_since(self.startup_splash.shown_at);
        let hard_timeout_hit = shown_for >= STARTUP_SPLASH_HARD_TIMEOUT;
        if self.startup_splash.fade_started_at.is_none()
            && (shown_for >= STARTUP_SPLASH_HOLD_DURATION || hard_timeout_hit)
        {
            self.startup_splash.fade_started_at = Some(frame_start);
        }
        if self.startup_splash.should_finish(frame_start) {
            self.end_startup_splash();
        }

        self.app.begin_input_frame();
    }

    fn step_frame(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
        if event_loop.exiting() || self.exit_result.is_some() {
            return;
        }
        self.app
            .runtime
            .set_active_refresh_rate(active_refresh_rate_hz(self.window.as_deref()));
        if !self.startup_splash.active && self.cap_blocks_frame(now) {
            self.apply_frame_control_flow(event_loop, now);
            return;
        }
        self.frame_index = self.frame_index.saturating_add(1);
        let frame_index = self.frame_index;
        let frame_start = now;
        let frame_delta = frame_start.duration_since(self.last_frame_start);
        self.last_frame_start = frame_start;

        let elapsed_since_start = frame_start.duration_since(self.run_start);
        self.app.set_elapsed_time(elapsed_since_start.as_secs_f32());
        let simulated_delta_seconds;
        let should_sample_timing = self.should_sample_timing();

        let idle_duration = frame_start.saturating_duration_since(self.last_frame_end);

        if self.startup_splash.active {
            self.step_startup_frame(
                event_loop,
                frame_index,
                frame_start,
                frame_delta,
                idle_duration,
            );
            return;
        }

        #[cfg(feature = "profile_heavy")]
        let work_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let work_start = should_sample_timing.then(Instant::now);
        let mut runtime_update_duration = Duration::ZERO;

        #[cfg(feature = "profile_heavy")]
        let simulation_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let simulation_start = should_sample_timing.then(Instant::now);
        #[cfg(feature = "profile_heavy")]
        let input_poll_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let input_poll_start = should_sample_timing.then(Instant::now);
        // Poll device inputs before update so scripts see the latest state.
        self.gamepad_input.begin_frame(&mut self.app);
        self.joycon_input.begin_frame(&mut self.app);
        #[cfg(feature = "profile_heavy")]
        let input_poll_duration = input_poll_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let input_poll_duration = input_poll_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let fixed_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let fixed_start = should_sample_timing.then(Instant::now);

        let fixed_accumulator_before = self.fixed_accumulator;
        let mut fixed_steps = 1u32;
        let mut fixed_step_seconds = frame_delta.as_secs_f32();
        let mut fixed_catchup_dropped = false;
        {
            if let Some(effective_fixed_step) = self.fixed_timestep {
                let plan = plan_fixed_steps(
                    frame_delta.as_secs_f32(),
                    effective_fixed_step,
                    self.fixed_accumulator,
                );
                fixed_steps = plan.steps;
                fixed_step_seconds = plan.step_seconds;
                fixed_catchup_dropped = plan.dropped_catchup;
                for _ in 0..plan.steps {
                    #[cfg(feature = "profile_heavy")]
                    {
                        let timing = self.app.fixed_update_runtime_timed(effective_fixed_step);
                        runtime_update_duration += timing.total;
                        self.batch_fixed_snapshot_update += timing.snapshot_update;
                        self.batch_fixed_script_update += timing.script_fixed_update;
                        self.batch_fixed_physics_update += timing.physics;
                        self.batch_fixed_internal_update += timing.internal_fixed_update;
                        self.batch_fixed_physics_pre_transforms += timing.physics_pre_transforms;
                        self.batch_fixed_physics_collect += timing.physics_collect;
                        self.batch_fixed_physics_sync_world += timing.physics_sync_world;
                        self.batch_fixed_physics_apply_forces_impulses +=
                            timing.physics_apply_forces_impulses;
                        self.batch_fixed_physics_step += timing.physics_step;
                        self.batch_fixed_physics_sync_nodes += timing.physics_sync_nodes;
                        self.batch_fixed_physics_post_transforms += timing.physics_post_transforms;
                        self.batch_fixed_physics_signals += timing.physics_signals;
                    }
                    #[cfg(not(feature = "profile_heavy"))]
                    {
                        let update_start = Instant::now();
                        self.app.fixed_update_runtime(effective_fixed_step);
                        runtime_update_duration += update_start.elapsed();
                    }
                }
                self.fixed_accumulator = plan.accumulator_after;
                self.app.set_physics_render_alpha(
                    (self.fixed_accumulator / effective_fixed_step).clamp(0.0, 1.0),
                );
                simulated_delta_seconds = effective_fixed_step as f64 * plan.steps as f64;
            } else {
                let variable_step = frame_delta.as_secs_f32();
                #[cfg(feature = "profile_heavy")]
                {
                    let timing = self.app.fixed_update_runtime_timed(variable_step);
                    runtime_update_duration += timing.total;
                    self.batch_fixed_snapshot_update += timing.snapshot_update;
                    self.batch_fixed_script_update += timing.script_fixed_update;
                    self.batch_fixed_physics_update += timing.physics;
                    self.batch_fixed_internal_update += timing.internal_fixed_update;
                    self.batch_fixed_physics_pre_transforms += timing.physics_pre_transforms;
                    self.batch_fixed_physics_collect += timing.physics_collect;
                    self.batch_fixed_physics_sync_world += timing.physics_sync_world;
                    self.batch_fixed_physics_apply_forces_impulses +=
                        timing.physics_apply_forces_impulses;
                    self.batch_fixed_physics_step += timing.physics_step;
                    self.batch_fixed_physics_sync_nodes += timing.physics_sync_nodes;
                    self.batch_fixed_physics_post_transforms += timing.physics_post_transforms;
                    self.batch_fixed_physics_signals += timing.physics_signals;
                }
                #[cfg(not(feature = "profile_heavy"))]
                {
                    let update_start = Instant::now();
                    self.app.fixed_update_runtime(variable_step);
                    runtime_update_duration += update_start.elapsed();
                }
                self.app.set_physics_render_alpha(1.0);
                simulated_delta_seconds = variable_step as f64;
            }
        }

        #[cfg(feature = "profile_heavy")]
        let fixed_duration = fixed_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let fixed_duration = fixed_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        let runtime_timing = self.app.update_runtime(frame_delta.as_secs_f32());
        runtime_update_duration += runtime_timing.total;
        self.apply_mouse_mode_request();
        self.apply_cursor_icon_request();
        self.apply_window_requests(event_loop);
        #[cfg(feature = "profile_heavy")]
        let simulation_duration = simulation_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let simulation_duration = simulation_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(not(feature = "profile_heavy"))]
        let _ = (
            runtime_update_duration,
            input_poll_duration,
            fixed_duration,
            runtime_timing,
            simulated_delta_seconds,
        );

        #[cfg(feature = "profile_heavy")]
        let present_timing = self.app.present_timed();
        #[cfg(not(feature = "profile_heavy"))]
        let present_timing = if should_sample_timing {
            Some(self.app.present_timed())
        } else {
            self.app.present();
            None
        };
        self.apply_cursor_icon_request();
        #[cfg(feature = "profile_heavy")]
        let work_duration = work_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let work_duration = work_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let present_wait_duration = present_timing.gpu_present;
        #[cfg(not(feature = "profile_heavy"))]
        let present_wait_duration = present_timing
            .as_ref()
            .map(|timing| timing.gpu_present)
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let present_active_duration = present_timing.active;
        #[cfg(not(feature = "profile_heavy"))]
        let present_active_duration = present_timing
            .as_ref()
            .map(|timing| timing.active)
            .unwrap_or(Duration::ZERO);
        let active_work_duration = work_duration.saturating_sub(present_wait_duration);
        let measured_frame_duration = active_work_duration
            .saturating_add(idle_duration)
            .saturating_add(present_wait_duration);

        let frame_end = Instant::now();
        self.last_frame_end = frame_end;
        self.update_frame_deadline(frame_start, frame_end);
        if should_sample_timing {
            self.app.set_frame_timing(
                simulation_duration,
                present_active_duration,
                measured_frame_duration,
                if measured_frame_duration.is_zero() {
                    0.0
                } else {
                    1.0 / measured_frame_duration.as_secs_f32()
                },
            );
            #[cfg(feature = "profile_heavy")]
            self.app.set_present_timing_profile(&present_timing);
        }

        let warmup_frame = self.timing_warmup_frames_left > 0;
        if !warmup_frame {
            self.batch_frames = self.batch_frames.saturating_add(1);
            if should_sample_timing {
                self.batch_timing_samples = self.batch_timing_samples.saturating_add(1);
                self.batch_work += active_work_duration;
                self.batch_simulation += simulation_duration;
                self.batch_present += present_active_duration;
                self.batch_idle_before_frame += idle_duration;
                self.batch_present_wait += present_wait_duration;
                self.batch_idle += idle_duration + present_wait_duration;
            }
        }
        #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
        if !warmup_frame && let Some(timing) = present_timing.as_ref() {
            self.batch_present_extract_ui += timing.extract_ui;
            self.batch_ui_layout += timing.ui_layout;
            self.batch_ui_commands += timing.ui_commands;
            self.batch_ui_dirty_nodes += timing.ui_dirty_nodes as u64;
            self.batch_ui_affected_nodes += timing.ui_affected_nodes as u64;
            self.batch_ui_recalculated_rects += timing.ui_recalculated_rects as u64;
            self.batch_ui_cached_rects += timing.ui_cached_rects as u64;
            self.batch_ui_auto_layout_batches += timing.ui_auto_layout_batches as u64;
            self.batch_ui_command_nodes += timing.ui_command_nodes as u64;
            self.batch_ui_command_emitted += timing.ui_command_emitted as u64;
            self.batch_ui_command_skipped += timing.ui_command_skipped as u64;
            self.batch_ui_removed_nodes += timing.ui_removed_nodes as u64;
        }
        if let Some(csv) = &mut self.timing_csv {
            csv.write(CsvFrameSample {
                frame_index,
                phase: "steady",
                warmup: warmup_frame,
                sampled: should_sample_timing,
                frame_delta_us: frame_delta.as_micros(),
                idle_before_frame_us: idle_duration.as_micros(),
                simulation_us: simulation_duration.as_micros(),
                render_active_us: present_active_duration.as_micros(),
                work_active_us: active_work_duration.as_micros(),
                present_wait_us: present_wait_duration.as_micros(),
                fixed_steps,
                fixed_step_us: Duration::from_secs_f32(fixed_step_seconds).as_micros(),
                fixed_accum_before_us: Duration::from_secs_f32(fixed_accumulator_before)
                    .as_micros(),
                fixed_accum_after_us: Duration::from_secs_f32(self.fixed_accumulator).as_micros(),
                fixed_catchup_dropped,
            });
        }
        if warmup_frame {
            self.timing_warmup_frames_left = self.timing_warmup_frames_left.saturating_sub(1);
            if self.timing_warmup_frames_left == 0 {
                self.batch_start = frame_end;
            }
            self.app.begin_input_frame();
            return;
        }
        #[cfg(feature = "profile_heavy")]
        {
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
            self.batch_present_extract_2d += present_timing.extract_2d;
            self.batch_present_extract_3d += present_timing.extract_3d;
            self.batch_present_extract_ui += present_timing.extract_ui;
            self.batch_ui_layout += present_timing.ui_layout;
            self.batch_ui_commands += present_timing.ui_commands;
            self.batch_ui_dirty_nodes += present_timing.ui_dirty_nodes as u64;
            self.batch_ui_affected_nodes += present_timing.ui_affected_nodes as u64;
            self.batch_ui_recalculated_rects += present_timing.ui_recalculated_rects as u64;
            self.batch_ui_cached_rects += present_timing.ui_cached_rects as u64;
            self.batch_ui_auto_layout_batches += present_timing.ui_auto_layout_batches as u64;
            self.batch_ui_command_nodes += present_timing.ui_command_nodes as u64;
            self.batch_ui_command_emitted += present_timing.ui_command_emitted as u64;
            self.batch_ui_command_skipped += present_timing.ui_command_skipped as u64;
            self.batch_ui_removed_nodes += present_timing.ui_removed_nodes as u64;
            self.batch_present_drain_commands += present_timing.drain_commands;
            self.batch_present_submit_commands += present_timing.submit_commands;
            self.batch_present_draw_frame += present_timing.gpu_present;
            self.batch_draw_process_commands += present_timing.draw_process_commands;
            self.batch_draw_prepare_cpu += present_timing.draw_prepare_cpu;
            self.batch_draw_gpu_prepare_2d += present_timing.draw_gpu_prepare_2d;
            self.batch_draw_gpu_prepare_3d += present_timing.draw_gpu_prepare_3d;
            self.batch_draw_gpu_prepare_particles_3d +=
                present_timing.draw_gpu_prepare_particles_3d;
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
            self.batch_draw_instances_3d += present_timing.draw_instances_3d as u64;
            self.batch_draw_material_refs_3d += present_timing.draw_material_refs_3d as u64;
            self.batch_render_command_count += present_timing.render_command_count as u64;
            self.batch_dirty_node_count += present_timing.dirty_node_count as u64;
            self.batch_active_meshes += present_timing.active_meshes as u64;
            self.batch_active_materials += present_timing.active_materials as u64;
            self.batch_active_textures += present_timing.active_textures as u64;
            self.batch_skip_prepare_2d += present_timing.skip_prepare_2d as u64;
            self.batch_skip_prepare_3d += present_timing.skip_prepare_3d as u64;
            self.batch_skip_prepare_particles_3d += present_timing.skip_prepare_particles_3d as u64;
            self.batch_skip_prepare_3d_frustum += present_timing.skip_prepare_3d_frustum as u64;
            self.batch_skip_prepare_3d_hiz += present_timing.skip_prepare_3d_hiz as u64;
            self.batch_skip_prepare_3d_indirect += present_timing.skip_prepare_3d_indirect as u64;
            self.batch_skip_prepare_3d_cull_inputs +=
                present_timing.skip_prepare_3d_cull_inputs as u64;
            self.batch_present_drain_events += present_timing.drain_events;
            self.batch_present_apply_events += present_timing.apply_events;
            self.batch_sim_delta_seconds += simulated_delta_seconds;
        }

        let batch_elapsed_secs = frame_end.duration_since(self.batch_start).as_secs_f32();
        if batch_elapsed_secs >= LOG_INTERVAL_SECONDS && self.batch_timing_samples > 0 {
            let avg_work_us = avg_micros(self.batch_work, self.batch_timing_samples);
            let avg_simulation_us = avg_micros(self.batch_simulation, self.batch_timing_samples);
            let avg_present_us = avg_micros(self.batch_present, self.batch_timing_samples);
            let avg_idle_before_frame_us =
                avg_micros(self.batch_idle_before_frame, self.batch_timing_samples);
            let avg_present_wait_us =
                avg_micros(self.batch_present_wait, self.batch_timing_samples);
            log_avg_sampled(
                avg_simulation_us,
                avg_present_us,
                avg_work_us,
                avg_idle_before_frame_us,
                avg_present_wait_us,
            );
            #[cfg(all(
                feature = "ui_profile",
                not(feature = "profile_heavy"),
                not(perro_no_console)
            ))]
            {
                let avg_present_extract_ui_us =
                    self.batch_present_extract_ui.as_micros() as f64 / self.batch_frames as f64;
                let avg_ui_layout_us =
                    self.batch_ui_layout.as_micros() as f64 / self.batch_frames as f64;
                let avg_ui_commands_us =
                    self.batch_ui_commands.as_micros() as f64 / self.batch_frames as f64;
                let avg_ui_dirty = self.batch_ui_dirty_nodes as f64 / self.batch_frames as f64;
                let avg_ui_affected =
                    self.batch_ui_affected_nodes as f64 / self.batch_frames as f64;
                let avg_ui_recalc =
                    self.batch_ui_recalculated_rects as f64 / self.batch_frames as f64;
                let avg_ui_cached = self.batch_ui_cached_rects as f64 / self.batch_frames as f64;
                let avg_ui_batches =
                    self.batch_ui_auto_layout_batches as f64 / self.batch_frames as f64;
                let avg_ui_cmd_nodes =
                    self.batch_ui_command_nodes as f64 / self.batch_frames as f64;
                let avg_ui_cmd_emit =
                    self.batch_ui_command_emitted as f64 / self.batch_frames as f64;
                let avg_ui_cmd_skip =
                    self.batch_ui_command_skipped as f64 / self.batch_frames as f64;
                let avg_ui_removed = self.batch_ui_removed_nodes as f64 / self.batch_frames as f64;
                println!(
                    "ui profile: total=({avg_present_extract_ui_us:.3}us) layout=({avg_ui_layout_us:.3}us) commands=({avg_ui_commands_us:.3}us) dirty=({avg_ui_dirty:.2}) affected=({avg_ui_affected:.2}) rect_recalc=({avg_ui_recalc:.2}) rect_cache=({avg_ui_cached:.2}) auto_batches=({avg_ui_batches:.2}) cmd_nodes=({avg_ui_cmd_nodes:.2}) cmd_emit=({avg_ui_cmd_emit:.2}) cmd_skip=({avg_ui_cmd_skip:.2}) rm=({avg_ui_removed:.2})"
                );
            }
            #[cfg(all(
                any(feature = "profile_heavy", feature = "mem_profile"),
                not(perro_no_console)
            ))]
            if self.mem_profile_enabled
                && let Some(sample) = process_memory_sample()
            {
                let avg_frame_us = avg_work_us
                    .saturating_add(avg_idle_before_frame_us)
                    .saturating_add(avg_present_wait_us);
                let avg_fps = if avg_frame_us > 0 {
                    1_000_000.0 / avg_frame_us as f64
                } else {
                    0.0
                };
                if let Some(csv) = &mut self.mem_profile_csv {
                    csv.write(MemProfileCsvSample {
                        batch_end_frame: self.frame_index,
                        sample,
                        avg_update_us: avg_simulation_us,
                        avg_render_us: avg_present_us,
                        avg_idle_us: avg_idle_before_frame_us,
                        avg_present_wait_us,
                        avg_frame_us,
                        avg_fps,
                    });
                }
            }
            #[cfg(all(feature = "profile_heavy", not(perro_no_console)))]
            {
                let avg_runtime_update_us =
                    self.batch_runtime_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_input_poll_us =
                    self.batch_input_poll.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_update_us =
                    self.batch_fixed_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_snapshot_update_us =
                    self.batch_fixed_snapshot_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_script_update_us =
                    self.batch_fixed_script_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_physics_update_us =
                    self.batch_fixed_physics_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_internal_update_us =
                    self.batch_fixed_internal_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_physics_pre_transforms_us =
                    self.batch_fixed_physics_pre_transforms.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_fixed_physics_collect_us =
                    self.batch_fixed_physics_collect.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_physics_sync_world_us =
                    self.batch_fixed_physics_sync_world.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_fixed_physics_apply_forces_impulses_us =
                    self.batch_fixed_physics_apply_forces_impulses.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_fixed_physics_step_us =
                    self.batch_fixed_physics_step.as_micros() as f64 / self.batch_frames as f64;
                let avg_fixed_physics_sync_nodes_us =
                    self.batch_fixed_physics_sync_nodes.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_fixed_physics_post_transforms_us =
                    self.batch_fixed_physics_post_transforms.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_fixed_physics_signals_us =
                    self.batch_fixed_physics_signals.as_micros() as f64 / self.batch_frames as f64;
                let avg_runtime_script_update_us =
                    self.batch_runtime_script_update.as_micros() as f64 / self.batch_frames as f64;
                let avg_runtime_script_count =
                    self.batch_runtime_script_count as f64 / self.batch_frames as f64;

                let avg_present_extract_2d_us =
                    self.batch_present_extract_2d.as_micros() as f64 / self.batch_frames as f64;
                let avg_present_extract_3d_us =
                    self.batch_present_extract_3d.as_micros() as f64 / self.batch_frames as f64;
                let avg_present_extract_ui_us =
                    self.batch_present_extract_ui.as_micros() as f64 / self.batch_frames as f64;
                let avg_ui_layout_us =
                    self.batch_ui_layout.as_micros() as f64 / self.batch_frames as f64;
                let avg_ui_commands_us =
                    self.batch_ui_commands.as_micros() as f64 / self.batch_frames as f64;
                let avg_ui_dirty = self.batch_ui_dirty_nodes as f64 / self.batch_frames as f64;
                let avg_ui_affected =
                    self.batch_ui_affected_nodes as f64 / self.batch_frames as f64;
                let avg_ui_recalc =
                    self.batch_ui_recalculated_rects as f64 / self.batch_frames as f64;
                let avg_ui_cached = self.batch_ui_cached_rects as f64 / self.batch_frames as f64;
                let avg_ui_batches =
                    self.batch_ui_auto_layout_batches as f64 / self.batch_frames as f64;
                let avg_ui_cmd_nodes =
                    self.batch_ui_command_nodes as f64 / self.batch_frames as f64;
                let avg_ui_cmd_emit =
                    self.batch_ui_command_emitted as f64 / self.batch_frames as f64;
                let avg_ui_cmd_skip =
                    self.batch_ui_command_skipped as f64 / self.batch_frames as f64;
                let avg_ui_removed = self.batch_ui_removed_nodes as f64 / self.batch_frames as f64;
                let avg_present_drain_commands_us =
                    self.batch_present_drain_commands.as_micros() as f64 / self.batch_frames as f64;
                let avg_present_submit_commands_us = self.batch_present_submit_commands.as_micros()
                    as f64
                    / self.batch_frames as f64;
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
                let avg_draw_gpu_prepare_3d_hiz_us = self.batch_draw_gpu_prepare_3d_hiz.as_micros()
                    as f64
                    / self.batch_frames as f64;
                let avg_draw_gpu_prepare_3d_indirect_us =
                    self.batch_draw_gpu_prepare_3d_indirect.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_draw_gpu_prepare_3d_cull_inputs_us =
                    self.batch_draw_gpu_prepare_3d_cull_inputs.as_micros() as f64
                        / self.batch_frames as f64;
                let avg_draw_gpu_acquire_us =
                    self.batch_draw_gpu_acquire.as_micros() as f64 / self.batch_frames as f64;
                let avg_draw_gpu_acquire_surface_us =
                    self.batch_draw_gpu_acquire_surface.as_micros() as f64
                        / self.batch_frames as f64;
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
                    self.batch_draw_gpu_submit_queue_main.as_micros() as f64
                        / self.batch_frames as f64;
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
                let avg_draw_instances_3d =
                    self.batch_draw_instances_3d as f64 / self.batch_frames as f64;
                let avg_instances_per_draw_3d = if self.batch_draw_calls_3d > 0 {
                    self.batch_draw_instances_3d as f64 / self.batch_draw_calls_3d as f64
                } else {
                    0.0
                };
                let avg_draw_material_refs_3d =
                    self.batch_draw_material_refs_3d as f64 / self.batch_frames as f64;
                let avg_render_commands =
                    self.batch_render_command_count as f64 / self.batch_frames as f64;
                let avg_dirty_nodes = self.batch_dirty_node_count as f64 / self.batch_frames as f64;
                let avg_active_meshes = self.batch_active_meshes as f64 / self.batch_frames as f64;
                let avg_active_materials =
                    self.batch_active_materials as f64 / self.batch_frames as f64;
                let avg_active_textures =
                    self.batch_active_textures as f64 / self.batch_frames as f64;
                let avg_present_drain_events_us =
                    self.batch_present_drain_events.as_micros() as f64 / self.batch_frames as f64;
                let avg_present_apply_events_us =
                    self.batch_present_apply_events.as_micros() as f64 / self.batch_frames as f64;
                let avg_frame_us = (self.batch_work.as_micros() as f64
                    + self.batch_idle_before_frame.as_micros() as f64
                    + self.batch_present_wait.as_micros() as f64)
                    / self.batch_frames as f64;
                let pct_skip_prepare_2d =
                    (self.batch_skip_prepare_2d as f64 * 100.0) / self.batch_frames as f64;
                let pct_skip_prepare_3d =
                    (self.batch_skip_prepare_3d as f64 * 100.0) / self.batch_frames as f64;
                let pct_skip_prepare_particles_3d = (self.batch_skip_prepare_particles_3d as f64
                    * 100.0)
                    / self.batch_frames as f64;
                let pct_skip_prepare_3d_frustum =
                    (self.batch_skip_prepare_3d_frustum as f64 * 100.0) / self.batch_frames as f64;
                let pct_skip_prepare_3d_hiz =
                    (self.batch_skip_prepare_3d_hiz as f64 * 100.0) / self.batch_frames as f64;
                let pct_skip_prepare_3d_indirect =
                    (self.batch_skip_prepare_3d_indirect as f64 * 100.0) / self.batch_frames as f64;
                let pct_skip_prepare_3d_cull_inputs =
                    (self.batch_skip_prepare_3d_cull_inputs as f64 * 100.0)
                        / self.batch_frames as f64;
                println!(
                    "simulation breakdown: input=({:.3}us) fixed=({:.3}us) runtime=({:.3}us)",
                    avg_input_poll_us, avg_fixed_update_us, avg_runtime_update_us
                );
                println!(
                    "fixed breakdown: snapshot=({:.3}us) scripts=({:.3}us) physics=({:.3}us) internal=({:.3}us)",
                    avg_fixed_snapshot_update_us,
                    avg_fixed_script_update_us,
                    avg_fixed_physics_update_us,
                    avg_fixed_internal_update_us
                );
                println!(
                    "physics breakdown: pre_xform=({:.3}us) collect=({:.3}us) sync_world=({:.3}us) apply=({:.3}us) step=({:.3}us) sync_nodes=({:.3}us) post_xform=({:.3}us) signals=({:.3}us)",
                    avg_fixed_physics_pre_transforms_us,
                    avg_fixed_physics_collect_us,
                    avg_fixed_physics_sync_world_us,
                    avg_fixed_physics_apply_forces_impulses_us,
                    avg_fixed_physics_step_us,
                    avg_fixed_physics_sync_nodes_us,
                    avg_fixed_physics_post_transforms_us,
                    avg_fixed_physics_signals_us
                );
                println!(
                    "user scripts: ({:.3}us avg) | script calls/frame: ({:.2}) | slowest script: ({:.3}us)",
                    avg_runtime_script_update_us,
                    avg_runtime_script_count,
                    self.batch_runtime_slowest_script.as_micros() as f64
                );
                println!(
                    "present breakdown: extract2d=({:.3}us) extract3d=({:.3}us) extract_ui=({:.3}us) ui_layout=({:.3}us) ui_commands=({:.3}us) drain=({:.3}us) submit=({:.3}us) draw=({:.3}us) events_drain=({:.3}us) events_apply=({:.3}us)",
                    avg_present_extract_2d_us,
                    avg_present_extract_3d_us,
                    avg_present_extract_ui_us,
                    avg_ui_layout_us,
                    avg_ui_commands_us,
                    avg_present_drain_commands_us,
                    avg_present_submit_commands_us,
                    avg_present_draw_frame_us,
                    avg_present_drain_events_us,
                    avg_present_apply_events_us
                );
                println!(
                    "ui nodes: dirty=({avg_ui_dirty:.2}) affected=({avg_ui_affected:.2}) rect_recalc=({avg_ui_recalc:.2}) rect_cache=({avg_ui_cached:.2}) auto_batches=({avg_ui_batches:.2}) cmd_nodes=({avg_ui_cmd_nodes:.2}) cmd_emit=({avg_ui_cmd_emit:.2}) cmd_skip=({avg_ui_cmd_skip:.2}) rm=({avg_ui_removed:.2})"
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
                if let Some(csv) = &mut self.profile_csv {
                    let row = ProfileCsvRow {
                        batch_end_frame: self.frame_index,
                        frames: self.batch_frames,
                        sampled_frames: self.batch_timing_samples,
                        avg_draw_calls_2d,
                        avg_draw_calls_3d,
                        avg_draw_calls_total,
                        avg_draw_instances_3d,
                        avg_instances_per_draw_3d,
                        avg_draw_material_refs_3d,
                        avg_render_commands,
                        avg_dirty_nodes,
                        avg_extract2d_us: avg_present_extract_2d_us,
                        avg_extract3d_us: avg_present_extract_3d_us,
                        avg_extract_ui_us: avg_present_extract_ui_us,
                        avg_drain_commands_us: avg_present_drain_commands_us,
                        avg_submit_commands_us: avg_present_submit_commands_us,
                        avg_draw_process_us: avg_draw_process_commands_us,
                        avg_draw_prep_us: avg_draw_prepare_cpu_us,
                        avg_active_meshes,
                        avg_active_materials,
                        avg_active_textures,
                        avg_present_wait_us: self.batch_present_wait.as_micros() as f64
                            / self.batch_frames as f64,
                        avg_frame_us,
                    };
                    csv.write(&row);
                }
            }
            self.batch_frames = 0;
            self.batch_timing_samples = 0;
            self.batch_work = Duration::ZERO;
            self.batch_simulation = Duration::ZERO;
            self.batch_present = Duration::ZERO;
            self.batch_idle_before_frame = Duration::ZERO;
            self.batch_present_wait = Duration::ZERO;
            self.batch_idle = Duration::ZERO;
            #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
            {
                self.batch_present_extract_ui = Duration::ZERO;
                self.batch_ui_layout = Duration::ZERO;
                self.batch_ui_commands = Duration::ZERO;
                self.batch_ui_dirty_nodes = 0;
                self.batch_ui_affected_nodes = 0;
                self.batch_ui_recalculated_rects = 0;
                self.batch_ui_cached_rects = 0;
                self.batch_ui_auto_layout_batches = 0;
                self.batch_ui_command_nodes = 0;
                self.batch_ui_command_emitted = 0;
                self.batch_ui_command_skipped = 0;
                self.batch_ui_removed_nodes = 0;
            }
            #[cfg(feature = "profile_heavy")]
            {
                self.batch_runtime_update = Duration::ZERO;
                self.batch_input_poll = Duration::ZERO;
                self.batch_fixed_update = Duration::ZERO;
                self.batch_fixed_snapshot_update = Duration::ZERO;
                self.batch_fixed_script_update = Duration::ZERO;
                self.batch_fixed_physics_update = Duration::ZERO;
                self.batch_fixed_internal_update = Duration::ZERO;
                self.batch_fixed_physics_pre_transforms = Duration::ZERO;
                self.batch_fixed_physics_collect = Duration::ZERO;
                self.batch_fixed_physics_sync_world = Duration::ZERO;
                self.batch_fixed_physics_apply_forces_impulses = Duration::ZERO;
                self.batch_fixed_physics_step = Duration::ZERO;
                self.batch_fixed_physics_sync_nodes = Duration::ZERO;
                self.batch_fixed_physics_post_transforms = Duration::ZERO;
                self.batch_fixed_physics_signals = Duration::ZERO;
                self.batch_runtime_start_schedule = Duration::ZERO;
                self.batch_runtime_snapshot_update = Duration::ZERO;
                self.batch_runtime_script_update = Duration::ZERO;
                self.batch_runtime_internal_update = Duration::ZERO;
                self.batch_runtime_slowest_script = Duration::ZERO;
                self.batch_runtime_script_count = 0;
                self.batch_present_extract_2d = Duration::ZERO;
                self.batch_present_extract_3d = Duration::ZERO;
                self.batch_present_extract_ui = Duration::ZERO;
                self.batch_ui_layout = Duration::ZERO;
                self.batch_ui_commands = Duration::ZERO;
                self.batch_ui_dirty_nodes = 0;
                self.batch_ui_affected_nodes = 0;
                self.batch_ui_recalculated_rects = 0;
                self.batch_ui_cached_rects = 0;
                self.batch_ui_auto_layout_batches = 0;
                self.batch_ui_command_nodes = 0;
                self.batch_ui_command_emitted = 0;
                self.batch_ui_command_skipped = 0;
                self.batch_ui_removed_nodes = 0;
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
                self.batch_draw_instances_3d = 0;
                self.batch_draw_material_refs_3d = 0;
                self.batch_render_command_count = 0;
                self.batch_dirty_node_count = 0;
                self.batch_active_meshes = 0;
                self.batch_active_materials = 0;
                self.batch_active_textures = 0;
                self.batch_skip_prepare_2d = 0;
                self.batch_skip_prepare_3d = 0;
                self.batch_skip_prepare_particles_3d = 0;
                self.batch_skip_prepare_3d_frustum = 0;
                self.batch_skip_prepare_3d_hiz = 0;
                self.batch_skip_prepare_3d_indirect = 0;
                self.batch_skip_prepare_3d_cull_inputs = 0;
                self.batch_present_drain_events = Duration::ZERO;
                self.batch_present_apply_events = Duration::ZERO;
                self.batch_sim_delta_seconds = 0.0;
            }
            self.batch_start = frame_end;
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
            let attrs = window_attributes(
                event_loop,
                self.app.runtime.project(),
                &self.title,
                #[cfg(not(target_arch = "wasm32"))]
                self.preloaded_images.window_icon.take(),
            );
            let window = Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("failed to create winit window"),
            );
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.last_window_position = window.outer_position().ok();
            }
            #[cfg(target_arch = "wasm32")]
            sync_web_window_size(window.as_ref());
            window.set_ime_allowed(true);
            self.app.attach_window(window.clone());
            let initial_size = window.inner_size();
            self.app
                .resize_surface(initial_size.width, initial_size.height);
            // Show before the first draw so a slow GPU/surface path cannot look like a launch hang.
            window.set_visible(true);
            if self.startup_splash.active {
                let splash_overlay = self.startup_splash_overlay_commands(1.0);
                let _ = self.app.present_with_overlay_timed_no_ui(splash_overlay);
            } else {
                self.app.present();
            }
            self.window = Some(window);
            self.set_mouse_mode(MouseMode::Visible);
            self.app
                .runtime
                .set_active_refresh_rate(active_refresh_rate_hz(self.window.as_deref()));
            eprintln!(
                "[perro][runtime] active_refresh_rate=({:?})",
                active_refresh_rate_hz(self.window.as_deref())
            );
            let now = Instant::now();
            self.last_frame_start = now;
            self.last_frame_end = now;
            self.run_start = now;
            if self.startup_splash.active {
                self.startup_splash.shown_at = now;
                self.startup_splash.ready_streak = 0;
                self.startup_splash.fade_started_at = None;
                self.startup_splash.first_frame_inflight.clear();
                self.startup_splash.first_frame_captured = false;
            }
            self.fixed_accumulator = 0.0;
            self.next_frame_deadline = None;
            self.frame_index = 0;
            self.batch_start = now;
            self.batch_frames = 0;
            self.batch_timing_samples = 0;
            self.timing_warmup_frames_left = TIMING_WARMUP_FRAMES;
            self.batch_work = Duration::ZERO;
            self.batch_simulation = Duration::ZERO;
            self.batch_present = Duration::ZERO;
            self.batch_idle_before_frame = Duration::ZERO;
            self.batch_present_wait = Duration::ZERO;
            self.batch_idle = Duration::ZERO;
            #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
            {
                self.batch_present_extract_ui = Duration::ZERO;
                self.batch_ui_layout = Duration::ZERO;
                self.batch_ui_commands = Duration::ZERO;
                self.batch_ui_dirty_nodes = 0;
                self.batch_ui_affected_nodes = 0;
                self.batch_ui_recalculated_rects = 0;
                self.batch_ui_cached_rects = 0;
                self.batch_ui_auto_layout_batches = 0;
                self.batch_ui_command_nodes = 0;
                self.batch_ui_command_emitted = 0;
                self.batch_ui_command_skipped = 0;
                self.batch_ui_removed_nodes = 0;
            }
            #[cfg(feature = "profile_heavy")]
            {
                self.batch_runtime_update = Duration::ZERO;
                self.batch_input_poll = Duration::ZERO;
                self.batch_fixed_update = Duration::ZERO;
                self.batch_fixed_snapshot_update = Duration::ZERO;
                self.batch_fixed_script_update = Duration::ZERO;
                self.batch_fixed_physics_update = Duration::ZERO;
                self.batch_fixed_internal_update = Duration::ZERO;
                self.batch_fixed_physics_pre_transforms = Duration::ZERO;
                self.batch_fixed_physics_collect = Duration::ZERO;
                self.batch_fixed_physics_sync_world = Duration::ZERO;
                self.batch_fixed_physics_apply_forces_impulses = Duration::ZERO;
                self.batch_fixed_physics_step = Duration::ZERO;
                self.batch_fixed_physics_sync_nodes = Duration::ZERO;
                self.batch_fixed_physics_post_transforms = Duration::ZERO;
                self.batch_fixed_physics_signals = Duration::ZERO;
                self.batch_runtime_start_schedule = Duration::ZERO;
                self.batch_runtime_snapshot_update = Duration::ZERO;
                self.batch_runtime_script_update = Duration::ZERO;
                self.batch_runtime_internal_update = Duration::ZERO;
                self.batch_runtime_slowest_script = Duration::ZERO;
                self.batch_runtime_script_count = 0;
                self.batch_present_extract_2d = Duration::ZERO;
                self.batch_present_extract_3d = Duration::ZERO;
                self.batch_present_extract_ui = Duration::ZERO;
                self.batch_ui_layout = Duration::ZERO;
                self.batch_ui_commands = Duration::ZERO;
                self.batch_ui_dirty_nodes = 0;
                self.batch_ui_affected_nodes = 0;
                self.batch_ui_recalculated_rects = 0;
                self.batch_ui_cached_rects = 0;
                self.batch_ui_auto_layout_batches = 0;
                self.batch_ui_command_nodes = 0;
                self.batch_ui_command_emitted = 0;
                self.batch_ui_command_skipped = 0;
                self.batch_ui_removed_nodes = 0;
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
                self.batch_draw_instances_3d = 0;
                self.batch_draw_material_refs_3d = 0;
                self.batch_render_command_count = 0;
                self.batch_dirty_node_count = 0;
                self.batch_active_meshes = 0;
                self.batch_active_materials = 0;
                self.batch_active_textures = 0;
                self.batch_skip_prepare_2d = 0;
                self.batch_skip_prepare_3d = 0;
                self.batch_skip_prepare_particles_3d = 0;
                self.batch_skip_prepare_3d_frustum = 0;
                self.batch_skip_prepare_3d_hiz = 0;
                self.batch_skip_prepare_3d_indirect = 0;
                self.batch_skip_prepare_3d_cull_inputs = 0;
                self.batch_present_drain_events = Duration::ZERO;
                self.batch_present_apply_events = Duration::ZERO;
                self.batch_sim_delta_seconds = 0.0;
            }
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
                #[cfg(target_arch = "wasm32")]
                if let Some(window) = &self.window {
                    sync_web_window_size(window.as_ref());
                }
                self.app.resize_surface(size.width, size.height);
            }
            WindowEvent::Moved(position) => {
                self.sync_window_position(position);
                // On Windows title-bar drag can suppress redraw cadence; tick on move events too.
                self.step_frame(event_loop, Instant::now());
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
                    self.set_mouse_mode(MouseMode::Visible);
                }
                self.kbm_input
                    .handle_window_event(&mut self.app, keyboard_event);
            }
            mouse_event @ WindowEvent::MouseInput { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
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
                if matches!(
                    self.mouse_mode,
                    MouseMode::Captured | MouseMode::Confined | MouseMode::ConfinedHidden
                ) {
                    self.clear_keyboard_mouse_focus_state();
                    self.set_mouse_mode(MouseMode::Visible);
                }
            }
            cursor_moved @ WindowEvent::CursorMoved { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.cursor_inside_window = true;
                if self.mouse_mode == MouseMode::Captured && self.mouse_uses_raw_motion {
                    self.kbm_input.reset_cursor_position();
                } else if (self.mouse_mode == MouseMode::Captured && !self.mouse_uses_raw_motion)
                    || self.mouse_mode == MouseMode::ConfinedHidden
                {
                    if let Some(window) = &self.window
                        && let WindowEvent::CursorMoved { position, .. } = cursor_moved
                    {
                        let center = window_center(window.as_ref());
                        let dx = (position.x - center.x) as f32;
                        let dy = (center.y - position.y) as f32;
                        if dx.abs() > 0.001 || dy.abs() > 0.001 {
                            self.app.add_mouse_delta(dx, dy);
                            self.app
                                .set_mouse_position(position.x as f32, position.y as f32);
                            center_cursor(window.as_ref());
                        }
                    }
                } else {
                    self.kbm_input
                        .handle_window_event(&mut self.app, &cursor_moved);
                }
            }
            WindowEvent::MouseWheel { .. } => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.kbm_input.handle_window_event(&mut self.app, &event);
            }
            ime_event @ WindowEvent::Ime(_) => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.kbm_input
                    .handle_window_event(&mut self.app, &ime_event);
            }
            WindowEvent::Focused(true) => {
                if self.startup_splash.blocks_input() {
                    return;
                }
                self.apply_mouse_mode_request();
            }
            WindowEvent::Focused(false) => {
                self.clear_keyboard_mouse_focus_state();
                self.set_mouse_mode(MouseMode::Visible);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    #[cfg(target_arch = "wasm32")]
                    sync_web_window_size(window.as_ref());
                    let size = window.inner_size();
                    self.app.resize_surface(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.step_frame(event_loop, Instant::now());
            }
            WindowEvent::CloseRequested => {
                self.exit_result = Some(AppExitResult::window_close());
                self.reset_mouse_mode_for_exit();
                if let Some(window) = self.window.take() {
                    window.set_visible(false);
                }
                event_loop.exit();
            }
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
        if self.mouse_mode == MouseMode::Captured
            && self.mouse_uses_raw_motion
            && let DeviceEvent::MouseMotion { delta } = event
        {
            self.kbm_input
                .handle_mouse_motion(&mut self.app, delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() || self.exit_result.is_some() {
            return;
        }
        let now = Instant::now();
        self.apply_frame_control_flow(event_loop, now);
        if self.window.is_some()
            && (self
                .next_frame_deadline
                .is_none_or(|deadline| deadline <= now)
                || self.startup_splash.active)
        {
            self.step_frame(event_loop, now);
        }
    }
}

impl<B: GraphicsBackend> Drop for RunnerState<B> {
    fn drop(&mut self) {
        self.reset_mouse_mode_for_exit();
    }
}

fn window_attributes(
    event_loop: &ActiveEventLoop,
    project: Option<&perro_runtime::RuntimeProject>,
    fallback_title: &str,
    #[cfg(not(target_arch = "wasm32"))] window_icon: Option<winit::window::Icon>,
) -> WindowAttributes {
    #[cfg(target_arch = "wasm32")]
    let _ = event_loop;

    let title = project
        .map(|project| project.config.name.as_str())
        .unwrap_or(fallback_title)
        .to_string();

    let mut attrs = WindowAttributes::default()
        .with_title(title)
        .with_visible(false);
    #[cfg(target_arch = "wasm32")]
    {
        attrs = attrs.with_append(true);
    }
    let Some(project) = project else {
        return attrs;
    };

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(icon) = window_icon {
        attrs = attrs.with_window_icon(Some(icon));
    }

    let desired = PhysicalSize::new(project.config.virtual_width, project.config.virtual_height);
    if desired.width == 0 || desired.height == 0 {
        return attrs;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let viewport = browser_viewport_size().unwrap_or(desired);
        attrs.with_inner_size(Size::Physical(viewport))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(monitor) = pick_monitor(event_loop) else {
            return attrs.with_inner_size(Size::Physical(desired));
        };
        if let Some(mode) = parse_window_mode_override()
            && (mode == "borderless" || mode == "borderless_fullscreen")
        {
            return attrs.with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
        }

        let max_width =
            ((monitor.size().width as f32) * INITIAL_WINDOW_MONITOR_FRACTION).floor() as u32;
        let max_height =
            ((monitor.size().height as f32) * INITIAL_WINDOW_MONITOR_FRACTION).floor() as u32;
        let fitted = fit_aspect(desired, max_width.max(1), max_height.max(1));
        let centered = center_position(&monitor, fitted);

        attrs = attrs.with_inner_size(Size::Physical(fitted));
        attrs.with_position(Position::Physical(centered))
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_viewport_size() -> Option<PhysicalSize<u32>> {
    let window = web_sys::window()?;
    let width = window.inner_width().ok()?.as_f64()?;
    let height = window.inner_height().ok()?.as_f64()?;
    let width = width.round().max(1.0) as u32;
    let height = height.round().max(1.0) as u32;
    Some(PhysicalSize::new(width, height))
}

#[cfg(target_arch = "wasm32")]
fn sync_web_window_size(window: &Window) {
    if let Some(viewport) = browser_viewport_size() {
        let _ = window.request_inner_size(viewport);
    }
    if let Some(canvas) = window.canvas() {
        let _ = canvas.set_attribute(
            "style",
            "display:block;width:100vw;height:100vh;outline:none;position:fixed;inset:0;",
        );
    }
}

fn window_center(window: &Window) -> PhysicalPosition<f64> {
    let size = window.inner_size();
    PhysicalPosition::new(size.width as f64 * 0.5, size.height as f64 * 0.5)
}

fn center_cursor(window: &Window) {
    let _ = window.set_cursor_position(window_center(window));
}

fn release_mouse(window: &Window) {
    let _ = window.set_cursor_grab(CursorGrabMode::None);
    window.set_cursor_visible(true);
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_window_mode_override() -> Option<String> {
    std::env::var("PERRO_WINDOW_MODE")
        .ok()
        .map(|raw| raw.trim().to_ascii_lowercase())
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_monitor(event_loop: &ActiveEventLoop) -> Option<MonitorHandle> {
    event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
}

#[cfg(target_arch = "wasm32")]
fn pick_monitor(_event_loop: &ActiveEventLoop) -> Option<MonitorHandle> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn fit_aspect(
    desired: PhysicalSize<u32>,
    max_width: u32,
    max_height: u32,
) -> PhysicalSize<u32> {
    let scale = f32::min(
        max_width as f32 / desired.width as f32,
        max_height as f32 / desired.height as f32,
    );
    let width = ((desired.width as f32) * scale).floor().max(1.0) as u32;
    let height = ((desired.height as f32) * scale).floor().max(1.0) as u32;
    PhysicalSize::new(width, height)
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(test)]
mod fixed_step_tests;
