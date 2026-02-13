use crate::App;
use perro_graphics::GraphicsBackend;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    monitor::MonitorHandle,
    window::{Window, WindowAttributes},
};

const DEFAULT_FPS_CAP: f32 = 60.0;
const DEFAULT_FIXED_TIMESTEP: Option<f32> = None;
const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;
const LOG_INTERVAL_SECONDS: f32 = 2.5;
const FPS_CAP_COMPENSATION: f32 = 1.01;

#[inline]
fn target_frame_duration(fps_cap: f32) -> Option<Duration> {
    let effective_fps = fps_cap * FPS_CAP_COMPENSATION;
    if !effective_fps.is_finite() || effective_fps <= 0.0 {
        return None;
    }
    let secs = 1.0f64 / effective_fps as f64;
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
    next_frame_deadline: Instant,
    run_start: Instant,
    batch_start: Instant,
    batch_work: Duration,
    batch_runtime_update: Duration,
    batch_present: Duration,
    batch_sim_delta_seconds: f64,
    fixed_timestep: Option<f32>,
    fps_cap: f32,
    fixed_accumulator: f32,
    batch_frames: u32,
}

impl<B: GraphicsBackend> RunnerState<B> {
    fn new(app: App<B>, title: &str, fps_cap: f32, fixed_timestep: Option<f32>) -> Self {
        let now = Instant::now();
        Self {
            app,
            title: title.to_owned(),
            window: None,
            fps_cap: fps_cap.max(1.0),
            fixed_timestep: fixed_timestep.filter(|v| *v > 0.0),
            fixed_accumulator: 0.0,
            last_frame_start: now,
            next_frame_deadline: now,
            run_start: now,
            batch_frames: 0,
            batch_start: now,
            batch_work: Duration::ZERO,
            batch_runtime_update: Duration::ZERO,
            batch_present: Duration::ZERO,
            batch_sim_delta_seconds: 0.0,
        }
    }
}

impl<B: GraphicsBackend> winit::application::ApplicationHandler for RunnerState<B> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
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
            self.window = Some(window);
            let now = Instant::now();
            self.last_frame_start = now;
            self.next_frame_deadline = now;
            self.run_start = now;
            self.fixed_accumulator = 0.0;
            self.batch_start = now;
            self.batch_frames = 0;
            self.batch_work = Duration::ZERO;
            self.batch_runtime_update = Duration::ZERO;
            self.batch_present = Duration::ZERO;
            self.batch_sim_delta_seconds = 0.0;
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_some_and(|window| window.id() != window_id)
        {
            return;
        }

        match event {
            WindowEvent::Resized(size) => {
                self.app.resize_surface(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    self.app.resize_surface(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                if let Some(target) = target_frame_duration(self.fps_cap) {
                    if self.next_frame_deadline <= self.last_frame_start {
                        self.next_frame_deadline = self.last_frame_start + target;
                    }

                    if now < self.next_frame_deadline {
                        // Skip this redraw without blocking event handling; we'll request another
                        // redraw in about_to_wait and render once the deadline is reached.
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
                let mut simulated_delta_seconds = frame_delta.as_secs_f64();

                let work_start = Instant::now();
                let mut runtime_update_duration = Duration::ZERO;
                let present_start;
                let present_duration;
                if let Some(step) = self.fixed_timestep {
                    self.fixed_accumulator += frame_delta.as_secs_f32();
                    let mut steps = 0u32;
                    while self.fixed_accumulator >= step && steps < MAX_FIXED_STEPS_PER_FRAME {
                        let update_start = Instant::now();
                        self.app.fixed_update_runtime(step);
                        runtime_update_duration += update_start.elapsed();
                        self.fixed_accumulator -= step;
                        steps += 1;
                    }
                    if steps == MAX_FIXED_STEPS_PER_FRAME && self.fixed_accumulator >= step {
                        // Drop excess accumulated time to avoid spiral-of-death behavior.
                        self.fixed_accumulator = 0.0;
                    }
                    present_start = Instant::now();
                    self.app.present();
                    present_duration = present_start.elapsed();
                    simulated_delta_seconds = step as f64 * steps as f64;
                } else {
                    let update_start = Instant::now();
                    self.app.update_runtime(frame_delta.as_secs_f32());
                    runtime_update_duration = update_start.elapsed();
                    present_start = Instant::now();
                    self.app.present();
                    present_duration = present_start.elapsed();
                }
                let work_duration = work_start.elapsed();

                let frame_end = Instant::now();

                self.batch_frames = self.batch_frames.saturating_add(1);
                self.batch_work += work_duration;
                self.batch_runtime_update += runtime_update_duration;
                self.batch_present += present_duration;
                self.batch_sim_delta_seconds += simulated_delta_seconds;

                let batch_elapsed_secs = frame_end.duration_since(self.batch_start).as_secs_f32();
                if batch_elapsed_secs >= LOG_INTERVAL_SECONDS && self.batch_frames > 0 {
                    let work_ms = self.batch_work.as_secs_f64() * 1_000.0;
                    let avg_work_us = (work_ms * 1_000.0) / self.batch_frames as f64;
                    let runtime_update_us = self.batch_runtime_update.as_secs_f64() * 1_000_000.0;
                    let avg_runtime_update_ns =
                        self.batch_runtime_update.as_nanos() as f64 / self.batch_frames as f64;
                    let present_ms = self.batch_present.as_secs_f64() * 1_000.0;
                    let avg_present_us = (present_ms * 1_000.0) / self.batch_frames as f64;
                    let loop_fps = if self.batch_work.is_zero() {
                        f64::INFINITY
                    } else {
                        self.batch_frames as f64 / self.batch_work.as_secs_f64()
                    };

                    println!(
                        "{} loops | update: {:.3}us total ({:.1}ns avg) | present: {:.3}ms total ({:.3}us avg) | total: {:.3}ms total ({:.3}us avg, {:.1} uncapped eq)",
                        self.batch_frames,
                        runtime_update_us,
                        avg_runtime_update_ns,
                        present_ms,
                        avg_present_us,
                        work_ms,
                        avg_work_us,
                        loop_fps
                    );

                    self.batch_frames = 0;
                    self.batch_work = Duration::ZERO;
                    self.batch_runtime_update = Duration::ZERO;
                    self.batch_present = Duration::ZERO;
                    self.batch_sim_delta_seconds = 0.0;
                    self.batch_start = frame_end;
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
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

    let mut attrs = WindowAttributes::default().with_title(title);
    let Some(project) = project else {
        return attrs;
    };

    let desired = PhysicalSize::new(project.config.virtual_width, project.config.virtual_height);
    if desired.width == 0 || desired.height == 0 {
        return attrs;
    }

    let Some(monitor) = pick_monitor(event_loop) else {
        return attrs.with_inner_size(Size::Physical(desired));
    };

    let max_width = ((monitor.size().width as f32) * 0.95f32).floor() as u32;
    let max_height = ((monitor.size().height as f32) * 0.95f32).floor() as u32;
    let fitted = fit_aspect(desired, max_width.max(1), max_height.max(1));
    let centered = center_position(&monitor, fitted);

    attrs = attrs.with_inner_size(Size::Physical(fitted));
    attrs.with_position(Position::Physical(centered))
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
