use perro_graphics::GraphicsBackend;
use perro_ids::{NodeID, TextureID, string_to_u64};
use perro_input_api::{InputFrame, InputRingBuffer};
use perro_render_bridge::{
    Camera2DState, Command2D, Rect2DCommand, RenderCommand, RenderEvent, RenderRequestID,
    ResourceCommand, Sprite2DCommand,
};
use perro_runtime::{Runtime, WindowRequest};
use perro_runtime_api::sub_apis::FrameRateCap as RuntimeFrameRateCap;
use std::{
    collections::VecDeque,
    fs,
    io::Write,
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize, Size},
    event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{ModifiersState, PhysicalKey},
    window::{Fullscreen, Icon, WindowAttributes},
};

use crate::input::{GamepadInput, JoyConInput};
use crate::winit_runner::{
    fit_aspect,
    image_helpers::{PreloadedStartupSplash, preload_project_images},
    map_cursor_icon,
};

const MIN_FRAME_RATE_CAP_FPS: f32 = 1.0;
const MAX_FRAME_RATE_CAP_FPS: f32 = 1000.0;
const HIGH_RATE_FRAME_INTERVAL: Duration = Duration::from_millis(8);
const INITIAL_WINDOW_MONITOR_FRACTION: f32 = 0.75;

fn normalize_frame_rate_cap(cap: RuntimeFrameRateCap) -> RuntimeFrameRateCap {
    match cap {
        RuntimeFrameRateCap::Fps(fps) if fps.is_finite() && fps > 0.0 => {
            RuntimeFrameRateCap::Fps(fps.clamp(MIN_FRAME_RATE_CAP_FPS, MAX_FRAME_RATE_CAP_FPS))
        }
        RuntimeFrameRateCap::Fps(_) => RuntimeFrameRateCap::Unlimited,
        other => other,
    }
}

fn project_frame_rate_cap(cap: perro_runtime::FrameRateCap) -> RuntimeFrameRateCap {
    match cap {
        perro_runtime::FrameRateCap::Unlimited => RuntimeFrameRateCap::Unlimited,
        perro_runtime::FrameRateCap::Fps(fps) => RuntimeFrameRateCap::Fps(fps),
        perro_runtime::FrameRateCap::RefreshRate => RuntimeFrameRateCap::RefreshRate,
    }
}

fn frame_interval_from_fps(fps: f32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(fps))
}

fn active_refresh_rate_hz(window: Option<&winit::window::Window>) -> Option<f32> {
    let monitor = window.and_then(winit::window::Window::current_monitor)?;
    let refresh_millihertz = monitor
        .video_modes()
        .map(|mode| mode.refresh_rate_millihertz())
        .max()?;
    if refresh_millihertz == 0 {
        return None;
    }
    Some(refresh_millihertz as f32 / 1000.0)
}

fn refresh_rate_interval(window: Option<&winit::window::Window>) -> Option<Duration> {
    let refresh_hz = active_refresh_rate_hz(window)?;
    Some(Duration::from_secs_f64(1.0 / f64::from(refresh_hz)))
}

fn sim_frame_cap_interval(cap: RuntimeFrameRateCap) -> Option<Duration> {
    match normalize_frame_rate_cap(cap) {
        RuntimeFrameRateCap::Unlimited => None,
        RuntimeFrameRateCap::Fps(fps) => Some(frame_interval_from_fps(fps)),
        RuntimeFrameRateCap::RefreshRate => None,
    }
}

fn wait_until_sim_deadline(deadline: Instant, interval: Duration) {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        let remaining = deadline.duration_since(now);
        if interval > HIGH_RATE_FRAME_INTERVAL && remaining > Duration::from_millis(2) {
            thread::sleep(remaining - Duration::from_millis(1));
        } else {
            std::hint::spin_loop();
        }
    }
}

const STARTUP_SPLASH_FADE_DURATION: Duration = Duration::from_millis(320);
const STARTUP_SPLASH_HOLD_DURATION: Duration = Duration::from_millis(2000);
const STARTUP_SPLASH_MAX_WIDTH_FRAC: f32 = 0.44;
const STARTUP_SPLASH_MAX_HEIGHT_FRAC: f32 = 0.34;
const STARTUP_SPLASH_TEXTURE_REQUEST: RenderRequestID = RenderRequestID::new(0x5453_504C_4153_485F);
const STARTUP_SPLASH_TEXTURE_ID: TextureID =
    TextureID::from_u64(string_to_u64("__threaded_startup_splash_tex__"));
const STARTUP_SPLASH_BG_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__threaded_startup_splash_bg__"));
const STARTUP_SPLASH_IMAGE_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__threaded_startup_splash_image__"));

const DEFAULT_INPUT_RING_CAPACITY: usize = 4096;
const DEFAULT_SNAPSHOT_RING_CAPACITY: usize = 3;
const LOG_INTERVAL_SECONDS: f32 = 3.0;
#[cfg(not(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps")))]
const LOG_TIMING_SAMPLE_STRIDE: u32 = 20;
const TIMING_WARMUP_FRAMES: u32 = 8;

