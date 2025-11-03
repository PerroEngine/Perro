use core::f32;
use std::process;
use std::sync::mpsc::Receiver; // NEW import
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
    app_command::AppCommand, rendering::{create_graphics, Graphics}, scene::Scene, script::ScriptProvider // NEW import
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

#[cfg(not(target_arch = "wasm32"))]
fn load_icon(path: &str) -> Option<winit::window::Icon> {
    use winit::window::Icon;
    use crate::asset_io::load_asset;
    use image::imageops::FilterType;

    println!("🔎 Loading icon from {path}");

    match load_asset(path) {
        Ok(bytes) => match image::load_from_memory(&bytes) {
            Ok(img) => {
                println!("✅ Successfully decoded {path} as icon");

                // Pick a good OS‑friendly size (Windows likes 32x32)
                let target_size = 32;
                let resized = img.resize_exact(target_size, target_size, FilterType::Lanczos3);

                let rgba = resized.into_rgba8();
                let (width, height) = rgba.dimensions();
                Some(Icon::from_rgba(rgba.into_raw(), width, height).ok()?)
            }
            Err(err) => {
                eprintln!("❌ Failed to decode image {path}: {err}");
                None
            }
        },
        Err(err) => {
            eprintln!("❌ Failed to load asset {path}: {err}");
            None
        }
    }
}

pub struct App<P: ScriptProvider> {
    state: State,
    window_title: String,
    window_icon_path: Option<String>,
    game_scene: Option<Scene<P>>,
    last_update: std::time::Instant,

    // FPS tracking
    fps_frames: u32,
    fps_measurement_start: std::time::Instant,

    // Frame pacing
    target_fps: f32,
    cached_operations: wgpu::Operations<wgpu::Color>,
    first_frame: bool,
    frame_debt: f64,
    total_frames_rendered: u64,
    start_time: std::time::Instant,
    skip_counter: u32,

    // NEW: Command receiver
    command_rx: Receiver<AppCommand>,
}

impl<P: ScriptProvider> App<P> {
    pub fn new(
        event_loop: &EventLoop<Graphics>,
        window_title: String,
        icon_path: Option<String>,
        mut game_scene: Option<Scene<P>>, // Changed to mut
        target_fps: f32,
    ) -> Self {
        let cached_operations = wgpu::Operations {
            load: wgpu::LoadOp::Clear(CLEAR_COLOR),
            store: wgpu::StoreOp::Store,
        };

        let now = std::time::Instant::now();

        // NEW: Create command channel
        use crate::app_command::create_command_channel;
        let (tx, rx) = create_command_channel();

        // NEW: Give the scene the sender
        if let Some(scene) = &mut game_scene {
            scene.app_command_tx = Some(tx);
        }

        Self {
            state: State::Init(Some(event_loop.create_proxy())),
            window_title,
            window_icon_path: icon_path,
            game_scene,
            last_update: now,

            fps_frames: 0,
            fps_measurement_start: now,

            target_fps,
            cached_operations,
            first_frame: true,
            frame_debt: 0.0,
            total_frames_rendered: 0,
            start_time: now,
            skip_counter: 0,

            command_rx: rx, // NEW field
        }
    }

#[inline(always)]
fn process_game(&mut self) {
    if let State::Ready(gfx) = &mut self.state {
        // 1) Poll app commands once per update
        while let Ok(cmd) = self.command_rx.try_recv() {
            match cmd {
                AppCommand::SetWindowTitle(title) => {
                    gfx.window().set_title(&title);
                    self.window_title = title;
                    println!("Window title set to: {}", self.window_title);
                }
                AppCommand::SetTargetFPS(fps) => {
                    self.target_fps = fps;
                    println!("Target FPS changed to: {}", fps);
                }
                AppCommand::Quit => {
                    println!("Quit command received");
                    std::process::exit(0);
                }
            }
        }

        // 2) Time for frame pacing
        let now = std::time::Instant::now();

        self.last_update = now;

        // 3) Update the scene
        if let Some(scene) = self.game_scene.as_mut() {
            scene.update();
        }

        // 4) Frame debt calculation for pacing rendering
        let elapsed = (now - self.start_time).as_secs_f64();
        let target_frames = elapsed * self.target_fps as f64;
        let mut frame_debt = target_frames - (self.total_frames_rendered as f64);
        frame_debt = frame_debt.min(self.target_fps as f64 * 0.025);
        self.frame_debt = frame_debt;

        // 5) Decide if we should render this frame
        let render_frame = self.first_frame || self.frame_debt > -1.0;

        if render_frame {
            self.first_frame = false;
            self.total_frames_rendered += 1;
            self.fps_frames += 1;

            // 6) FPS measurement once per second
            let measurement_interval = (now - self.fps_measurement_start).as_secs_f32();
            if measurement_interval >= 1.0 {
                let fps = self.fps_frames as f32 / measurement_interval;
                println!(
                    "fps: {:.1}",
                    fps
                );

                self.fps_frames = 0;
                self.fps_measurement_start = now;
                self.skip_counter = 0;
            }

            // 7) Render scene
            if let Some(scene) = self.game_scene.as_mut() {
                scene.render(gfx);
            }

            // 8) Draw frame with wgpu
            let (frame, view, mut encoder) = gfx.begin_frame();
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: self.cached_operations,
            };

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(RENDER_PASS_LABEL),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                gfx.draw_instances(&mut rpass);
            }
            gfx.end_frame(frame, encoder);
        } else {
            self.skip_counter += 1;
        }

        // 9) Request redraw after processing game
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
                    let primary_monitor =
                        event_loop.primary_monitor().unwrap();
                    let monitor_size = primary_monitor.size();

                    let target_width =
                        (monitor_size.width as f32 * MONITOR_SCALE_FACTOR) as u32;
                    let target_height =
                        (monitor_size.height as f32 * MONITOR_SCALE_FACTOR) as u32;

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
                    // apply default size
                    attrs = attrs.with_inner_size(default_size);

                    // apply icon if provided
                    if let Some(icon_path) = &self.window_icon_path {
                        println!("Loading window icon from path: {}", icon_path);
                        if let Some(icon) = load_icon(icon_path) {
                            attrs = attrs.with_window_icon(Some(icon));
                        }
                    }
                }

                #[cfg(target_arch = "wasm32")]
                {
                    use winit::platform::web::WindowAttributesExtWebSys;
                    attrs = attrs.with_append(true);
                }

                #[cfg(target_arch = "wasm32")]
                let window = Rc::new(
                    event_loop.create_window(attrs).expect("create window"),
                );

                #[cfg(not(target_arch = "wasm32"))]
                let window = Arc::new(
                    event_loop.create_window(attrs).expect("create window"),
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