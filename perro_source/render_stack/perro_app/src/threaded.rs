use perro_graphics::GraphicsBackend;
use perro_ids::{NodeID, TextureID, string_to_u64};
use perro_input_api::{InputFrame, InputRingBuffer};
use perro_render_bridge::{
    Camera2DState, Command2D, Rect2DCommand, RenderCommand, RenderEvent, RenderRequestID,
    ResourceCommand, Sprite2DCommand,
};
use perro_runtime::{Runtime, WindowRequest};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use winit::{
    dpi::{PhysicalSize, Size},
    event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{ModifiersState, PhysicalKey},
    window::{Fullscreen, WindowAttributes},
};

use crate::input::{GamepadInput, JoyConInput};
use crate::winit_runner::image_helpers::load_image_size;

const STARTUP_SPLASH_FADE_DURATION: Duration = Duration::from_millis(320);
const STARTUP_SPLASH_HOLD_DURATION: Duration = Duration::from_millis(2000);
const STARTUP_SPLASH_TEXTURE_REQUEST: RenderRequestID = RenderRequestID::new(0x5453_504C_4153_485F);
const STARTUP_SPLASH_TEXTURE_ID: TextureID =
    TextureID::from_u64(string_to_u64("__threaded_startup_splash_tex__"));
const STARTUP_SPLASH_BG_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__threaded_startup_splash_bg__"));
const STARTUP_SPLASH_IMAGE_NODE: NodeID =
    NodeID::from_u64(string_to_u64("__threaded_startup_splash_image__"));

const DEFAULT_INPUT_RING_CAPACITY: usize = 4096;
const DEFAULT_SNAPSHOT_RING_CAPACITY: usize = 3;

#[derive(Debug, Clone)]
pub struct RenderSnapshot {
    pub frame_id: u64,
    pub sim_time: f32,
    pub viewport_size: [u32; 2],
    pub commands: Vec<RenderCommand>,
}

