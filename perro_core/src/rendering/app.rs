use core::f32;
use std::process;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

use crate::{
    rendering::{create_graphics, Graphics},
    scene::Scene, ScriptProvider,
};

enum State {
    Init(Option<EventLoopProxy<Graphics>>),
    Ready(Graphics),
}

const RENDER_PASS_LABEL: &str = "Main Pass";
const CLEAR_COLOR: wgpu::Color = wgpu::Color::BLACK;
const WINDOW_CANDIDATES: [PhysicalSize<u32>; 5] = [
    PhysicalSize::new(640, 360),
    PhysicalSize::new(1280, 720),
    PhysicalSize::new(1600, 900),
    PhysicalSize::new(1920, 1080),
    PhysicalSize::new(2560, 1440),
];
const MONITOR_SCALE_FACTOR: f32 = 0.75;

pub struct App<P: ScriptProvider> {
    state: State,
    window_title: String,
    game_scene: Option<Scene<P>>,
    last_update: std::time::Instant,

    // FPS/UPS tracking
    fps_frames: u32,
    ups_frames: u32,
    fps_measurement_start: std::time::Instant,

    // Frame pacing
    target_fps: f32,
    cached_operations: wgpu::Operations<wgpu::Color>,
    first_frame: bool,
    frame_debt: f64,
    total_frames_rendered: u64,
    start_time: std::time::Instant,
    skip_counter: u32,
}

impl<P: ScriptProvider> App<P> {
    pub fn new(
        event_loop: &EventLoop<Graphics>,
        window_title: String,
        game_scene: Option<Scene<P>>,
        target_fps: f32
    ) -> Self {
        let cached_operations = wgpu::Operations {
            load: wgpu::LoadOp::Clear(CLEAR_COLOR),
            store: wgpu::StoreOp::Store,
        };

        let now = std::time::Instant::now();

        Self {
            state: State::Init(Some(event_loop.create_proxy())),
            window_title,
            game_scene,
            last_update: now,

            fps_frames: 0,
            ups_frames: 0,
            fps_measurement_start: now,

            target_fps,
            cached_operations,
            first_frame: true,
            frame_debt: 0.0,
            total_frames_rendered: 0,
            start_time: now,
            skip_counter: 0,
        }
    }

    #[inline(always)]
    fn process_game(&mut self) {
        if let State::Ready(gfx) = &mut self.state {
            let now = std::time::Instant::now();
            let dt = (now - self.last_update).as_secs_f32();
            self.last_update = now;

            // --- Scene update (UPS) ---
            if let Some(scene) = self.game_scene.as_mut() {
                scene.update(dt);
                self.ups_frames += 1;
            }

            // --- Frame debt system ---
            let elapsed_time = (now - self.start_time).as_secs_f64();
            let target_frames = elapsed_time * self.target_fps as f64;
            self.frame_debt = target_frames - self.total_frames_rendered as f64;

            // Cap frame debt to prevent excessive catch-up
            self.frame_debt = self.frame_debt.min(self.target_fps as f64 * 0.025); // Max 0.025 seconds of debt

            let should_render = self.first_frame || self.frame_debt > -1.0;

            if should_render {
                self.first_frame = false;
                self.total_frames_rendered += 1;
                self.fps_frames += 1;

                // --- FPS + UPS measurement (once per second) ---
                let measurement_duration = (now - self.fps_measurement_start).as_secs_f32();
                if measurement_duration >= 1.0 {
                    let fps = self.fps_frames as f32 / measurement_duration;
                    let ups = self.ups_frames as f32 / measurement_duration;

                    println!(
                        "fps: {:.1}, ups: {:.1} (debt: {:.2}, skipped: {})",
                        fps, ups, self.frame_debt, self.skip_counter
                    );

                    self.fps_frames = 0;
                    self.ups_frames = 0;
                    self.fps_measurement_start = now;
                    self.skip_counter = 0;
                }

                // --- Render scene ---
                if let Some(scene) = self.game_scene.as_mut() {
                    scene.render(gfx);
                }

                let (frame, view, mut encoder) = gfx.begin_frame();
                let color_attachment = wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: self.cached_operations,
                };

                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(RENDER_PASS_LABEL),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                gfx.draw_instances(&mut rpass);
                drop(rpass);
                gfx.end_frame(frame, encoder);
            } else {
                // Skip frame
                self.skip_counter += 1;
            }

            // Always keep the loop alive
            gfx.window().request_redraw();
        }
    }

    #[inline(always)]
    fn resized(&mut self, size: PhysicalSize<u32>) {
        if let State::Ready(gfx) = &mut self.state {
            gfx.resize(size);
        }
    }
}

impl<P: ScriptProvider> ApplicationHandler<Graphics> for App<P> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let State::Init(proxy_opt) = &mut self.state {
            if let Some(proxy) = proxy_opt.take() {
                #[cfg(not(target_arch = "wasm32"))]
                let default_size = {
                    let primary_monitor = event_loop.primary_monitor().unwrap();
                    let monitor_size = primary_monitor.size();

                    let target_width = (monitor_size.width as f32 * MONITOR_SCALE_FACTOR) as u32;
                    let target_height = (monitor_size.height as f32 * MONITOR_SCALE_FACTOR) as u32;

                    *WINDOW_CANDIDATES
                        .iter()
                        .min_by_key(|size| {
                            let dw = size.width as i32 - target_width as i32;
                            let dh = size.height as i32 - target_height as i32;
                            (dw * dw + dh * dh) as u32
                        })
                        .unwrap()
                };

                let mut attrs = Window::default_attributes()
                    .with_title(&self.window_title)
                    .with_visible(false);

                #[cfg(not(target_arch = "wasm32"))]
                {
                    attrs = attrs.with_inner_size(default_size);
                }

                #[cfg(target_arch = "wasm32")]
                {
                    use winit::platform::web::WindowAttributesExtWebSys;
                    attrs = attrs.with_append(true);
                }

                #[cfg(target_arch = "wasm32")]
                let window = Rc::new(
                    event_loop
                        .create_window(attrs)
                        .expect("create window"),
                );

                #[cfg(not(target_arch = "wasm32"))]
                let window = Arc::new(
                    event_loop
                        .create_window(attrs)
                        .expect("create window"),
                );

                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(create_graphics(window, proxy));

                #[cfg(not(target_arch = "wasm32"))]
                pollster::block_on(create_graphics(window, proxy));
            }
        }
    }

    #[inline(always)]
    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => self.process_game(),
            WindowEvent::CloseRequested => process::exit(0),
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut graphics: Graphics) {
        // --- One-shot first clear ---
        {
            let (frame, view, mut encoder) = graphics.begin_frame();
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: self.cached_operations,
            };
            {
                let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("First Clear Pass"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }
            graphics.end_frame(frame, encoder);

            graphics.window().set_visible(true);
            graphics.window().request_redraw();
        }

        self.state = State::Ready(graphics);
    }
}