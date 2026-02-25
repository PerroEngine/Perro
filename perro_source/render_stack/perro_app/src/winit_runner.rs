use crate::App;
use perro_graphics::GraphicsBackend;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    monitor::MonitorHandle,
    window::{Window, WindowAttributes},
};

const DEFAULT_FPS_CAP: f32 = 60.0;
const DEFAULT_FIXED_TIMESTEP: Option<f32> = None;
const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;
const LOG_INTERVAL_SECONDS: f32 = 2.5;
const FPS_CAP_COMPENSATION: f32 = 1.01;
const INITIAL_WINDOW_MONITOR_FRACTION: f32 = 0.75;

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
    last_cursor_position: Option<PhysicalPosition<f64>>,
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
            last_cursor_position: None,
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
        let mut simulated_delta_seconds = frame_delta.as_secs_f64();

        let work_start = Instant::now();
        let mut runtime_update_duration = Duration::ZERO;
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
            let present_start = Instant::now();
            self.app.present();
            present_duration = present_start.elapsed();
            simulated_delta_seconds = step as f64 * steps as f64;
        } else {
            let update_start = Instant::now();
            self.app.update_runtime(frame_delta.as_secs_f32());
            runtime_update_duration = update_start.elapsed();
            let present_start = Instant::now();
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
            let avg_runtime_update_us =
                self.batch_runtime_update.as_micros() as f64 / self.batch_frames as f64;
            let present_ms = self.batch_present.as_secs_f64() * 1_000.0;
            let avg_present_us = (present_ms * 1_000.0) / self.batch_frames as f64;

            println!(
                "update: ({:.3}us avg) | frame present:  ({:.3}us avg) | total: ({:.3}us avg)",
                avg_runtime_update_us, avg_present_us, avg_work_us,
            );

            self.batch_frames = 0;
            self.batch_work = Duration::ZERO;
            self.batch_runtime_update = Duration::ZERO;
            self.batch_present = Duration::ZERO;
            self.batch_sim_delta_seconds = 0.0;
            self.batch_start = frame_end;
        }

        // Keep a continuous redraw chain like the legacy runner.
        if let Some(window) = &self.window {
            window.request_redraw();
        }

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
            self.last_cursor_position = None;
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
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = map_winit_key_code(code)
                {
                    self.app
                        .set_key_state(key, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(mapped) = map_winit_mouse_button(button) {
                    self.app
                        .set_mouse_button_state(mapped, state == ElementState::Pressed);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(prev) = self.last_cursor_position {
                    let dx = (position.x - prev.x) as f32;
                    let dy = (position.y - prev.y) as f32;
                    self.app.add_mouse_delta(dx, dy);
                }
                self.last_cursor_position = Some(position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_cursor_position = None;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(pos) => ((pos.x as f32) / 40.0, (pos.y as f32) / 40.0),
                };
                self.app.add_mouse_wheel(dx, dy);
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn map_winit_key_code(code: winit::keyboard::KeyCode) -> Option<perro_input::KeyCode> {
    match code {
        winit::keyboard::KeyCode::Backquote => Some(perro_input::KeyCode::Backquote),
        winit::keyboard::KeyCode::Backslash => Some(perro_input::KeyCode::Backslash),
        winit::keyboard::KeyCode::BracketLeft => Some(perro_input::KeyCode::BracketLeft),
        winit::keyboard::KeyCode::BracketRight => Some(perro_input::KeyCode::BracketRight),
        winit::keyboard::KeyCode::Comma => Some(perro_input::KeyCode::Comma),
        winit::keyboard::KeyCode::Digit0 => Some(perro_input::KeyCode::Digit0),
        winit::keyboard::KeyCode::Digit1 => Some(perro_input::KeyCode::Digit1),
        winit::keyboard::KeyCode::Digit2 => Some(perro_input::KeyCode::Digit2),
        winit::keyboard::KeyCode::Digit3 => Some(perro_input::KeyCode::Digit3),
        winit::keyboard::KeyCode::Digit4 => Some(perro_input::KeyCode::Digit4),
        winit::keyboard::KeyCode::Digit5 => Some(perro_input::KeyCode::Digit5),
        winit::keyboard::KeyCode::Digit6 => Some(perro_input::KeyCode::Digit6),
        winit::keyboard::KeyCode::Digit7 => Some(perro_input::KeyCode::Digit7),
        winit::keyboard::KeyCode::Digit8 => Some(perro_input::KeyCode::Digit8),
        winit::keyboard::KeyCode::Digit9 => Some(perro_input::KeyCode::Digit9),
        winit::keyboard::KeyCode::Equal => Some(perro_input::KeyCode::Equal),
        winit::keyboard::KeyCode::IntlBackslash => Some(perro_input::KeyCode::IntlBackslash),
        winit::keyboard::KeyCode::IntlRo => Some(perro_input::KeyCode::IntlRo),
        winit::keyboard::KeyCode::IntlYen => Some(perro_input::KeyCode::IntlYen),
        winit::keyboard::KeyCode::KeyA => Some(perro_input::KeyCode::KeyA),
        winit::keyboard::KeyCode::KeyB => Some(perro_input::KeyCode::KeyB),
        winit::keyboard::KeyCode::KeyC => Some(perro_input::KeyCode::KeyC),
        winit::keyboard::KeyCode::KeyD => Some(perro_input::KeyCode::KeyD),
        winit::keyboard::KeyCode::KeyE => Some(perro_input::KeyCode::KeyE),
        winit::keyboard::KeyCode::KeyF => Some(perro_input::KeyCode::KeyF),
        winit::keyboard::KeyCode::KeyG => Some(perro_input::KeyCode::KeyG),
        winit::keyboard::KeyCode::KeyH => Some(perro_input::KeyCode::KeyH),
        winit::keyboard::KeyCode::KeyI => Some(perro_input::KeyCode::KeyI),
        winit::keyboard::KeyCode::KeyJ => Some(perro_input::KeyCode::KeyJ),
        winit::keyboard::KeyCode::KeyK => Some(perro_input::KeyCode::KeyK),
        winit::keyboard::KeyCode::KeyL => Some(perro_input::KeyCode::KeyL),
        winit::keyboard::KeyCode::KeyM => Some(perro_input::KeyCode::KeyM),
        winit::keyboard::KeyCode::KeyN => Some(perro_input::KeyCode::KeyN),
        winit::keyboard::KeyCode::KeyO => Some(perro_input::KeyCode::KeyO),
        winit::keyboard::KeyCode::KeyP => Some(perro_input::KeyCode::KeyP),
        winit::keyboard::KeyCode::KeyQ => Some(perro_input::KeyCode::KeyQ),
        winit::keyboard::KeyCode::KeyR => Some(perro_input::KeyCode::KeyR),
        winit::keyboard::KeyCode::KeyS => Some(perro_input::KeyCode::KeyS),
        winit::keyboard::KeyCode::KeyT => Some(perro_input::KeyCode::KeyT),
        winit::keyboard::KeyCode::KeyU => Some(perro_input::KeyCode::KeyU),
        winit::keyboard::KeyCode::KeyV => Some(perro_input::KeyCode::KeyV),
        winit::keyboard::KeyCode::KeyW => Some(perro_input::KeyCode::KeyW),
        winit::keyboard::KeyCode::KeyX => Some(perro_input::KeyCode::KeyX),
        winit::keyboard::KeyCode::KeyY => Some(perro_input::KeyCode::KeyY),
        winit::keyboard::KeyCode::KeyZ => Some(perro_input::KeyCode::KeyZ),
        winit::keyboard::KeyCode::Minus => Some(perro_input::KeyCode::Minus),
        winit::keyboard::KeyCode::Period => Some(perro_input::KeyCode::Period),
        winit::keyboard::KeyCode::Quote => Some(perro_input::KeyCode::Quote),
        winit::keyboard::KeyCode::Semicolon => Some(perro_input::KeyCode::Semicolon),
        winit::keyboard::KeyCode::Slash => Some(perro_input::KeyCode::Slash),
        winit::keyboard::KeyCode::AltLeft => Some(perro_input::KeyCode::AltLeft),
        winit::keyboard::KeyCode::AltRight => Some(perro_input::KeyCode::AltRight),
        winit::keyboard::KeyCode::Backspace => Some(perro_input::KeyCode::Backspace),
        winit::keyboard::KeyCode::CapsLock => Some(perro_input::KeyCode::CapsLock),
        winit::keyboard::KeyCode::ContextMenu => Some(perro_input::KeyCode::ContextMenu),
        winit::keyboard::KeyCode::ControlLeft => Some(perro_input::KeyCode::ControlLeft),
        winit::keyboard::KeyCode::ControlRight => Some(perro_input::KeyCode::ControlRight),
        winit::keyboard::KeyCode::Enter => Some(perro_input::KeyCode::Enter),
        winit::keyboard::KeyCode::SuperLeft => Some(perro_input::KeyCode::SuperLeft),
        winit::keyboard::KeyCode::SuperRight => Some(perro_input::KeyCode::SuperRight),
        winit::keyboard::KeyCode::ShiftLeft => Some(perro_input::KeyCode::ShiftLeft),
        winit::keyboard::KeyCode::ShiftRight => Some(perro_input::KeyCode::ShiftRight),
        winit::keyboard::KeyCode::Space => Some(perro_input::KeyCode::Space),
        winit::keyboard::KeyCode::Tab => Some(perro_input::KeyCode::Tab),
        winit::keyboard::KeyCode::Convert => Some(perro_input::KeyCode::Convert),
        winit::keyboard::KeyCode::KanaMode => Some(perro_input::KeyCode::KanaMode),
        winit::keyboard::KeyCode::Lang1 => Some(perro_input::KeyCode::Lang1),
        winit::keyboard::KeyCode::Lang2 => Some(perro_input::KeyCode::Lang2),
        winit::keyboard::KeyCode::Lang3 => Some(perro_input::KeyCode::Lang3),
        winit::keyboard::KeyCode::Lang4 => Some(perro_input::KeyCode::Lang4),
        winit::keyboard::KeyCode::Lang5 => Some(perro_input::KeyCode::Lang5),
        winit::keyboard::KeyCode::NonConvert => Some(perro_input::KeyCode::NonConvert),
        winit::keyboard::KeyCode::Delete => Some(perro_input::KeyCode::Delete),
        winit::keyboard::KeyCode::End => Some(perro_input::KeyCode::End),
        winit::keyboard::KeyCode::Help => Some(perro_input::KeyCode::Help),
        winit::keyboard::KeyCode::Home => Some(perro_input::KeyCode::Home),
        winit::keyboard::KeyCode::Insert => Some(perro_input::KeyCode::Insert),
        winit::keyboard::KeyCode::PageDown => Some(perro_input::KeyCode::PageDown),
        winit::keyboard::KeyCode::PageUp => Some(perro_input::KeyCode::PageUp),
        winit::keyboard::KeyCode::ArrowDown => Some(perro_input::KeyCode::ArrowDown),
        winit::keyboard::KeyCode::ArrowLeft => Some(perro_input::KeyCode::ArrowLeft),
        winit::keyboard::KeyCode::ArrowRight => Some(perro_input::KeyCode::ArrowRight),
        winit::keyboard::KeyCode::ArrowUp => Some(perro_input::KeyCode::ArrowUp),
        winit::keyboard::KeyCode::NumLock => Some(perro_input::KeyCode::NumLock),
        winit::keyboard::KeyCode::Numpad0 => Some(perro_input::KeyCode::Numpad0),
        winit::keyboard::KeyCode::Numpad1 => Some(perro_input::KeyCode::Numpad1),
        winit::keyboard::KeyCode::Numpad2 => Some(perro_input::KeyCode::Numpad2),
        winit::keyboard::KeyCode::Numpad3 => Some(perro_input::KeyCode::Numpad3),
        winit::keyboard::KeyCode::Numpad4 => Some(perro_input::KeyCode::Numpad4),
        winit::keyboard::KeyCode::Numpad5 => Some(perro_input::KeyCode::Numpad5),
        winit::keyboard::KeyCode::Numpad6 => Some(perro_input::KeyCode::Numpad6),
        winit::keyboard::KeyCode::Numpad7 => Some(perro_input::KeyCode::Numpad7),
        winit::keyboard::KeyCode::Numpad8 => Some(perro_input::KeyCode::Numpad8),
        winit::keyboard::KeyCode::Numpad9 => Some(perro_input::KeyCode::Numpad9),
        winit::keyboard::KeyCode::NumpadAdd => Some(perro_input::KeyCode::NumpadAdd),
        winit::keyboard::KeyCode::NumpadBackspace => Some(perro_input::KeyCode::NumpadBackspace),
        winit::keyboard::KeyCode::NumpadClear => Some(perro_input::KeyCode::NumpadClear),
        winit::keyboard::KeyCode::NumpadClearEntry => Some(perro_input::KeyCode::NumpadClearEntry),
        winit::keyboard::KeyCode::NumpadComma => Some(perro_input::KeyCode::NumpadComma),
        winit::keyboard::KeyCode::NumpadDecimal => Some(perro_input::KeyCode::NumpadDecimal),
        winit::keyboard::KeyCode::NumpadDivide => Some(perro_input::KeyCode::NumpadDivide),
        winit::keyboard::KeyCode::NumpadEnter => Some(perro_input::KeyCode::NumpadEnter),
        winit::keyboard::KeyCode::NumpadEqual => Some(perro_input::KeyCode::NumpadEqual),
        winit::keyboard::KeyCode::NumpadHash => Some(perro_input::KeyCode::NumpadHash),
        winit::keyboard::KeyCode::NumpadMemoryAdd => Some(perro_input::KeyCode::NumpadMemoryAdd),
        winit::keyboard::KeyCode::NumpadMemoryClear => {
            Some(perro_input::KeyCode::NumpadMemoryClear)
        }
        winit::keyboard::KeyCode::NumpadMemoryRecall => {
            Some(perro_input::KeyCode::NumpadMemoryRecall)
        }
        winit::keyboard::KeyCode::NumpadMemoryStore => {
            Some(perro_input::KeyCode::NumpadMemoryStore)
        }
        winit::keyboard::KeyCode::NumpadMemorySubtract => {
            Some(perro_input::KeyCode::NumpadMemorySubtract)
        }
        winit::keyboard::KeyCode::NumpadMultiply => Some(perro_input::KeyCode::NumpadMultiply),
        winit::keyboard::KeyCode::NumpadParenLeft => Some(perro_input::KeyCode::NumpadParenLeft),
        winit::keyboard::KeyCode::NumpadParenRight => Some(perro_input::KeyCode::NumpadParenRight),
        winit::keyboard::KeyCode::NumpadStar => Some(perro_input::KeyCode::NumpadStar),
        winit::keyboard::KeyCode::NumpadSubtract => Some(perro_input::KeyCode::NumpadSubtract),
        winit::keyboard::KeyCode::Escape => Some(perro_input::KeyCode::Escape),
        winit::keyboard::KeyCode::Fn => Some(perro_input::KeyCode::Fn),
        winit::keyboard::KeyCode::FnLock => Some(perro_input::KeyCode::FnLock),
        winit::keyboard::KeyCode::PrintScreen => Some(perro_input::KeyCode::PrintScreen),
        winit::keyboard::KeyCode::ScrollLock => Some(perro_input::KeyCode::ScrollLock),
        winit::keyboard::KeyCode::Pause => Some(perro_input::KeyCode::Pause),
        winit::keyboard::KeyCode::BrowserBack => Some(perro_input::KeyCode::BrowserBack),
        winit::keyboard::KeyCode::BrowserFavorites => Some(perro_input::KeyCode::BrowserFavorites),
        winit::keyboard::KeyCode::BrowserForward => Some(perro_input::KeyCode::BrowserForward),
        winit::keyboard::KeyCode::BrowserHome => Some(perro_input::KeyCode::BrowserHome),
        winit::keyboard::KeyCode::BrowserRefresh => Some(perro_input::KeyCode::BrowserRefresh),
        winit::keyboard::KeyCode::BrowserSearch => Some(perro_input::KeyCode::BrowserSearch),
        winit::keyboard::KeyCode::BrowserStop => Some(perro_input::KeyCode::BrowserStop),
        winit::keyboard::KeyCode::Eject => Some(perro_input::KeyCode::Eject),
        winit::keyboard::KeyCode::LaunchApp1 => Some(perro_input::KeyCode::LaunchApp1),
        winit::keyboard::KeyCode::LaunchApp2 => Some(perro_input::KeyCode::LaunchApp2),
        winit::keyboard::KeyCode::LaunchMail => Some(perro_input::KeyCode::LaunchMail),
        winit::keyboard::KeyCode::MediaPlayPause => Some(perro_input::KeyCode::MediaPlayPause),
        winit::keyboard::KeyCode::MediaSelect => Some(perro_input::KeyCode::MediaSelect),
        winit::keyboard::KeyCode::MediaStop => Some(perro_input::KeyCode::MediaStop),
        winit::keyboard::KeyCode::MediaTrackNext => Some(perro_input::KeyCode::MediaTrackNext),
        winit::keyboard::KeyCode::MediaTrackPrevious => {
            Some(perro_input::KeyCode::MediaTrackPrevious)
        }
        winit::keyboard::KeyCode::Power => Some(perro_input::KeyCode::Power),
        winit::keyboard::KeyCode::Sleep => Some(perro_input::KeyCode::Sleep),
        winit::keyboard::KeyCode::AudioVolumeDown => Some(perro_input::KeyCode::AudioVolumeDown),
        winit::keyboard::KeyCode::AudioVolumeMute => Some(perro_input::KeyCode::AudioVolumeMute),
        winit::keyboard::KeyCode::AudioVolumeUp => Some(perro_input::KeyCode::AudioVolumeUp),
        winit::keyboard::KeyCode::WakeUp => Some(perro_input::KeyCode::WakeUp),
        winit::keyboard::KeyCode::Meta => Some(perro_input::KeyCode::Meta),
        winit::keyboard::KeyCode::Hyper => Some(perro_input::KeyCode::Hyper),
        winit::keyboard::KeyCode::Turbo => Some(perro_input::KeyCode::Turbo),
        winit::keyboard::KeyCode::Abort => Some(perro_input::KeyCode::Abort),
        winit::keyboard::KeyCode::Resume => Some(perro_input::KeyCode::Resume),
        winit::keyboard::KeyCode::Suspend => Some(perro_input::KeyCode::Suspend),
        winit::keyboard::KeyCode::Again => Some(perro_input::KeyCode::Again),
        winit::keyboard::KeyCode::Copy => Some(perro_input::KeyCode::Copy),
        winit::keyboard::KeyCode::Cut => Some(perro_input::KeyCode::Cut),
        winit::keyboard::KeyCode::Find => Some(perro_input::KeyCode::Find),
        winit::keyboard::KeyCode::Open => Some(perro_input::KeyCode::Open),
        winit::keyboard::KeyCode::Paste => Some(perro_input::KeyCode::Paste),
        winit::keyboard::KeyCode::Props => Some(perro_input::KeyCode::Props),
        winit::keyboard::KeyCode::Select => Some(perro_input::KeyCode::Select),
        winit::keyboard::KeyCode::Undo => Some(perro_input::KeyCode::Undo),
        winit::keyboard::KeyCode::Hiragana => Some(perro_input::KeyCode::Hiragana),
        winit::keyboard::KeyCode::Katakana => Some(perro_input::KeyCode::Katakana),
        winit::keyboard::KeyCode::F1 => Some(perro_input::KeyCode::F1),
        winit::keyboard::KeyCode::F2 => Some(perro_input::KeyCode::F2),
        winit::keyboard::KeyCode::F3 => Some(perro_input::KeyCode::F3),
        winit::keyboard::KeyCode::F4 => Some(perro_input::KeyCode::F4),
        winit::keyboard::KeyCode::F5 => Some(perro_input::KeyCode::F5),
        winit::keyboard::KeyCode::F6 => Some(perro_input::KeyCode::F6),
        winit::keyboard::KeyCode::F7 => Some(perro_input::KeyCode::F7),
        winit::keyboard::KeyCode::F8 => Some(perro_input::KeyCode::F8),
        winit::keyboard::KeyCode::F9 => Some(perro_input::KeyCode::F9),
        winit::keyboard::KeyCode::F10 => Some(perro_input::KeyCode::F10),
        winit::keyboard::KeyCode::F11 => Some(perro_input::KeyCode::F11),
        winit::keyboard::KeyCode::F12 => Some(perro_input::KeyCode::F12),
        winit::keyboard::KeyCode::F13 => Some(perro_input::KeyCode::F13),
        winit::keyboard::KeyCode::F14 => Some(perro_input::KeyCode::F14),
        winit::keyboard::KeyCode::F15 => Some(perro_input::KeyCode::F15),
        winit::keyboard::KeyCode::F16 => Some(perro_input::KeyCode::F16),
        winit::keyboard::KeyCode::F17 => Some(perro_input::KeyCode::F17),
        winit::keyboard::KeyCode::F18 => Some(perro_input::KeyCode::F18),
        winit::keyboard::KeyCode::F19 => Some(perro_input::KeyCode::F19),
        winit::keyboard::KeyCode::F20 => Some(perro_input::KeyCode::F20),
        winit::keyboard::KeyCode::F21 => Some(perro_input::KeyCode::F21),
        winit::keyboard::KeyCode::F22 => Some(perro_input::KeyCode::F22),
        winit::keyboard::KeyCode::F23 => Some(perro_input::KeyCode::F23),
        winit::keyboard::KeyCode::F24 => Some(perro_input::KeyCode::F24),
        winit::keyboard::KeyCode::F25 => Some(perro_input::KeyCode::F25),
        winit::keyboard::KeyCode::F26 => Some(perro_input::KeyCode::F26),
        winit::keyboard::KeyCode::F27 => Some(perro_input::KeyCode::F27),
        winit::keyboard::KeyCode::F28 => Some(perro_input::KeyCode::F28),
        winit::keyboard::KeyCode::F29 => Some(perro_input::KeyCode::F29),
        winit::keyboard::KeyCode::F30 => Some(perro_input::KeyCode::F30),
        winit::keyboard::KeyCode::F31 => Some(perro_input::KeyCode::F31),
        winit::keyboard::KeyCode::F32 => Some(perro_input::KeyCode::F32),
        winit::keyboard::KeyCode::F33 => Some(perro_input::KeyCode::F33),
        winit::keyboard::KeyCode::F34 => Some(perro_input::KeyCode::F34),
        winit::keyboard::KeyCode::F35 => Some(perro_input::KeyCode::F35),

        _ => None,
    }
}

fn map_winit_mouse_button(button: WinitMouseButton) -> Option<perro_input::MouseButton> {
    match button {
        WinitMouseButton::Left => Some(perro_input::MouseButton::Left),
        WinitMouseButton::Right => Some(perro_input::MouseButton::Right),
        WinitMouseButton::Middle => Some(perro_input::MouseButton::Middle),
        WinitMouseButton::Back => Some(perro_input::MouseButton::Back),
        WinitMouseButton::Forward => Some(perro_input::MouseButton::Forward),
        _ => None,
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
