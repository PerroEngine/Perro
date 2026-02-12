use crate::App;
use perro_graphics::GraphicsBackend;
use std::time::{Duration, Instant};
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

const DEFAULT_FPS_CAP: f32 = 60.0;
const LOG_INTERVAL_SECONDS: f32 = 2.5;
const FPS_CAP_COMPENSATION: f32 = 1.03;
const SPIN_TAIL_THRESHOLD: Duration = Duration::from_micros(500);

pub struct WinitRunner;

impl WinitRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run<B: GraphicsBackend>(self, app: App<B>, title: &str) {
        self.run_with_fps_cap(app, title, DEFAULT_FPS_CAP);
    }

    pub fn run_with_fps_cap<B: GraphicsBackend>(self, app: App<B>, title: &str, fps_cap: f32) {
        let event_loop = EventLoop::new().expect("failed to create winit event loop");
        let mut state = RunnerState::new(app, title, fps_cap);
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
    fps_cap: f32,
    last_frame_end: Instant,
    batch_frames: u32,
    batch_start: Instant,
    batch_work: Duration,
}

impl<B: GraphicsBackend> RunnerState<B> {
    fn new(app: App<B>, title: &str, fps_cap: f32) -> Self {
        let now = Instant::now();
        Self {
            app,
            title: title.to_owned(),
            window: None,
            fps_cap: fps_cap.max(1.0),
            last_frame_end: now,
            batch_frames: 0,
            batch_start: now,
            batch_work: Duration::ZERO,
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
            self.last_frame_end = now;
            self.batch_start = now;
            self.batch_frames = 0;
            self.batch_work = Duration::ZERO;
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
                let target =
                    Duration::from_secs_f32(1.0 / (self.fps_cap * FPS_CAP_COMPENSATION));
                let frame_start = Instant::now();
                let delta = frame_start.duration_since(self.last_frame_end);

                let work_start = Instant::now();
                self.app.frame(delta.as_secs_f32());
                let work_duration = work_start.elapsed();

                let elapsed = frame_start.elapsed();
                if elapsed < target {
                    let remaining = target - elapsed;
                    if remaining > SPIN_TAIL_THRESHOLD {
                        std::thread::sleep(remaining - SPIN_TAIL_THRESHOLD);
                    }
                    let deadline = frame_start + target;
                    while Instant::now() < deadline {
                        std::hint::spin_loop();
                    }
                }

                let frame_end = Instant::now();
                self.last_frame_end = frame_end;

                self.batch_frames = self.batch_frames.saturating_add(1);
                self.batch_work += work_duration;

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
                    let delta_ms = delta.as_secs_f64() * 1000.0;

                    println!(
                        "delta: {:.3}ms | capped_fps: {:.2} | {} loops work: {:.2}us total ({:.3}us avg | {:.1}ns avg, {:.1} uncapped eq)",
                        delta_ms,
                        capped_fps,
                        self.batch_frames,
                        work_us,
                        avg_work_us,
                        avg_work_ns,
                        loop_fps
                    );

                    self.batch_frames = 0;
                    self.batch_work = Duration::ZERO;
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