#[inline]
fn should_sample_timing_frame(frame_index: u64) -> bool {
    #[cfg(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps"))]
    {
        let _ = frame_index;
        true
    }
    #[cfg(not(any(feature = "profile_heavy", feature = "ui_profile", feature = "fps")))]
    {
        frame_index == 1 || frame_index.is_multiple_of(LOG_TIMING_SAMPLE_STRIDE as u64)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SimFrameTiming {
    pub simulation_time: Duration,
    pub fixed_steps: u32,
    pub fixed_step_seconds: f32,
    pub fixed_accum_before: f32,
    pub fixed_accum_after: f32,
    pub fixed_catchup_dropped: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct RenderFrameTiming {
    graphics_time: Duration,
    frame_time: Duration,
    fps: f32,
    active_refresh_rate: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ThreadedPresentTiming {
    gpu_present: Duration,
    active: Duration,
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

#[inline]
fn avg_micros(total: Duration, samples: u32) -> u128 {
    if samples == 0 {
        return 0;
    }
    total.as_micros() / u128::from(samples)
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

#[derive(Debug, Clone)]
pub struct RenderSnapshot {
    pub frame_id: u64,
    pub sim_time: f32,
    pub timing: SimFrameTiming,
    pub viewport_size: [u32; 2],
    pub commands: Vec<RenderCommand>,
}

impl RenderSnapshot {
    pub fn new(
        frame_id: u64,
        sim_time: f32,
        timing: SimFrameTiming,
        viewport_size: [u32; 2],
        commands: Vec<RenderCommand>,
    ) -> Self {
        Self {
            frame_id,
            sim_time,
            timing,
            viewport_size,
            commands,
        }
    }

    fn recycle_commands(self, pool: &CommandBufferPool) {
        pool.recycle(self.commands);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotRingStats {
    pub dropped_snapshots: u64,
    pub skipped_snapshots: u64,
    pub published_snapshots: u64,
}

pub struct SnapshotRing {
    snapshots: VecDeque<RenderSnapshot>,
    capacity: usize,
    stats: SnapshotRingStats,
}

impl SnapshotRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(capacity),
            capacity,
            stats: SnapshotRingStats::default(),
        }
    }

    fn push(&mut self, snapshot: RenderSnapshot, pool: &CommandBufferPool) {
        if self.capacity == 0 {
            self.stats.dropped_snapshots = self.stats.dropped_snapshots.saturating_add(1);
            snapshot.recycle_commands(pool);
            return;
        }
        if self.snapshots.len() == self.capacity {
            if let Some(snapshot) = self.snapshots.pop_front() {
                snapshot.recycle_commands(pool);
            }
            self.stats.dropped_snapshots = self.stats.dropped_snapshots.saturating_add(1);
        }
        self.snapshots.push_back(snapshot);
        self.stats.published_snapshots = self.stats.published_snapshots.saturating_add(1);
    }

    fn take_latest(&mut self, pool: &CommandBufferPool) -> Option<RenderSnapshot> {
        let skipped = self.snapshots.len().saturating_sub(1) as u64;
        self.stats.skipped_snapshots = self.stats.skipped_snapshots.saturating_add(skipped);
        let latest = self.snapshots.pop_back();
        while let Some(snapshot) = self.snapshots.pop_front() {
            snapshot.recycle_commands(pool);
        }
        latest
    }

    #[inline]
    pub fn stats(&self) -> SnapshotRingStats {
        self.stats
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

#[derive(Default)]
struct CommandBufferPool {
    buffers: Mutex<Vec<Vec<RenderCommand>>>,
}

impl CommandBufferPool {
    fn checkout(&self) -> Vec<RenderCommand> {
        match self.buffers.lock() {
            Ok(mut buffers) => buffers.pop().unwrap_or_default(),
            Err(err) => err.into_inner().pop().unwrap_or_default(),
        }
    }

    fn recycle(&self, mut buffer: Vec<RenderCommand>) {
        buffer.clear();
        match self.buffers.lock() {
            Ok(mut buffers) => buffers.push(buffer),
            Err(err) => err.into_inner().push(buffer),
        }
    }

    #[cfg(test)]
    fn available(&self) -> usize {
        match self.buffers.lock() {
            Ok(buffers) => buffers.len(),
            Err(err) => err.into_inner().len(),
        }
    }
}

#[derive(Clone)]
pub struct SharedSnapshotRing {
    ring: Arc<Mutex<SnapshotRing>>,
    dropped_on_lock: Arc<AtomicU64>,
}

impl SharedSnapshotRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: Arc::new(Mutex::new(SnapshotRing::new(capacity))),
            dropped_on_lock: Arc::new(AtomicU64::new(0)),
        }
    }

    fn publish_latest_wins(&self, snapshot: RenderSnapshot, pool: &CommandBufferPool) {
        match self.ring.try_lock() {
            Ok(mut ring) => ring.push(snapshot, pool),
            Err(TryLockError::WouldBlock) => {
                self.dropped_on_lock.fetch_add(1, Ordering::Relaxed);
                snapshot.recycle_commands(pool);
            }
            Err(TryLockError::Poisoned(err)) => err.into_inner().push(snapshot, pool),
        }
    }

    fn take_latest(&self, pool: &CommandBufferPool) -> Option<RenderSnapshot> {
        match self.ring.lock() {
            Ok(mut ring) => ring.take_latest(pool),
            Err(err) => err.into_inner().take_latest(pool),
        }
    }

    pub fn stats(&self) -> SnapshotRingStats {
        let mut stats = match self.ring.lock() {
            Ok(ring) => ring.stats(),
            Err(err) => err.into_inner().stats(),
        };
        stats.dropped_snapshots = stats
            .dropped_snapshots
            .saturating_add(self.dropped_on_lock.load(Ordering::Relaxed));
        stats
    }
}

#[derive(Clone)]
pub struct RenderThreadBridge {
    input_ring: Arc<Mutex<InputRingBuffer>>,
    snapshot_ring: SharedSnapshotRing,
    command_pool: Arc<CommandBufferPool>,
    render_timing: Arc<Mutex<RenderFrameTiming>>,
    render_event_tx: Sender<RenderEvent>,
    window_request_rx: Arc<Mutex<Receiver<WindowRequest>>>,
    stop: Arc<AtomicBool>,
}

impl RenderThreadBridge {
    pub fn new() -> (Self, SimBridge) {
        Self::with_capacity(DEFAULT_INPUT_RING_CAPACITY, DEFAULT_SNAPSHOT_RING_CAPACITY)
    }

    pub fn with_capacity(input_capacity: usize, snapshot_capacity: usize) -> (Self, SimBridge) {
        let input_ring = Arc::new(Mutex::new(InputRingBuffer::new(input_capacity)));
        let snapshot_ring = SharedSnapshotRing::new(snapshot_capacity);
        let command_pool = Arc::new(CommandBufferPool::default());
        let render_timing = Arc::new(Mutex::new(RenderFrameTiming::default()));
        let (render_event_tx, render_event_rx) = mpsc::channel();
        let (window_request_tx, window_request_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));

        (
            Self {
                input_ring: input_ring.clone(),
                snapshot_ring: snapshot_ring.clone(),
                command_pool: command_pool.clone(),
                render_timing: render_timing.clone(),
                render_event_tx,
                window_request_rx: Arc::new(Mutex::new(window_request_rx)),
                stop: stop.clone(),
            },
            SimBridge {
                input_ring,
                snapshot_ring,
                command_pool,
                render_timing,
                render_event_rx,
                window_request_tx,
                stop,
            },
        )
    }

    pub fn push_input_event(&self, event: perro_input_api::InputEvent) {
        match self.input_ring.lock() {
            Ok(mut ring) => ring.push(event),
            Err(err) => err.into_inner().push(event),
        }
    }

    pub fn take_latest_snapshot(&self) -> Option<RenderSnapshot> {
        self.snapshot_ring.take_latest(&self.command_pool)
    }

    pub fn send_render_events<I>(&self, events: I)
    where
        I: IntoIterator<Item = RenderEvent>,
    {
        for event in events {
            let _ = self.render_event_tx.send(event);
        }
    }

    pub fn drain_window_requests(&self, out: &mut Vec<WindowRequest>) {
        let rx = match self.window_request_rx.lock() {
            Ok(rx) => rx,
            Err(err) => err.into_inner(),
        };
        while let Ok(request) = rx.try_recv() {
            out.push(request);
        }
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    fn recycle_command_buffer(&self, buffer: Vec<RenderCommand>) {
        self.command_pool.recycle(buffer);
    }

    fn publish_render_timing(&self, timing: RenderFrameTiming) {
        match self.render_timing.lock() {
            Ok(mut slot) => *slot = timing,
            Err(err) => *err.into_inner() = timing,
        }
    }

    pub fn snapshot_stats(&self) -> SnapshotRingStats {
        self.snapshot_ring.stats()
    }
}

pub struct SimBridge {
    input_ring: Arc<Mutex<InputRingBuffer>>,
    snapshot_ring: SharedSnapshotRing,
    command_pool: Arc<CommandBufferPool>,
    render_timing: Arc<Mutex<RenderFrameTiming>>,
    render_event_rx: Receiver<RenderEvent>,
    window_request_tx: Sender<WindowRequest>,
    stop: Arc<AtomicBool>,
}

impl SimBridge {
    fn seal_input_frame(&self) -> InputFrame {
        match self.input_ring.lock() {
            Ok(mut ring) => ring.seal_frame(),
            Err(err) => err.into_inner().seal_frame(),
        }
    }

    fn apply_render_events(&self, runtime: &mut Runtime) {
        while let Ok(event) = self.render_event_rx.try_recv() {
            runtime.apply_render_event(event);
        }
    }

    fn publish_snapshot(&self, snapshot: RenderSnapshot) {
        self.snapshot_ring
            .publish_latest_wins(snapshot, &self.command_pool);
    }

    fn send_window_requests(&self, requests: &mut Vec<WindowRequest>) {
        for request in requests.drain(..) {
            let _ = self.window_request_tx.send(request);
        }
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    fn checkout_command_buffer(&self) -> Vec<RenderCommand> {
        self.command_pool.checkout()
    }

    fn read_render_timing(&self) -> RenderFrameTiming {
        match self.render_timing.lock() {
            Ok(slot) => *slot,
            Err(err) => *err.into_inner(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SimThreadConfig {
    pub fixed_timestep: Option<f32>,
    pub idle_sleep: Duration,
}

impl Default for SimThreadConfig {
    fn default() -> Self {
        Self {
            fixed_timestep: None,
            idle_sleep: Duration::ZERO,
        }
    }
}

pub struct SimThread {
    handle: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl SimThread {
    pub fn spawn<F>(runtime_factory: F, bridge: SimBridge, config: SimThreadConfig) -> Self
    where
        F: FnOnce() -> Runtime + Send + 'static,
    {
        let stop = bridge.stop.clone();
        let handle = thread::Builder::new()
            .name("perro_sim".to_string())
            .spawn(move || run_sim_loop(runtime_factory(), bridge, config))
            .expect("failed to spawn perro sim thread");
        Self {
            handle: Some(handle),
            stop,
        }
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn join(mut self) -> thread::Result<()> {
        self.request_stop();
        if let Some(handle) = self.handle.take() {
            handle.join()
        } else {
            Ok(())
        }
    }
}

impl Drop for SimThread {
    fn drop(&mut self) {
        self.request_stop();
    }
}

pub struct SnapshotPresenter<B: GraphicsBackend> {
    graphics: B,
    event_buffer: Vec<RenderEvent>,
    current_snapshot: Option<RenderSnapshot>,
}

impl<B: GraphicsBackend> SnapshotPresenter<B> {
    pub fn new(graphics: B) -> Self {
        Self {
            graphics,
            event_buffer: Vec::new(),
            current_snapshot: None,
        }
    }

    pub fn graphics(&self) -> &B {
        &self.graphics
    }

    pub fn graphics_mut(&mut self) -> &mut B {
        &mut self.graphics
    }

    pub fn present_from_bridge(&mut self, bridge: &RenderThreadBridge) {
        self.present_from_bridge_with_overlay(bridge, std::iter::empty::<RenderCommand>());
    }

    pub fn present_from_bridge_with_overlay<I>(&mut self, bridge: &RenderThreadBridge, overlay: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        if let Some(snapshot) = bridge.take_latest_snapshot()
            && let Some(old_snapshot) = self.current_snapshot.replace(snapshot)
        {
            bridge.recycle_command_buffer(old_snapshot.commands);
        }
        if let Some(snapshot) = &self.current_snapshot {
            self.graphics.submit_many(snapshot.commands.iter().cloned());
        }
        self.graphics.submit_many(overlay);
        self.graphics.draw_frame();
        self.graphics.drain_events(&mut self.event_buffer);
        bridge.send_render_events(self.event_buffer.drain(..));
    }

    fn present_from_bridge_with_overlay_timed<I>(
        &mut self,
        bridge: &RenderThreadBridge,
        overlay: I,
    ) -> ThreadedPresentTiming
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        let total_start = Instant::now();
        if let Some(snapshot) = bridge.take_latest_snapshot()
            && let Some(old_snapshot) = self.current_snapshot.replace(snapshot)
        {
            bridge.recycle_command_buffer(old_snapshot.commands);
        }
        if let Some(snapshot) = &self.current_snapshot {
            self.graphics.submit_many(snapshot.commands.iter().cloned());
        }
        self.graphics.submit_many(overlay);
        let draw_timing = self.graphics.draw_frame_timed();
        let gpu_present = draw_timing
            .as_ref()
            .map(|timing| timing.gpu_acquire + timing.gpu_submit_queue_main + timing.gpu_present)
            .unwrap_or(Duration::ZERO);
        let active = total_start.elapsed().saturating_sub(gpu_present);
        self.graphics.drain_events(&mut self.event_buffer);
        bridge.send_render_events(self.event_buffer.drain(..));
        ThreadedPresentTiming {
            gpu_present,
            active,
        }
    }
}

pub struct ThreadedWinitRunner;

impl ThreadedWinitRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run<B, F>(
        self,
        graphics: B,
        title: &str,
        runtime_factory: F,
    ) -> Result<crate::winit_runner::AppExitResult, crate::winit_runner::AppExitError>
    where
        B: GraphicsBackend + 'static,
        F: FnOnce() -> Runtime + Send + 'static,
    {
        self.run_with_timestep(graphics, title, runtime_factory, None)
    }

    pub fn run_with_timestep<B, F>(
        self,
        graphics: B,
        title: &str,
        runtime_factory: F,
        fixed_timestep: Option<f32>,
    ) -> Result<crate::winit_runner::AppExitResult, crate::winit_runner::AppExitError>
    where
        B: GraphicsBackend + 'static,
        F: FnOnce() -> Runtime + Send + 'static,
    {
        let event_loop = EventLoop::new().map_err(|err| crate::winit_runner::AppExitError {
            message: format!("failed to create winit event loop: {err}"),
        })?;
        let (render_bridge, sim_bridge) = RenderThreadBridge::new();
        let sim = SimThread::spawn(
            runtime_factory,
            sim_bridge,
            SimThreadConfig {
                fixed_timestep,
                ..SimThreadConfig::default()
            },
        );
        let mut state = ThreadedRunnerState::new(
            SnapshotPresenter::new(graphics),
            render_bridge,
            sim,
            title.to_string(),
            None,
            None,
        );
        event_loop
            .run_app(&mut state)
            .map_err(|err| crate::winit_runner::AppExitError {
                message: format!("winit event loop failed: {err}"),
            })?;
        state.shutdown();
        Ok(state
            .exit_result
            .take()
            .unwrap_or_else(crate::winit_runner::AppExitResult::event_loop_exit))
    }

    pub fn run_with_timestep_and_startup<B, F>(
        self,
        graphics: B,
        title: &str,
        runtime_factory: F,
        fixed_timestep: Option<f32>,
        startup_splash: Option<ThreadedStartupSplash>,
        window_icon: Option<Icon>,
    ) -> Result<crate::winit_runner::AppExitResult, crate::winit_runner::AppExitError>
    where
        B: GraphicsBackend + 'static,
        F: FnOnce() -> Runtime + Send + 'static,
    {
        let event_loop = EventLoop::new().map_err(|err| crate::winit_runner::AppExitError {
            message: format!("failed to create winit event loop: {err}"),
        })?;
        let (render_bridge, sim_bridge) = RenderThreadBridge::new();
        let sim = SimThread::spawn(
            runtime_factory,
            sim_bridge,
            SimThreadConfig {
                fixed_timestep,
                ..SimThreadConfig::default()
            },
        );
        let mut state = ThreadedRunnerState::new(
            SnapshotPresenter::new(graphics),
            render_bridge,
            sim,
            title.to_string(),
            startup_splash,
            window_icon,
        );
        event_loop
            .run_app(&mut state)
            .map_err(|err| crate::winit_runner::AppExitError {
                message: format!("winit event loop failed: {err}"),
            })?;
        state.shutdown();
        Ok(state
            .exit_result
            .take()
            .unwrap_or_else(crate::winit_runner::AppExitResult::event_loop_exit))
    }
}

impl Default for ThreadedWinitRunner {
    fn default() -> Self {
        Self::new()
    }
}

struct ThreadedRunnerState<B: GraphicsBackend> {
    presenter: SnapshotPresenter<B>,
    bridge: RenderThreadBridge,
    sim: Option<SimThread>,
    title: String,
    window: Option<Arc<winit::window::Window>>,
    window_requests: Vec<WindowRequest>,
    exit_result: Option<crate::winit_runner::AppExitResult>,
    last_cursor_position: Option<winit::dpi::PhysicalPosition<f64>>,
    last_window_position: Option<PhysicalPosition<i32>>,
    modifiers: ModifiersState,
    gamepad_input: GamepadInput,
    joycon_input: JoyConInput,
    startup_splash: Option<ThreadedStartupSplashState>,
    window_icon: Option<Icon>,
    last_frame_start: Instant,
    last_frame_end: Instant,
    frame_rate_cap: RuntimeFrameRateCap,
    next_frame_deadline: Option<Instant>,
    frame_index: u64,
    timing_csv: Option<TimingCsvWriter>,
    timing_warmup_frames_left: u32,
    batch_start: Instant,
    batch_frames: u32,
    batch_timing_samples: u32,
    batch_work: Duration,
    batch_simulation: Duration,
    batch_present: Duration,
    batch_idle_before_frame: Duration,
    batch_present_wait: Duration,
    batch_idle: Duration,
}

impl<B: GraphicsBackend> ThreadedRunnerState<B> {
    fn new(
        presenter: SnapshotPresenter<B>,
        bridge: RenderThreadBridge,
        sim: SimThread,
        title: String,
        startup_splash: Option<ThreadedStartupSplash>,
        window_icon: Option<Icon>,
    ) -> Self {
        let now = Instant::now();
        Self {
            presenter,
            bridge,
            sim: Some(sim),
            title,
            window: None,
            window_requests: Vec::new(),
            exit_result: None,
            last_cursor_position: None,
            last_window_position: None,
            modifiers: ModifiersState::empty(),
            gamepad_input: GamepadInput::new(),
            joycon_input: JoyConInput::new(),
            startup_splash: startup_splash.map(ThreadedStartupSplashState::new),
            window_icon,
            last_frame_start: now,
            last_frame_end: now,
            frame_rate_cap: RuntimeFrameRateCap::Unlimited,
            next_frame_deadline: None,
            frame_index: 0,
            timing_csv: TimingCsvWriter::from_env(),
            timing_warmup_frames_left: TIMING_WARMUP_FRAMES,
            batch_start: now,
            batch_frames: 0,
            batch_timing_samples: 0,
            batch_work: Duration::ZERO,
            batch_simulation: Duration::ZERO,
            batch_present: Duration::ZERO,
            batch_idle_before_frame: Duration::ZERO,
            batch_present_wait: Duration::ZERO,
            batch_idle: Duration::ZERO,
        }
    }

    fn window_attributes(&mut self, event_loop: &ActiveEventLoop) -> WindowAttributes {
        let desired = self
            .startup_splash
            .as_ref()
            .map(|splash| {
                PhysicalSize::new(
                    splash.config.virtual_size[0].max(1),
                    splash.config.virtual_size[1].max(1),
                )
            })
            .unwrap_or_else(|| PhysicalSize::new(1920, 1080));
        let mut attrs = WindowAttributes::default().with_title(self.title.clone());
        if let Some(icon) = self.window_icon.take() {
            attrs = attrs.with_window_icon(Some(icon));
        }
        let Some(monitor) = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
        else {
            return attrs.with_inner_size(Size::Physical(desired));
        };
        let max_width =
            ((monitor.size().width as f32) * INITIAL_WINDOW_MONITOR_FRACTION).floor() as u32;
        let max_height =
            ((monitor.size().height as f32) * INITIAL_WINDOW_MONITOR_FRACTION).floor() as u32;
        attrs.with_inner_size(Size::Physical(fit_aspect(
            desired,
            max_width.max(1),
            max_height.max(1),
        )))
    }

    fn shutdown(&mut self) {
        self.bridge.request_stop();
        if let Some(sim) = self.sim.take() {
            let _ = sim.join();
        }
        if let Some(snapshot) = self.presenter.current_snapshot.take() {
            self.bridge.recycle_command_buffer(snapshot.commands);
        }
    }

    fn apply_window_requests(&mut self) {
        self.bridge.drain_window_requests(&mut self.window_requests);
        let Some(window) = &self.window else {
            self.window_requests.clear();
            return;
        };
        for request in self.window_requests.drain(..) {
            match request {
                WindowRequest::SetTitle(title) => window.set_title(&title),
                WindowRequest::SetSize { width, height } => {
                    let _ = window.request_inner_size(PhysicalSize::new(width, height));
                }
                WindowRequest::SetMode(perro_runtime::WindowMode::Windowed) => {
                    window.set_fullscreen(None);
                }
                WindowRequest::SetMode(perro_runtime::WindowMode::BorderlessFullscreen) => {
                    window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
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
                    window.set_cursor(map_cursor_icon(icon));
                }
            }
        }
    }

    fn frame_cap_interval(&self) -> Option<Duration> {
        match self.frame_rate_cap {
            RuntimeFrameRateCap::Unlimited => None,
            RuntimeFrameRateCap::Fps(fps) => Some(frame_interval_from_fps(fps)),
            RuntimeFrameRateCap::RefreshRate => refresh_rate_interval(self.window.as_deref())
                .or_else(|| Some(frame_interval_from_fps(60.0))),
        }
    }

    fn cap_blocks_frame(&self, now: Instant) -> bool {
        self.next_frame_deadline
            .is_some_and(|deadline| deadline > now)
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
        if self.startup_splash.is_some()
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

    fn handle_input_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = crate::input::kbm::map_winit_key_code(code)
                {
                    self.bridge
                        .push_input_event(perro_input_api::InputEvent::Key {
                            key,
                            is_down: event.state == ElementState::Pressed,
                        });
                }
                if event.state == ElementState::Pressed
                    && !self.text_input_suppressed()
                    && let Some(text) = event.text.as_ref()
                    && text.chars().any(|ch| !ch.is_control())
                {
                    self.bridge
                        .push_input_event(perro_input_api::InputEvent::Text(text.to_string()));
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::Focused(false) => {
                self.modifiers = ModifiersState::empty();
                self.last_cursor_position = None;
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.bridge
                    .push_input_event(perro_input_api::InputEvent::Text(text.clone()));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = crate::input::kbm::map_winit_mouse_button(*button) {
                    self.bridge
                        .push_input_event(perro_input_api::InputEvent::MouseButton {
                            button,
                            is_down: *state == ElementState::Pressed,
                        });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(prev) = self.last_cursor_position {
                    self.bridge
                        .push_input_event(perro_input_api::InputEvent::MouseDelta {
                            dx: (position.x - prev.x) as f32,
                            dy: (prev.y - position.y) as f32,
                        });
                }
                self.bridge
                    .push_input_event(perro_input_api::InputEvent::MousePosition {
                        x: position.x as f32,
                        y: position.y as f32,
                    });
                self.last_cursor_position = Some(*position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_cursor_position = None;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => {
                        ((pos.x as f32) / 40.0, (pos.y as f32) / 40.0)
                    }
                };
                self.bridge
                    .push_input_event(perro_input_api::InputEvent::MouseWheel { dx, dy });
            }
            _ => {}
        }
    }

    fn text_input_suppressed(&self) -> bool {
        self.modifiers.control_key() || self.modifiers.alt_key() || self.modifiers.super_key()
    }

    fn sync_window_position(&mut self, position: PhysicalPosition<i32>) {
        if let (Some(prev), Some(cursor)) = (self.last_window_position, self.last_cursor_position) {
            let next = PhysicalPosition::new(
                cursor.x + f64::from(prev.x - position.x),
                cursor.y + f64::from(prev.y - position.y),
            );
            self.last_cursor_position = Some(next);
            self.bridge
                .push_input_event(perro_input_api::InputEvent::MousePosition {
                    x: next.x as f32,
                    y: next.y as f32,
                });
        }
        self.last_window_position = Some(position);
    }

    #[inline]
    fn should_sample_timing(&self) -> bool {
        should_sample_timing_frame(self.frame_index)
    }

    fn reset_timing_batch(&mut self, now: Instant) {
        self.timing_warmup_frames_left = TIMING_WARMUP_FRAMES;
        self.batch_start = now;
        self.batch_frames = 0;
        self.batch_timing_samples = 0;
        self.batch_work = Duration::ZERO;
        self.batch_simulation = Duration::ZERO;
        self.batch_present = Duration::ZERO;
        self.batch_idle_before_frame = Duration::ZERO;
        self.batch_present_wait = Duration::ZERO;
        self.batch_idle = Duration::ZERO;
    }

    fn record_timing(
        &mut self,
        phase: &'static str,
        frame_start: Instant,
        frame_delta: Duration,
        idle_duration: Duration,
        present_timing: Option<ThreadedPresentTiming>,
    ) {
        let should_sample_timing = self.should_sample_timing();
        let present_wait_duration = present_timing
            .map(|timing| timing.gpu_present)
            .unwrap_or(Duration::ZERO);
        let present_active_duration = present_timing
            .map(|timing| timing.active)
            .unwrap_or(Duration::ZERO);
        let work_active_duration = present_active_duration;
        let simulation_timing = self
            .presenter
            .current_snapshot
            .as_ref()
            .map(|snapshot| snapshot.timing)
            .unwrap_or_default();
        let simulation_duration = simulation_timing.simulation_time;
        let measured_frame_duration = work_active_duration
            .saturating_add(idle_duration)
            .saturating_add(present_wait_duration);
        let frame_end = Instant::now();
        self.last_frame_end = frame_end;
        if should_sample_timing {
            self.bridge.publish_render_timing(RenderFrameTiming {
                graphics_time: present_active_duration,
                frame_time: measured_frame_duration,
                fps: if measured_frame_duration.is_zero() {
                    0.0
                } else {
                    1.0 / measured_frame_duration.as_secs_f32()
                },
                active_refresh_rate: active_refresh_rate_hz(self.window.as_deref()),
            });
        }

        if let Some(csv) = &mut self.timing_csv {
            csv.write(CsvFrameSample {
                frame_index: self.frame_index,
                phase,
                warmup: self.timing_warmup_frames_left > 0,
                sampled: should_sample_timing,
                frame_delta_us: frame_delta.as_micros(),
                idle_before_frame_us: idle_duration.as_micros(),
                simulation_us: simulation_duration.as_micros(),
                render_active_us: present_active_duration.as_micros(),
                work_active_us: work_active_duration.as_micros(),
                present_wait_us: present_wait_duration.as_micros(),
                fixed_steps: simulation_timing.fixed_steps,
                fixed_step_us: Duration::from_secs_f32(simulation_timing.fixed_step_seconds)
                    .as_micros(),
                fixed_accum_before_us: Duration::from_secs_f32(
                    simulation_timing.fixed_accum_before,
                )
                .as_micros(),
                fixed_accum_after_us: Duration::from_secs_f32(simulation_timing.fixed_accum_after)
                    .as_micros(),
                fixed_catchup_dropped: simulation_timing.fixed_catchup_dropped,
            });
        }

        if self.timing_warmup_frames_left > 0 {
            self.timing_warmup_frames_left = self.timing_warmup_frames_left.saturating_sub(1);
            if self.timing_warmup_frames_left == 0 {
                self.batch_start = frame_end;
            }
            return;
        }

        self.batch_frames = self.batch_frames.saturating_add(1);
        if should_sample_timing {
            self.batch_timing_samples = self.batch_timing_samples.saturating_add(1);
            self.batch_work += work_active_duration;
            self.batch_simulation += simulation_duration;
            self.batch_present += present_active_duration;
            self.batch_idle_before_frame += idle_duration;
            self.batch_present_wait += present_wait_duration;
            self.batch_idle += idle_duration + present_wait_duration;
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
            self.batch_start = frame_end;
            self.batch_frames = 0;
            self.batch_timing_samples = 0;
            self.batch_work = Duration::ZERO;
            self.batch_simulation = Duration::ZERO;
            self.batch_present = Duration::ZERO;
            self.batch_idle_before_frame = Duration::ZERO;
            self.batch_present_wait = Duration::ZERO;
            self.batch_idle = Duration::ZERO;
        }

        let _ = frame_start;
    }
}

#[derive(Clone, Debug)]
pub struct ThreadedStartupSplash {
    pub source: Option<String>,
    pub source_hash: Option<u64>,
    pub image_size: Option<(u32, u32)>,
    pub texture_size: Option<(u32, u32)>,
    pub virtual_size: [u32; 2],
}

impl ThreadedStartupSplash {
    pub fn from_project(project: &perro_runtime::RuntimeProject) -> Self {
        let preload = preload_project_images(Some(project));
        Self::from_preloaded(project, preload.startup_splash)
    }

    pub(crate) fn from_preloaded(
        project: &perro_runtime::RuntimeProject,
        preload: Option<PreloadedStartupSplash>,
    ) -> Self {
        Self {
            source: preload.as_ref().map(|splash| splash.source.clone()),
            source_hash: preload.as_ref().and_then(|splash| splash.source_hash),
            image_size: preload.as_ref().and_then(|splash| splash.image_size),
            texture_size: preload.as_ref().and_then(|splash| splash.texture_size),
            virtual_size: [
                project.config.virtual_width.max(1),
                project.config.virtual_height.max(1),
            ],
        }
    }
}

struct ThreadedStartupSplashState {
    config: ThreadedStartupSplash,
    shown_at: Instant,
    fade_started_at: Option<Instant>,
    texture_requested: bool,
    active: bool,
}

impl ThreadedStartupSplashState {
    fn new(config: ThreadedStartupSplash) -> Self {
        Self {
            config,
            shown_at: Instant::now(),
            fade_started_at: None,
            texture_requested: false,
            active: true,
        }
    }

    fn alpha(&self, now: Instant) -> f32 {
        let Some(started) = self.fade_started_at else {
            return 1.0;
        };
        let elapsed = now.saturating_duration_since(started);
        if elapsed >= STARTUP_SPLASH_FADE_DURATION {
            0.0
        } else {
            1.0 - elapsed.as_secs_f32() / STARTUP_SPLASH_FADE_DURATION.as_secs_f32()
        }
    }

    fn update(&mut self, now: Instant, has_snapshot: bool) {
        if !self.active {
            return;
        }
        let shown_for = now.saturating_duration_since(self.shown_at);
        if self.fade_started_at.is_none()
            && has_snapshot
            && shown_for >= STARTUP_SPLASH_HOLD_DURATION
        {
            self.fade_started_at = Some(now);
        }
        if self.fade_started_at.is_some_and(|started| {
            now.saturating_duration_since(started) >= STARTUP_SPLASH_FADE_DURATION
        }) {
            self.active = false;
        }
    }

    fn commands(&mut self, now: Instant, window_size: PhysicalSize<u32>) -> Vec<RenderCommand> {
        let alpha = self.alpha(now);
        let fallback_width = self.config.virtual_size[0] as f32;
        let fallback_height = self.config.virtual_size[1] as f32;
        let window_width = if window_size.width > 0 {
            window_size.width as f32
        } else {
            fallback_width
        };
        let window_height = if window_size.height > 0 {
            window_size.height as f32
        } else {
            fallback_height
        };
        let mut commands = Vec::with_capacity(4);
        commands.push(RenderCommand::TwoD(Command2D::SetCamera {
            camera: Camera2DState::default(),
        }));
        commands.push(RenderCommand::TwoD(Command2D::UpsertRect {
            node: STARTUP_SPLASH_BG_NODE,
            rect: Rect2DCommand {
                center: [0.0, 0.0],
                size: [window_width, window_height],
                color: [0.0, 0.0, 0.0, alpha].into(),
                z_index: 950,
            },
        }));
        if !self.texture_requested
            && let Some(source) = self.config.source.clone()
        {
            self.texture_requested = true;
            commands.push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request: STARTUP_SPLASH_TEXTURE_REQUEST,
                id: STARTUP_SPLASH_TEXTURE_ID,
                source: self
                    .config
                    .source_hash
                    .map(|v| v.to_string())
                    .unwrap_or(source),
                reserved: true,
            }));
        }
        if self.config.source.is_some() {
            let (image_w, image_h) = self.config.image_size.unwrap_or((512, 512));
            let (texture_w, texture_h) = self.config.texture_size.unwrap_or((image_w, image_h));
            let max_w = window_width * STARTUP_SPLASH_MAX_WIDTH_FRAC;
            let max_h = window_height * STARTUP_SPLASH_MAX_HEIGHT_FRAC;
            let scale = (max_w / image_w as f32)
                .min(max_h / image_h as f32)
                .max(0.001);
            commands.push(RenderCommand::TwoD(Command2D::UpsertSprite {
                node: STARTUP_SPLASH_IMAGE_NODE,
                sprite: Sprite2DCommand {
                    texture: STARTUP_SPLASH_TEXTURE_ID,
                    model: [[scale, 0.0, 0.0], [0.0, scale, 0.0], [0.0, 0.0, 1.0]],
                    tint: [1.0, 1.0, 1.0, alpha].into(),
                    z_index: 951,
                    uv_min: [0.0, 0.0],
                    uv_max: [texture_w as f32, texture_h as f32],
                    size: [image_w as f32, image_h as f32],
                },
            }));
        }
        commands
    }

    fn cleanup_commands() -> [RenderCommand; 2] {
        [
            RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_BG_NODE,
            }),
            RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_IMAGE_NODE,
            }),
        ]
    }
}

impl<B: GraphicsBackend> Drop for ThreadedRunnerState<B> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl<B: GraphicsBackend> winit::application::ApplicationHandler for ThreadedRunnerState<B> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(self.window_attributes(event_loop))
                .expect("failed to create winit window"),
        );
        self.last_window_position = window.outer_position().ok();
        window.set_ime_allowed(true);
        let size = window.inner_size();
        self.presenter.graphics_mut().attach_window(window.clone());
        self.presenter
            .graphics_mut()
            .resize(size.width, size.height);
        let now = Instant::now();
        self.last_frame_start = now;
        self.last_frame_end = now;
        self.batch_start = now;
        self.bridge
            .push_input_event(perro_input_api::InputEvent::ViewportSize {
                width: size.width,
                height: size.height,
            });
        eprintln!(
            "[perro][runtime] active_refresh_rate=({:?})",
            active_refresh_rate_hz(Some(window.as_ref()))
        );
        self.window = Some(window);
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

        match &event {
            WindowEvent::Resized(size) => {
                self.presenter
                    .graphics_mut()
                    .resize(size.width, size.height);
                self.bridge
                    .push_input_event(perro_input_api::InputEvent::ViewportSize {
                        width: size.width,
                        height: size.height,
                    });
            }
            WindowEvent::Moved(position) => {
                self.sync_window_position(*position);
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                if self.startup_splash.is_none() && self.cap_blocks_frame(frame_start) {
                    self.apply_frame_control_flow(event_loop, frame_start);
                    return;
                }
                self.frame_index = self.frame_index.saturating_add(1);
                let frame_delta = frame_start.duration_since(self.last_frame_start);
                self.last_frame_start = frame_start;
                let idle_duration = frame_start.saturating_duration_since(self.last_frame_end);
                let had_snapshot = self.bridge.snapshot_stats().published_snapshots > 0;
                let now = frame_start;
                let mut overlay = Vec::new();
                let mut phase = "steady";
                let mut splash_finished = false;
                if let Some(splash) = &mut self.startup_splash {
                    phase = "startup";
                    splash.update(now, had_snapshot);
                    if splash.active {
                        let window_size = self
                            .window
                            .as_ref()
                            .map(|window| window.inner_size())
                            .unwrap_or_else(|| PhysicalSize::new(0, 0));
                        overlay = splash.commands(now, window_size);
                    } else {
                        overlay.extend(ThreadedStartupSplashState::cleanup_commands());
                        self.startup_splash = None;
                        splash_finished = true;
                    }
                }
                let present_timing = Some(
                    self.presenter
                        .present_from_bridge_with_overlay_timed(&self.bridge, overlay),
                );
                self.apply_window_requests();
                if splash_finished {
                    self.reset_timing_batch(now);
                }
                self.record_timing(
                    phase,
                    frame_start,
                    frame_delta,
                    idle_duration,
                    present_timing,
                );
                self.update_frame_deadline(frame_start, Instant::now());
            }
            WindowEvent::CloseRequested => {
                self.exit_result = Some(crate::winit_runner::AppExitResult::window_close());
                self.bridge.request_stop();
                event_loop.exit();
            }
            _ => self.handle_input_event(&event),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.bridge
                .push_input_event(perro_input_api::InputEvent::MouseDelta {
                    dx: delta.0 as f32,
                    dy: -(delta.1 as f32),
                });
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() || self.exit_result.is_some() {
            return;
        }
        let now = Instant::now();
        self.apply_frame_control_flow(event_loop, now);
        self.gamepad_input.begin_frame_threaded(&self.bridge);
        self.joycon_input.begin_frame_threaded(&self.bridge);
        if let Some(window) = &self.window
            && (self
                .next_frame_deadline
                .is_none_or(|deadline| deadline <= now)
                || self.startup_splash.is_some())
        {
            window.request_redraw();
        }
    }
}

fn run_sim_loop(mut runtime: Runtime, bridge: SimBridge, config: SimThreadConfig) {
    let mut frame_id = 0u64;
    let run_start = Instant::now();
    let mut last_tick = Instant::now();
    let mut fixed_accumulator = 0.0f32;
    let mut commands = bridge.checkout_command_buffer();
    let mut window_requests = Vec::new();
    let mut sim_frame_rate_cap = runtime
        .project()
        .map(|project| project_frame_rate_cap(project.config.frame_rate_cap))
        .unwrap_or(RuntimeFrameRateCap::Unlimited);
    eprintln!("[perro][runtime] frame_rate_cap=({sim_frame_rate_cap:?})");
    if runtime.project().is_some() {
        window_requests.push(WindowRequest::SetFrameRateCap(sim_frame_rate_cap));
        bridge.send_window_requests(&mut window_requests);
    }

    while !bridge.should_stop() {
        let tick_start = Instant::now();
        let delta = tick_start
            .saturating_duration_since(last_tick)
            .as_secs_f32()
            .min(0.250);
        last_tick = tick_start;
        let render_timing = bridge.read_render_timing();
        let frame_delta = render_timing.frame_time.as_secs_f32();
        let delta = if frame_delta.is_finite() && frame_delta > 0.0 {
            frame_delta.min(0.250)
        } else {
            delta
        };
        runtime.set_active_refresh_rate(render_timing.active_refresh_rate);
        let next_frame_id = frame_id.saturating_add(1);
        let should_sample_timing = should_sample_timing_frame(next_frame_id);

        let input_frame = bridge.seal_input_frame();
        runtime.time.elapsed = tick_start
            .saturating_duration_since(run_start)
            .as_secs_f32();
        if should_sample_timing {
            runtime.time.graphics = render_timing.graphics_time;
            runtime.time.frame = render_timing.frame_time;
            runtime.time.fps = render_timing.fps;
            runtime.time.draw_gpu_prepare_3d = Duration::ZERO;
            runtime.time.draw_gpu_prepare_3d_frustum = Duration::ZERO;
            runtime.time.draw_gpu_prepare_3d_hiz = Duration::ZERO;
            runtime.time.draw_gpu_prepare_3d_indirect = Duration::ZERO;
            runtime.time.draw_gpu_prepare_3d_cull_inputs = Duration::ZERO;
            runtime.time.draw_calls_2d = 0;
            runtime.time.draw_calls_3d = 0;
            runtime.time.draw_calls_total = 0;
            runtime.time.draw_instances_3d = 0;
            runtime.time.draw_material_refs_3d = 0;
            runtime.time.skip_prepare_3d = 0;
            runtime.time.skip_prepare_3d_frustum = 0;
            runtime.time.skip_prepare_3d_hiz = 0;
            runtime.time.skip_prepare_3d_indirect = 0;
            runtime.time.skip_prepare_3d_cull_inputs = 0;
        }
        runtime.apply_input_frame(&input_frame);
        bridge.apply_render_events(&mut runtime);

        let fixed_accumulator_before = fixed_accumulator;
        let mut fixed_steps = 1u32;
        let mut fixed_step_seconds = delta;
        let fixed_catchup_dropped = false;
        if let Some(step) = config.fixed_timestep {
            fixed_steps = 0;
            fixed_accumulator += delta;
            while fixed_accumulator >= step {
                runtime.fixed_update(step);
                fixed_accumulator -= step;
                fixed_steps = fixed_steps.saturating_add(1);
            }
            fixed_step_seconds = step;
            runtime.set_physics_render_alpha((fixed_accumulator / step).clamp(0.0, 1.0));
        } else {
            runtime.fixed_update(delta);
            runtime.set_physics_render_alpha(1.0);
        }
        runtime.update(delta);

        runtime.drain_window_requests(&mut window_requests);
        for request in &window_requests {
            if let WindowRequest::SetFrameRateCap(cap) = request {
                sim_frame_rate_cap = normalize_frame_rate_cap(*cap);
                eprintln!("[perro][runtime] sim_frame_rate_cap=({sim_frame_rate_cap:?})");
            }
        }
        bridge.send_window_requests(&mut window_requests);

        commands.clear();
        runtime.extract_render_snapshot_commands(&mut commands);
        let mut snapshot_commands = bridge.checkout_command_buffer();
        std::mem::swap(&mut commands, &mut snapshot_commands);
        let simulation_duration = if should_sample_timing {
            let duration = tick_start.elapsed();
            runtime.time.simulation = duration;
            duration
        } else {
            runtime.time.simulation
        };
        frame_id = next_frame_id;
        bridge.publish_snapshot(RenderSnapshot::new(
            frame_id,
            runtime.time.elapsed,
            SimFrameTiming {
                simulation_time: simulation_duration,
                fixed_steps,
                fixed_step_seconds,
                fixed_accum_before: fixed_accumulator_before,
                fixed_accum_after: fixed_accumulator,
                fixed_catchup_dropped,
            },
            runtime.input_viewport_size_pixels(),
            snapshot_commands,
        ));

        if let Some(interval) = sim_frame_cap_interval(sim_frame_rate_cap) {
            let target = tick_start
                .checked_add(interval)
                .unwrap_or_else(Instant::now);
            wait_until_sim_deadline(target, interval);
        } else if !config.idle_sleep.is_zero() {
            thread::sleep(config.idle_sleep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_render_bridge::{Command2D, RenderCommand};

    fn snapshot(frame_id: u64) -> RenderSnapshot {
        RenderSnapshot::new(
            frame_id,
            frame_id as f32,
            SimFrameTiming::default(),
            [640, 360],
            vec![RenderCommand::TwoD(Command2D::RemoveNode {
                node: perro_ids::NodeID::from_u64(frame_id),
            })],
        )
    }

    fn assert_send<T: Send>() {}

    #[test]
    fn render_snapshot_is_send() {
        assert_send::<RenderSnapshot>();
    }

    #[test]
    fn ring_keeps_latest_and_counts_drops() {
        let mut ring = SnapshotRing::new(3);
        let pool = CommandBufferPool::default();
        ring.push(snapshot(1), &pool);
        ring.push(snapshot(2), &pool);
        ring.push(snapshot(3), &pool);
        ring.push(snapshot(4), &pool);

        assert_eq!(ring.stats().dropped_snapshots, 1);
        let latest = ring.take_latest(&pool).unwrap();
        assert_eq!(latest.frame_id, 4);
        assert_eq!(ring.stats().skipped_snapshots, 2);
        assert!(ring.is_empty());
    }

    #[test]
    fn ring_recycles_dropped_and_skipped_buffers() {
        let mut ring = SnapshotRing::new(2);
        let pool = CommandBufferPool::default();

        ring.push(snapshot(1), &pool);
        ring.push(snapshot(2), &pool);
        ring.push(snapshot(3), &pool);
        let latest = ring.take_latest(&pool).unwrap();
        latest.recycle_commands(&pool);

        assert_eq!(pool.available(), 3);
    }

    #[test]
    fn bridge_routes_window_requests() {
        let (render, sim) = RenderThreadBridge::new();
        let mut requests = vec![WindowRequest::SetTitle("x".to_string())];
        sim.send_window_requests(&mut requests);

        let mut out = Vec::new();
        render.drain_window_requests(&mut out);

        assert_eq!(out, vec![WindowRequest::SetTitle("x".to_string())]);
    }

    #[test]
    fn default_sim_config_does_not_sleep_when_uncapped() {
        assert_eq!(SimThreadConfig::default().idle_sleep, Duration::ZERO);
    }

    #[test]
    fn high_rate_caps_use_short_intervals() {
        let interval = sim_frame_cap_interval(RuntimeFrameRateCap::Fps(300.0)).unwrap();
        assert!(interval <= HIGH_RATE_FRAME_INTERVAL);
    }

    #[test]
    fn sim_publishes_snapshots_without_render_drain() {
        let (render, sim) = RenderThreadBridge::with_capacity(8, 3);
        let sim = SimThread::spawn(
            Runtime::new,
            sim,
            SimThreadConfig {
                fixed_timestep: Some(1.0 / 60.0),
                idle_sleep: Duration::from_millis(1),
            },
        );
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline && render.snapshot_stats().published_snapshots < 3 {
            thread::sleep(Duration::from_millis(1));
        }
        let stats = render.snapshot_stats();
        let _ = sim.join();

        assert!(stats.published_snapshots >= 3);
    }
}
