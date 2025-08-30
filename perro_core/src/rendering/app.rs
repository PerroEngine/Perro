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

// Pre-computed constants
const RENDER_PASS_LABEL: &'static str = "Main Pass";
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
    last_frame: std::time::Instant,
    last_render: std::time::Instant,
    fps_timer: f32,
    fps_accumulator: f32,
    fps_frames: u32,
    max_fps: f32,
    cached_operations: wgpu::Operations<wgpu::Color>,
}

impl<P: ScriptProvider> App<P> {
    pub fn new(
        event_loop: &EventLoop<Graphics>,
        window_title: String,
        game_scene: Option<Scene<P>>,
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
            last_frame: now,
            last_render: now,
            fps_timer: 0.0,
            fps_accumulator: 0.0,
            fps_frames: 0,
            max_fps: 144.0,
            cached_operations,
        }
    }

    #[inline(always)]
    fn process_game(&mut self) {
        if let State::Ready(gfx) = &mut self.state {
            let now = std::time::Instant::now();
            let dt = (now - self.last_frame).as_secs_f32();
            self.last_frame = now;

            // --- Rolling average FPS ---
            self.fps_timer += dt;
            self.fps_accumulator += dt;
            self.fps_frames += 1;

            if self.fps_timer >= 2.0 {
                let avg_dt = self.fps_accumulator / self.fps_frames as f32;
                println!("average lps: {:.1}", 1.0 / avg_dt);

                self.fps_timer = 0.0;
                self.fps_accumulator = 0.0;
                self.fps_frames = 0;
            }

            // --- Scene process and internal script updates always runs at full speed ---
            if let Some(scene) = self.game_scene.as_mut() {
                scene.process(gfx, dt);
            }

            // --- Render capped ---
            let min_frame_time = 1.0 / self.max_fps;
            let since_last_render = (now - self.last_render).as_secs_f32();
            if since_last_render >= min_frame_time {
                self.last_render = now;

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
            }

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
                    .with_title(&self.window_title);

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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, graphics: Graphics) {
        self.state = State::Ready(graphics);
    }
}