impl RenderSnapshot {
    pub fn new(
        frame_id: u64,
        sim_time: f32,
        viewport_size: [u32; 2],
        commands: Vec<RenderCommand>,
    ) -> Self {
        Self {
            frame_id,
            sim_time,
            viewport_size,
            commands,
        }
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

    pub fn push(&mut self, snapshot: RenderSnapshot) {
        if self.capacity == 0 {
            self.stats.dropped_snapshots = self.stats.dropped_snapshots.saturating_add(1);
            return;
        }
        if self.snapshots.len() == self.capacity {
            self.snapshots.pop_front();
            self.stats.dropped_snapshots = self.stats.dropped_snapshots.saturating_add(1);
        }
        self.snapshots.push_back(snapshot);
        self.stats.published_snapshots = self.stats.published_snapshots.saturating_add(1);
    }

    pub fn take_latest(&mut self) -> Option<RenderSnapshot> {
        let skipped = self.snapshots.len().saturating_sub(1) as u64;
        self.stats.skipped_snapshots = self.stats.skipped_snapshots.saturating_add(skipped);
        let latest = self.snapshots.pop_back();
        self.snapshots.clear();
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

    pub fn publish_latest_wins(&self, snapshot: RenderSnapshot) {
        match self.ring.try_lock() {
            Ok(mut ring) => ring.push(snapshot),
            Err(TryLockError::WouldBlock) => {
                self.dropped_on_lock.fetch_add(1, Ordering::Relaxed);
            }
            Err(TryLockError::Poisoned(err)) => err.into_inner().push(snapshot),
        }
    }

    pub fn take_latest(&self) -> Option<RenderSnapshot> {
        match self.ring.lock() {
            Ok(mut ring) => ring.take_latest(),
            Err(err) => err.into_inner().take_latest(),
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
        let (render_event_tx, render_event_rx) = mpsc::channel();
        let (window_request_tx, window_request_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));

        (
            Self {
                input_ring: input_ring.clone(),
                snapshot_ring: snapshot_ring.clone(),
                render_event_tx,
                window_request_rx: Arc::new(Mutex::new(window_request_rx)),
                stop: stop.clone(),
            },
            SimBridge {
                input_ring,
                snapshot_ring,
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
        self.snapshot_ring.take_latest()
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

    pub fn snapshot_stats(&self) -> SnapshotRingStats {
        self.snapshot_ring.stats()
    }
}

pub struct SimBridge {
    input_ring: Arc<Mutex<InputRingBuffer>>,
    snapshot_ring: SharedSnapshotRing,
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
        self.snapshot_ring.publish_latest_wins(snapshot);
    }

    fn send_window_requests(&self, requests: &mut Vec<WindowRequest>) {
        for request in requests.drain(..) {
            let _ = self.window_request_tx.send(request);
        }
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Acquire)
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
            idle_sleep: Duration::from_millis(1),
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
        if let Some(snapshot) = bridge.take_latest_snapshot() {
            self.current_snapshot = Some(snapshot);
        }
        if let Some(snapshot) = &self.current_snapshot {
            self.graphics.submit_many(snapshot.commands.iter().cloned());
        }
        self.graphics.submit_many(overlay);
        self.graphics.draw_frame();
        self.graphics.drain_events(&mut self.event_buffer);
        bridge.send_render_events(self.event_buffer.drain(..));
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
    modifiers: ModifiersState,
    gamepad_input: GamepadInput,
    joycon_input: JoyConInput,
    startup_splash: Option<ThreadedStartupSplashState>,
}

impl<B: GraphicsBackend> ThreadedRunnerState<B> {
    fn new(
        presenter: SnapshotPresenter<B>,
        bridge: RenderThreadBridge,
        sim: SimThread,
        title: String,
        startup_splash: Option<ThreadedStartupSplash>,
    ) -> Self {
        Self {
            presenter,
            bridge,
            sim: Some(sim),
            title,
            window: None,
            window_requests: Vec::new(),
            exit_result: None,
            last_cursor_position: None,
            modifiers: ModifiersState::empty(),
            gamepad_input: GamepadInput::new(),
            joycon_input: JoyConInput::new(),
            startup_splash: startup_splash.map(ThreadedStartupSplashState::new),
        }
    }

    fn shutdown(&mut self) {
        self.bridge.request_stop();
        if let Some(sim) = self.sim.take() {
            let _ = sim.join();
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
            }
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
}

#[derive(Clone, Debug)]
pub struct ThreadedStartupSplash {
    pub source: Option<String>,
    pub source_hash: Option<u64>,
    pub image_size: Option<(u32, u32)>,
    pub virtual_size: [u32; 2],
}

impl ThreadedStartupSplash {
    pub fn from_project(project: &perro_runtime::RuntimeProject) -> Self {
        let mut source = None::<String>;
        let mut source_hash = None::<u64>;
        let splash = project.config.startup_splash.trim();
        if !splash.is_empty() {
            source = Some(splash.to_string());
            source_hash = project.config.startup_splash_hash;
        } else {
            let icon = project.config.icon.trim();
            if !icon.is_empty() {
                source = Some(icon.to_string());
                source_hash = project.config.icon_hash;
            }
        }
        let image_size = source
            .as_deref()
            .and_then(|s| load_image_size(project, s, source_hash));
        Self {
            source,
            source_hash,
            image_size,
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

    fn commands(&mut self, now: Instant) -> Vec<RenderCommand> {
        let alpha = self.alpha(now);
        let virtual_width = self.config.virtual_size[0] as f32;
        let virtual_height = self.config.virtual_size[1] as f32;
        let mut commands = Vec::with_capacity(4);
        commands.push(RenderCommand::TwoD(Command2D::SetCamera {
            camera: Camera2DState::default(),
        }));
        commands.push(RenderCommand::TwoD(Command2D::UpsertRect {
            node: STARTUP_SPLASH_BG_NODE,
            rect: Rect2DCommand {
                center: [0.0, 0.0],
                size: [virtual_width, virtual_height],
                color: [0.0, 0.0, 0.0, alpha],
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
            let scale = ((virtual_width * 0.44) / image_w as f32)
                .min((virtual_height * 0.34) / image_h as f32)
                .max(0.001);
            commands.push(RenderCommand::TwoD(Command2D::UpsertSprite {
                node: STARTUP_SPLASH_IMAGE_NODE,
                sprite: Sprite2DCommand {
                    texture: STARTUP_SPLASH_TEXTURE_ID,
                    model: [[scale, 0.0, 0.0], [0.0, scale, 0.0], [0.0, 0.0, 1.0]],
                    tint: [1.0, 1.0, 1.0, alpha],
                    z_index: 951,
                    uv_min: [0.0, 0.0],
                    uv_max: [0.0, 0.0],
                    size: [0.0, 0.0],
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
                .create_window(
                    WindowAttributes::default()
                        .with_title(self.title.clone())
                        .with_inner_size(Size::Physical(PhysicalSize::new(1280, 720))),
                )
                .expect("failed to create winit window"),
        );
        window.set_ime_allowed(true);
        let size = window.inner_size();
        self.presenter.graphics_mut().attach_window(window.clone());
        self.presenter
            .graphics_mut()
            .resize(size.width, size.height);
        self.bridge
            .push_input_event(perro_input_api::InputEvent::ViewportSize {
                width: size.width,
                height: size.height,
            });
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
            WindowEvent::RedrawRequested => {
                let had_snapshot = self.bridge.snapshot_stats().published_snapshots > 0;
                let now = Instant::now();
                let mut overlay = Vec::new();
                if let Some(splash) = &mut self.startup_splash {
                    splash.update(now, had_snapshot);
                    if splash.active {
                        overlay = splash.commands(now);
                    } else {
                        overlay.extend(ThreadedStartupSplashState::cleanup_commands());
                        self.startup_splash = None;
                    }
                }
                self.presenter
                    .present_from_bridge_with_overlay(&self.bridge, overlay);
                self.apply_window_requests();
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
        self.gamepad_input.begin_frame_threaded(&self.bridge);
        self.joycon_input.begin_frame_threaded(&self.bridge);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn run_sim_loop(mut runtime: Runtime, bridge: SimBridge, config: SimThreadConfig) {
    let mut frame_id = 0u64;
    let run_start = Instant::now();
    let mut last_tick = Instant::now();
    let mut fixed_accumulator = 0.0f32;
    let mut commands = Vec::new();
    let mut window_requests = Vec::new();

    while !bridge.should_stop() {
        let tick_start = Instant::now();
        let delta = tick_start
            .saturating_duration_since(last_tick)
            .as_secs_f32()
            .min(0.250);
        last_tick = tick_start;

        let input_frame = bridge.seal_input_frame();
        runtime.time.elapsed = tick_start
            .saturating_duration_since(run_start)
            .as_secs_f32();
        runtime.apply_input_frame(&input_frame);
        bridge.apply_render_events(&mut runtime);

        if let Some(step) = config.fixed_timestep {
            fixed_accumulator += delta;
            while fixed_accumulator >= step {
                runtime.fixed_update(step);
                fixed_accumulator -= step;
            }
        } else {
            runtime.fixed_update(delta);
        }
        runtime.update(delta);

        runtime.drain_window_requests(&mut window_requests);
        bridge.send_window_requests(&mut window_requests);

        commands.clear();
        runtime.extract_render_snapshot_commands(&mut commands);
        frame_id = frame_id.saturating_add(1);
        bridge.publish_snapshot(RenderSnapshot::new(
            frame_id,
            runtime.time.elapsed,
            runtime.input_viewport_size_pixels(),
            std::mem::take(&mut commands),
        ));
        commands = Vec::new();

        if !config.idle_sleep.is_zero() {
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
        ring.push(snapshot(1));
        ring.push(snapshot(2));
        ring.push(snapshot(3));
        ring.push(snapshot(4));

        assert_eq!(ring.stats().dropped_snapshots, 1);
        let latest = ring.take_latest().unwrap();
        assert_eq!(latest.frame_id, 4);
        assert_eq!(ring.stats().skipped_snapshots, 2);
        assert!(ring.is_empty());
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
        let deadline = Instant::now() + Duration::from_millis(80);
        while Instant::now() < deadline && render.snapshot_stats().published_snapshots < 3 {
            thread::sleep(Duration::from_millis(1));
        }
        let stats = render.snapshot_stats();
        let _ = sim.join();

        assert!(stats.published_snapshots >= 3);
    }
}
