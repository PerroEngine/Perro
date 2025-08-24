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

/// Generic App that works with any ScriptProvider
pub struct App<P: ScriptProvider> {
    state: State,
    window_title: String,
    game_scene: Option<Scene<P>>,
    last_frame: std::time::Instant,
}

impl<P: ScriptProvider> App<P> {
    pub fn new(
        event_loop: &EventLoop<Graphics>,
        window_title: String,
        game_scene: Option<Scene<P>>,
    ) -> Self {
        Self {
            state: State::Init(Some(event_loop.create_proxy())),
            window_title,
            game_scene,
            last_frame: std::time::Instant::now(),
        }
    }

    fn process_frame(&mut self) {
        if let State::Ready(gfx) = &mut self.state {
            // compute delta-time
            let now = std::time::Instant::now();
            let dt = (now - self.last_frame).as_secs_f32();
            self.last_frame = now;

            // begin frame
            let (frame, view, mut encoder) = gfx.begin_frame();
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // update & draw
            if let Some(scene) = self.game_scene.as_mut() {
                scene.tick(gfx, &mut rpass, dt);
            }

            // submit
            drop(rpass);
            gfx.end_frame(frame, encoder);

            // schedule next frame
            gfx.window().request_redraw();
        }
    }

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
                // --- Detect monitor size ---
                #[cfg(not(target_arch = "wasm32"))]
                let default_size = {
                    use winit::dpi::PhysicalSize;
                    let primary_monitor = event_loop.primary_monitor().unwrap();
                    let monitor_size = primary_monitor.size();

                    // List of "nice" resolutions
                    let candidates = [
                        PhysicalSize::new(640, 360),
                        PhysicalSize::new(1280, 720),
                        PhysicalSize::new(1600, 900),
                        PhysicalSize::new(1920, 1080),
                        PhysicalSize::new(2560, 1440),
                    ];

                    // Target: about 75% of monitor size
                    let target_width = (monitor_size.width as f32 * 0.75) as u32;
                    let target_height = (monitor_size.height as f32 * 0.75) as u32;

                    *candidates
                        .iter()
                        .min_by_key(|size| {
                            let dw = size.width as i32 - target_width as i32;
                            let dh = size.height as i32 - target_height as i32;
                            (dw * dw + dh * dh) as u32
                        })
                        .unwrap()
                };

                // --- Build window attributes ---
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

                // --- Create window ---
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

                // --- Create graphics ---
                #[cfg(target_arch = "wasm32")]
                wasm_bindgen_futures::spawn_local(create_graphics(window, proxy));

                #[cfg(not(target_arch = "wasm32"))]
                pollster::block_on(create_graphics(window, proxy));
            }
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => self.process_frame(),
            WindowEvent::CloseRequested => process::exit(0),
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, graphics: Graphics) {
        self.state = State::Ready(graphics);
    }
}