use crate::App;
use perro_graphics::GraphicsBackend;
use std::time::{Duration, Instant};
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

const DEFAULT_FPS_CAP: f32 = 144.0;
const DEFAULT_FIXED_TIMESTEP: Option<f32> = None;
const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;
const LOG_INTERVAL_SECONDS: f32 = 2.5;
const FPS_CAP_COMPENSATION: f32 = 1.0;
const SPIN_TAIL_THRESHOLD: Duration = Duration::from_micros(500);

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
    if d.is_zero() {
        None
    } else {
        Some(d)
    }
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
        let _ = event_loop.run_app(&mut state);
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
    window: Option<Window>,
    last_frame_start: Instant,
    next_frame_deadline: Instant,
    run_start: Instant,
    batch_start: Instant,
    batch_work: Duration,
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
            batch_sim_delta_seconds: 0.0,
        }
    }
}

impl<B: GraphicsBackend> winit::application::ApplicationHandler for RunnerState<B> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default().with_title(self.title.clone());
            let window = event_loop
                .create_window(attrs)
                .expect("failed to create winit window");
            self.window = Some(window);
            let now = Instant::now();
            self.last_frame_start = now;
            self.next_frame_deadline = now;
            self.run_start = now;
            self.fixed_accumulator = 0.0;
            self.batch_start = now;
            self.batch_frames = 0;
            self.batch_work = Duration::ZERO;
            self.batch_sim_delta_seconds = 0.0;
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                let mut now = Instant::now();
                if let Some(target) = target_frame_duration(self.fps_cap) {
                    if self.next_frame_deadline <= self.last_frame_start {
                        self.next_frame_deadline = self.last_frame_start + target;
                    }

                    if now < self.next_frame_deadline {
                        let remaining = self.next_frame_deadline - now;
                        if remaining > SPIN_TAIL_THRESHOLD {
                            std::thread::sleep(remaining - SPIN_TAIL_THRESHOLD);
                        }
                        while Instant::now() < self.next_frame_deadline {
                            std::hint::spin_loop();
                        }
                        now = Instant::now();
                    }

                    self.next_frame_deadline += target;
                    while self.next_frame_deadline <= now {
                        self.next_frame_deadline += target;
                    }
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
                if let Some(step) = self.fixed_timestep {
                    self.fixed_accumulator += frame_delta.as_secs_f32();
                    let mut steps = 0u32;
                    while self.fixed_accumulator >= step && steps < MAX_FIXED_STEPS_PER_FRAME {
                        self.app.fixed_update_runtime(step);
                        self.fixed_accumulator -= step;
                        steps += 1;
                    }
                    if steps == MAX_FIXED_STEPS_PER_FRAME && self.fixed_accumulator >= step {
                        // Drop excess accumulated time to avoid spiral-of-death behavior.
                        self.fixed_accumulator = 0.0;
                    }
                    self.app.present();
                    simulated_delta_seconds = step as f64 * steps as f64;
                } else {
                    self.app.frame(frame_delta.as_secs_f32());
                }
                let work_duration = work_start.elapsed();

                let frame_end = Instant::now();

                self.batch_frames = self.batch_frames.saturating_add(1);
                self.batch_work += work_duration;
                self.batch_sim_delta_seconds += simulated_delta_seconds;

                let batch_elapsed_secs = frame_end.duration_since(self.batch_start).as_secs_f32();
                if batch_elapsed_secs >= LOG_INTERVAL_SECONDS && self.batch_frames > 0 {
                    let batch_elapsed = frame_end.duration_since(self.batch_start);
                    let capped_fps = self.batch_frames as f32 / batch_elapsed.as_secs_f32();
                    let work_us = self.batch_work.as_secs_f64() * 1_000_000.0;
                    let avg_work_us = work_us / self.batch_frames as f64;
                    let avg_work_ns = avg_work_us * 1_000.0;
                    let loop_fps = if self.batch_work.is_zero() {
                        f64::INFINITY
                    } else {
                        self.batch_frames as f64 / self.batch_work.as_secs_f64()
                    };
                    let delta_ms = frame_delta.as_secs_f64() * 1000.0;
                    let avg_sim_delta_ms =
                        (self.batch_sim_delta_seconds * 1000.0) / self.batch_frames as f64;

                    println!(
                        "delta: {:.3}ms | sim_avg: {:.3}ms | capped_fps: {:.2} | {} loops work: {:.2}us total ({:.3}us avg | {:.1}ns avg, {:.1} uncapped eq)",
                        delta_ms,
                        avg_sim_delta_ms,
                        capped_fps,
                        self.batch_frames,
                        work_us,
                        avg_work_us,
                        avg_work_ns,
                        loop_fps
                    );

                    self.batch_frames = 0;
                    self.batch_work = Duration::ZERO;
                    self.batch_sim_delta_seconds = 0.0;
                    self.batch_start = frame_end;
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
