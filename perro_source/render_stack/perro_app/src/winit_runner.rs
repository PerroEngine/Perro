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
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use std::{fs, sync::Arc};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::EventLoopProxy;
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

use crate::frame_pacing::{FRAME_WAKE_HEADROOM, FramePacer, project_frame_rate_cap};

const DEFAULT_FIXED_TIMESTEP: Option<f32> = None;
const MAX_FIXED_STEPS_PER_FRAME: u32 = 2;
const MAX_FRAME_DELTA_SECONDS: f32 = 0.250;
const LOG_INTERVAL_SECONDS: f32 = 3.0;
#[cfg(not(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps")))]
const LOG_TIMING_SAMPLE_STRIDE: u32 = 20;
const TIMING_WARMUP_FRAMES: u32 = 8;
// Reported fps averages real frame counts over this window; a single-frame
// reciprocal is too noisy to represent perceived smoothness.
const FPS_WINDOW_SECONDS: f32 = 0.5;
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
    timestamp_ms: u128,
}

struct TimingCsvWriter {
    file: std::io::BufWriter<fs::File>,
}

impl TimingCsvWriter {
    fn from_env() -> Option<Self> {
        let path = std::env::var("PERRO_TIMING_CSV").ok()?;
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let mut file = std::io::BufWriter::with_capacity(256 * 1024, file);
        let _ = writeln!(
            file,
            "frame,phase,warmup,sampled,frame_delta_us,idle_before_frame_us,simulation_us,render_active_us,work_active_us,present_wait_us,fixed_steps,fixed_step_us,fixed_accum_before_us,fixed_accum_after_us,fixed_catchup_dropped,timestamp_ms"
        );
        Some(Self { file })
    }

    fn write(&mut self, sample: CsvFrameSample) {
        let _ = writeln!(
            self.file,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
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
            if sample.fixed_catchup_dropped { 1 } else { 0 },
            sample.timestamp_ms
        );
    }
}

#[inline]
fn unix_timestamp_ms() -> u128 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    }
    #[cfg(target_arch = "wasm32")]
    {
        0
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
    avg_sprite_batches_2d: f64,
    avg_sprite_bind_group_switches_2d: f64,
    avg_draw_batches_3d: f64,
    avg_pipeline_switches_3d: f64,
    avg_texture_bind_group_switches_3d: f64,
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
            "batch_end_frame,frames,sampled_frames,avg_draw_calls_2d,avg_draw_calls_3d,avg_draw_calls_total,avg_sprite_batches_2d,avg_sprite_bind_group_switches_2d,avg_draw_batches_3d,avg_pipeline_switches_3d,avg_texture_bind_group_switches_3d,avg_draw_instances_3d,avg_instances_per_draw_3d,avg_draw_material_refs_3d,avg_render_commands,avg_dirty_nodes,avg_extract2d_us,avg_extract3d_us,avg_extract_ui_us,avg_drain_commands_us,avg_submit_commands_us,avg_draw_process_us,avg_draw_prep_us,avg_active_meshes,avg_active_materials,avg_active_textures,avg_present_wait_us,avg_frame_us"
        );
        Some(Self { file })
    }

    fn write(&mut self, row: &ProfileCsvRow) {
        let _ = writeln!(
            self.file,
            "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            row.batch_end_frame,
            row.frames,
            row.sampled_frames,
            row.avg_draw_calls_2d,
            row.avg_draw_calls_3d,
            row.avg_draw_calls_total,
            row.avg_sprite_batches_2d,
            row.avg_sprite_bind_group_switches_2d,
            row.avg_draw_batches_3d,
            row.avg_pipeline_switches_3d,
            row.avg_texture_bind_group_switches_3d,
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunnerUserEvent {
    RequestExit,
}

#[cfg(target_arch = "wasm32")]
type RunnerUserEvent = ();

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
        let event_loop = EventLoop::<RunnerUserEvent>::with_user_event()
            .build()
            .map_err(|err| AppExitError {
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
            let _timer_resolution = crate::timer_resolution::TimerResolutionGuard::acquire_1ms();
            install_ctrl_c_exit_proxy(event_loop.create_proxy());
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
        let mut builder = EventLoop::<RunnerUserEvent>::with_user_event();
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

#[cfg(not(target_arch = "wasm32"))]
fn ctrl_c_proxy_slot() -> &'static Mutex<Option<EventLoopProxy<RunnerUserEvent>>> {
    static SLOT: OnceLock<Mutex<Option<EventLoopProxy<RunnerUserEvent>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

#[cfg(not(target_arch = "wasm32"))]
fn install_ctrl_c_exit_proxy(proxy: EventLoopProxy<RunnerUserEvent>) {
    if let Ok(mut slot) = ctrl_c_proxy_slot().lock() {
        *slot = Some(proxy);
    }

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    if let Err(err) = ctrlc::set_handler(|| {
        let proxy = ctrl_c_proxy_slot()
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().cloned());
        if let Some(proxy) = proxy {
            let _ = proxy.send_event(RunnerUserEvent::RequestExit);
        }
    }) {
        eprintln!("[perro][runtime] failed to install ctrl-c handler: {err}");
    }
}

/// Always-on per-batch frame accounting. Reset = assign `Default::default()`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BatchCoreStats {
    pub(crate) frames: u32,
    pub(crate) timing_samples: u32,
    pub(crate) work: Duration,
    pub(crate) simulation: Duration,
    pub(crate) present: Duration,
    pub(crate) idle_before_frame: Duration,
    pub(crate) present_wait: Duration,
    pub(crate) idle: Duration,
}

/// UI pipeline batch counters (`ui_profile` or `profile_heavy`).
#[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
#[derive(Clone, Copy, Debug, Default)]
struct BatchUiStats {
    extract_ui: Duration,
    layout: Duration,
    commands: Duration,
    dirty_nodes: u64,
    affected_nodes: u64,
    recalculated_rects: u64,
    cached_rects: u64,
    auto_layout_batches: u64,
    command_nodes: u64,
    command_emitted: u64,
    command_skipped: u64,
    removed_nodes: u64,
}

/// Deep profiling batch counters (`profile_heavy` only).
#[cfg(feature = "profile_heavy")]
#[derive(Clone, Copy, Debug, Default)]
struct BatchHeavyStats {
    runtime_update: Duration,
    input_poll: Duration,
    fixed_update: Duration,
    fixed_snapshot_update: Duration,
    fixed_script_update: Duration,
    fixed_physics_update: Duration,
    fixed_internal_update: Duration,
    fixed_physics_pre_transforms: Duration,
    fixed_physics_collect: Duration,
    fixed_physics_sync_world: Duration,
    fixed_physics_apply_forces_impulses: Duration,
    fixed_physics_step: Duration,
    fixed_physics_sync_nodes: Duration,
    fixed_physics_post_transforms: Duration,
    fixed_physics_signals: Duration,
    runtime_start_schedule: Duration,
    runtime_snapshot_update: Duration,
    runtime_script_update: Duration,
    runtime_internal_update: Duration,
    runtime_slowest_script: Duration,
    runtime_script_count: u64,
    present_extract_2d: Duration,
    present_extract_3d: Duration,
    present_drain_commands: Duration,
    present_submit_commands: Duration,
    present_draw_frame: Duration,
    draw_process_commands: Duration,
    draw_prepare_cpu: Duration,
    draw_gpu_prepare_2d: Duration,
    draw_gpu_prepare_3d: Duration,
    draw_gpu_prepare_particles_3d: Duration,
    draw_gpu_prepare_3d_frustum: Duration,
    draw_gpu_prepare_3d_hiz: Duration,
    draw_gpu_prepare_3d_indirect: Duration,
    draw_gpu_prepare_3d_cull_inputs: Duration,
    draw_gpu_acquire: Duration,
    draw_gpu_acquire_surface: Duration,
    draw_gpu_acquire_view: Duration,
    draw_gpu_encode_main: Duration,
    draw_gpu_submit_main: Duration,
    draw_gpu_submit_finish_main: Duration,
    draw_gpu_submit_queue_main: Duration,
    draw_gpu_post_process: Duration,
    draw_gpu_accessibility: Duration,
    draw_gpu_present: Duration,
    draw_calls_2d: u64,
    draw_calls_3d: u64,
    draw_calls_total: u64,
    sprite_batches_2d: u64,
    sprite_bind_group_switches_2d: u64,
    draw_batches_3d: u64,
    pipeline_switches_3d: u64,
    texture_bind_group_switches_3d: u64,
    draw_instances_3d: u64,
    draw_material_refs_3d: u64,
    skip_prepare_2d: u64,
    skip_prepare_3d: u64,
    skip_prepare_particles_3d: u64,
    skip_prepare_3d_frustum: u64,
    skip_prepare_3d_hiz: u64,
    skip_prepare_3d_indirect: u64,
    skip_prepare_3d_cull_inputs: u64,
    present_drain_events: Duration,
    present_apply_events: Duration,
    sim_delta_seconds: f64,
    render_command_count: u64,
    dirty_node_count: u64,
    active_meshes: u64,
    active_materials: u64,
    active_textures: u64,
}

struct RunnerState<B: GraphicsBackend> {
    app: App<B>,
    title: String,
    window: Option<Arc<Window>>,
    last_frame_start: Instant,
    last_frame_end: Instant,
    run_start: Instant,
    batch_start: Instant,
    batch: BatchCoreStats,
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile"))]
    batch_ui: BatchUiStats,
    #[cfg(feature = "profile_heavy")]
    batch_heavy: BatchHeavyStats,
    fixed_timestep: Option<f32>,
    fixed_accumulator: f32,
    pacer: FramePacer,
    frame_index: u64,
    fps_window_start: Instant,
    fps_window_frames: u32,
    timing_csv: Option<TimingCsvWriter>,
    #[cfg(feature = "profile_heavy")]
    profile_csv: Option<ProfileCsvWriter>,
    #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
    mem_profile_enabled: bool,
    #[cfg(any(feature = "profile_heavy", feature = "mem_profile"))]
    mem_profile_csv: Option<MemProfileCsvWriter>,
    timing_warmup_frames_left: u32,
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

mod frame;
mod splash;
mod state;

impl<B: GraphicsBackend> winit::application::ApplicationHandler<RunnerUserEvent>
    for RunnerState<B>
{
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
            // Web GPU init async; kp prior show point.
            #[cfg(target_arch = "wasm32")]
            window.set_visible(true);
            if self.startup_splash.active {
                let splash_overlay = self.startup_splash_overlay_commands(1.0);
                let _ = self.app.present_with_overlay_timed_no_ui(splash_overlay);
            } else {
                self.app.present();
            }
            // Show native win only aft first GPU present -> no blank white flash.
            #[cfg(not(target_arch = "wasm32"))]
            window.set_visible(true);
            self.window = Some(window);
            self.set_mouse_mode(MouseMode::Visible);
            self.sync_refresh_rate();
            eprintln!(
                "[perro][runtime] active_refresh_rate=({:?})",
                self.pacer.refresh_hz()
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
            self.pacer.reset_deadline();
            self.frame_index = 0;
            self.fps_window_start = now;
            self.fps_window_frames = 0;
            self.batch_start = now;
            self.timing_warmup_frames_left = TIMING_WARMUP_FRAMES;
            self.batch = BatchCoreStats::default();
            #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
            {
                self.batch_ui = BatchUiStats::default();
            }
            #[cfg(feature = "profile_heavy")]
            {
                self.batch_ui = BatchUiStats::default();
                self.batch_heavy = BatchHeavyStats::default();
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
                // Window may land on another monitor; refresh the cached rate.
                self.sync_refresh_rate();
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
                self.sync_refresh_rate();
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
                self.request_exit(event_loop, AppExitResult::window_close());
            }
            _ => {}
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RunnerUserEvent) {
        match event {
            RunnerUserEvent::RequestExit => {
                self.request_exit(event_loop, AppExitResult::event_loop_exit());
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: RunnerUserEvent) {}

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
        if self.window.is_some() && !self.pacer.blocks_frame(now) {
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
