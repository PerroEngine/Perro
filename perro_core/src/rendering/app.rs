use core::f32;
use std::process;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
use std::sync::mpsc::Receiver;

#[cfg(not(target_arch = "wasm32"))]
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

use crate::{
    graphics::Graphics,
    scene::Scene,
    script::ScriptProvider,
    scripting::{
        app_command::{AppCommand, CursorIcon},
        script::SceneAccess,
    },
};

// Graphics are always created synchronously before App initialization
// This allows us to render the first frame before showing the window (no flash)
struct State {
    graphics: Option<Graphics>,
}

impl State {
    #[inline(always)]
    fn take_graphics(&mut self) -> Option<Graphics> {
        self.graphics.take()
    }
    
    #[inline(always)]
    fn put_graphics(&mut self, gfx: Graphics) {
        self.graphics = Some(gfx);
    }
}

const RENDER_PASS_LABEL: &str = "Main Pass";
const CLEAR_COLOR: wgpu::Color = wgpu::Color::BLACK;

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
const WINDOW_CANDIDATES: [PhysicalSize<u32>; 5] = [
    PhysicalSize::new(640, 360),
    PhysicalSize::new(1280, 720),
    PhysicalSize::new(1600, 900),
    PhysicalSize::new(1920, 1080),
    PhysicalSize::new(2560, 1440),
];

#[allow(dead_code)]
const MONITOR_SCALE_FACTOR: f32 = 0.75;
const FPS_MEASUREMENT_INTERVAL: f32 = 3.0;
const MAX_FRAME_DEBT: f32 = 0.025; // 25ms worth of frames
const MAX_CATCHUP_FPS: f32 = 10.0; // Maximum FPS above target for catch-up

// Default Perro icon embedded at compile time
const DEFAULT_ICON_BYTES: &[u8] = include_bytes!("../resources/default-icon.png");

#[cfg(not(target_arch = "wasm32"))]
pub fn load_default_icon() -> Option<winit::window::Icon> {
    use image::imageops::FilterType;
    use winit::window::Icon;

    if DEFAULT_ICON_BYTES.is_empty() {
        eprintln!("⚠ Default icon bytes are empty");
        return None;
    }

    match image::load_from_memory(DEFAULT_ICON_BYTES) {
        Ok(img) => {
            println!("✅ Loading default Perro icon (embedded)");
            let target_size = 32;
            let resized = img.resize_exact(target_size, target_size, FilterType::Lanczos3);
            let rgba = resized.into_rgba8();
            let (width, height) = rgba.dimensions();
            Icon::from_rgba(rgba.into_raw(), width, height).ok()
        }
        Err(e) => {
            eprintln!("❌ Failed to decode default icon: {}", e);
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_icon(path: &str) -> Option<winit::window::Icon> {
    use crate::asset_io::load_asset;
    use crate::runtime::get_static_textures;
    use image::ImageBuffer;
    use image::imageops::FilterType;
    use winit::window::Icon;

    println!("🔎 Loading icon from {path}");

    // Check static textures first (runtime mode)
    let img = if let Some(static_textures) = get_static_textures() {
        if let Some(static_data) = static_textures.get(path) {
            println!(
                "✅ Loading icon from static texture: {} ({}x{})",
                path, static_data.width, static_data.height
            );
            // Convert pre-decoded RGBA8 bytes to DynamicImage
            ImageBuffer::from_raw(
                static_data.width,
                static_data.height,
                static_data.rgba8_bytes.to_vec(),
            )
            .map(image::DynamicImage::ImageRgba8)
            .ok_or_else(|| "Failed to create image from static texture data".to_string())
        } else {
            // Not in static textures, load from disk/BRK
            match load_asset(path) {
                Ok(bytes) => image::load_from_memory(&bytes)
                    .map_err(|e| format!("Failed to decode image: {}", e)),
                Err(e) => Err(format!("Failed to load asset: {}", e)),
            }
        }
    } else {
        // Dev mode: no static textures, load from disk/BRK
        match load_asset(path) {
            Ok(bytes) => image::load_from_memory(&bytes)
                .map_err(|e| format!("Failed to decode image: {}", e)),
            Err(e) => Err(format!("Failed to load asset: {}", e)),
        }
    };

    match img {
        Ok(img) => {
            println!("✅ Successfully decoded {path} as icon");

            let target_size = 32;
            let resized = img.resize_exact(target_size, target_size, FilterType::Lanczos3);

            let rgba = resized.into_rgba8();
            let (width, height) = rgba.dimensions();
            Icon::from_rgba(rgba.into_raw(), width, height).ok()
        }
        Err(err) => {
            eprintln!("❌ Failed to load/decode icon {path}: {err}");
            eprintln!("⚠ Falling back to default Perro icon");
            load_default_icon()
        }
    }
}

pub struct App<P: ScriptProvider> {
    state: State,
    window_title: String,
    #[allow(dead_code)]
    window_icon_path: Option<String>,
    game_scene: Option<Scene<P>>,

    // Render loop timing (capped to target FPS)
    start_time: std::time::Instant,

    // FPS tracking
    fps_frames: u32,
    fps_measurement_start: std::time::Instant,

    // UPS tracking
    ups_updates: u32,
    ups_measurement_start: std::time::Instant,

    // Frame pacing (limits rendering to target FPS)
    target_fps: f32,
    frame_debt: f32,
    last_frame_time: Option<std::time::Instant>,
    total_frames_rendered: u64,
            first_frame: bool,

            // Cached render state
    cached_operations: wgpu::Operations<wgpu::Color>,

    // Command receiver
    command_rx: Receiver<AppCommand>,
}

impl<P: ScriptProvider> App<P> {
    /// Create a new App with pre-created Graphics
    /// Graphics must be created synchronously before calling this, so we can render
    /// the first frame before showing the window (prevents black/white flash)
    pub fn new(
        _event_loop: &EventLoop<Graphics>,
        window_title: String,
        icon_path: Option<String>,
        mut game_scene: Option<Scene<P>>,
        target_fps: f32,
        graphics: Graphics,
    ) -> Self {
        let now = std::time::Instant::now();

        // Create command channel
        use crate::scripting::app_command::create_command_channel;
        let (tx, rx) = create_command_channel();

        // Give the scene the sender
        if let Some(scene) = &mut game_scene {
            scene.app_command_tx = Some(tx);
        }

        Self {
            state: State { graphics: Some(graphics) },
            window_title,
            window_icon_path: icon_path,
            game_scene,

            // Render loop timing
            start_time: now,

            // FPS tracking
            fps_frames: 0,
            fps_measurement_start: now,

            // UPS tracking
            ups_updates: 0,
            ups_measurement_start: now,

            // Frame pacing (capped)
            target_fps,
            frame_debt: 0.0,
            last_frame_time: None,
            total_frames_rendered: 0,
            first_frame: true,

            // Cached render state
            cached_operations: wgpu::Operations {
                load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                store: wgpu::StoreOp::Store,
            },

            command_rx: rx,
        }
    }

    fn process_commands(&mut self, gfx: &Graphics) {
        // OPTIMIZED: Use try_iter() directly - it's already optimized and doesn't allocate
        // Only process if there are commands (early exit if empty)
        for cmd in self.command_rx.try_iter() {
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
                AppCommand::SetCursorIcon(icon) => {
                    use winit::window::CursorIcon as WinitCursorIcon;
                    let winit_icon = match icon {
                        CursorIcon::Default => WinitCursorIcon::Default,
                        CursorIcon::Hand => WinitCursorIcon::Pointer,
                        CursorIcon::Text => WinitCursorIcon::Text,
                        CursorIcon::NotAllowed => WinitCursorIcon::NotAllowed,
                        CursorIcon::Wait => WinitCursorIcon::Wait,
                        CursorIcon::Crosshair => WinitCursorIcon::Crosshair,
                        CursorIcon::Move => WinitCursorIcon::Move,
                        CursorIcon::ResizeVertical => WinitCursorIcon::NsResize,
                        CursorIcon::ResizeHorizontal => WinitCursorIcon::EwResize,
                        CursorIcon::ResizeDiagonal1 => WinitCursorIcon::NwseResize,
                        CursorIcon::ResizeDiagonal2 => WinitCursorIcon::NeswResize,
                    };
                    gfx.window().set_cursor(winit_icon);
                }
                AppCommand::Quit => {
                    println!("Quit command received");
                    process::exit(0);
                }
            }
        }
    }

    fn calculate_frame_debt(&mut self, now: std::time::Instant) {
        // OPTIMIZED: Use as_secs_f64() then cast to f32 (faster than as_secs_f32() on some platforms)
        // Also cache MAX_FRAME_DEBT * target_fps to avoid multiplication every frame
        let elapsed = (now - self.start_time).as_secs_f64() as f32;
        let target_frames = elapsed * self.target_fps;
        let mut frame_debt = target_frames - (self.total_frames_rendered as f32);
        // OPTIMIZED: Cache max_debt calculation (target_fps rarely changes)
        let max_debt = self.target_fps * MAX_FRAME_DEBT;
        frame_debt = frame_debt.min(max_debt);
        self.frame_debt = frame_debt;
    }

    fn should_render_frame(&self, now: std::time::Instant) -> bool {
        // Frame pacing: only render when we're behind by at least half a frame
        if self.first_frame {
            return true;
        }
        
        // Check if a game process is running - if so, be more lenient with rendering
        // to reduce GPU contention (game will use GPU, so editor should be more aggressive)
        let game_running = self.game_scene.as_ref()
            .map(|scene| {
                let project = scene.project.borrow();
                project.get_runtime_param("runtime_process_running")
                    .map(|s| s == "true")
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        
        // When game is running, use lower threshold and higher catch-up rate to reduce GPU contention
        // Lower threshold means we render more often (less strict)
        // Higher catch-up rate means we can catch up faster when behind
        let frame_debt_threshold = if game_running { 0.1 } else { 0.5 };
        let catchup_fps_boost = if game_running { 20.0 } else { MAX_CATCHUP_FPS };
        
        if self.frame_debt < frame_debt_threshold {
            return false;
        }
        
        // Cap catch-up rate: even if we have debt, don't render faster than max_catchup_fps
        if let Some(last_time) = self.last_frame_time {
            let max_catchup_fps = self.target_fps + catchup_fps_boost;
            let min_frame_interval = 1.0 / max_catchup_fps;
            let elapsed = (now - last_time).as_secs_f64() as f32;
            if elapsed < min_frame_interval {
                return false; // Don't render yet - would exceed max catch-up rate
            }
        }
        
        true
    }

    fn update_fps_measurement(&mut self, now: std::time::Instant) {
        let measurement_interval = (now - self.fps_measurement_start).as_secs_f32();
        if measurement_interval >= FPS_MEASUREMENT_INTERVAL {
            let fps = self.fps_frames as f32 / measurement_interval;
            println!("FPS: {:.1}", fps);

            self.fps_frames = 0;
            self.fps_measurement_start = now;
        }
    }

    fn update_ups_measurement(&mut self, now: std::time::Instant) {
        let measurement_interval = (now - self.ups_measurement_start).as_secs_f32();
        if measurement_interval >= FPS_MEASUREMENT_INTERVAL {
            let ups = self.ups_updates as f32 / measurement_interval;
            println!("UPS: {:.1}", ups);

            self.ups_updates = 0;
            self.ups_measurement_start = now;
        }
    }

    fn process_game(&mut self) {
        #[cfg(feature = "profiling")]
        let _span = tracing::span!(tracing::Level::INFO, "process_game").entered();
        
        // OPTIMIZED: Use take_graphics() helper which is faster than manual mem::replace
        let mut gfx = match self.state.take_graphics() {
            Some(g) => g,
            None => return,
        };

        let now = std::time::Instant::now();

        // 1. Process app commands
        // OPTIMIZED: Only process if there are commands (try_iter is already fast, but early exit helps)
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "process_commands").entered();
            self.process_commands(&gfx);
        }

        // 2. UPDATE LOOP
        if let Some(scene) = self.game_scene.as_mut() {
            // OPTIMIZED: Only reset scroll delta if input manager exists (avoids Option check overhead)
            // Most projects don't use scroll, so this is usually a no-op
            if let Some(input_mgr) = scene.get_input_manager() {
                // OPTIMIZED: Use try_lock() to avoid blocking (very rare case where it would block)
                if let Ok(mut input_mgr) = input_mgr.try_lock() {
                    input_mgr.reset_scroll_delta();
                }
            }

            {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "scene_update").entered();
                // OPTIMIZED: Pass now to avoid duplicate Instant::now() call
                scene.update(&mut gfx, now);
            }
            
            // Track UPS (updates happen every frame, uncapped)
            self.ups_updates += 1;
            {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "update_ups_measurement").entered();
                self.update_ups_measurement(now);
            }
        }

        // 3. CAPPED RENDER LOOP - Frame pacing limits to target FPS
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "calculate_frame_debt").entered();
            self.calculate_frame_debt(now);
        }

        // Only render when frame pacing allows (capped FPS)
        let should_render = {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "should_render_frame").entered();
            self.should_render_frame(now)
        };
        
        if should_render {
            {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "render_frame").entered();
                // OPTIMIZED: Pass now to render_frame to avoid duplicate Instant::now() call
                self.render_frame(&mut gfx, now);
            }

            // Update last frame time for catch-up rate limiting
            self.last_frame_time = Some(now);

            if self.first_frame {
                self.first_frame = false;
            }
            self.total_frames_rendered += 1;
            self.fps_frames += 1;

            {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "update_fps_measurement").entered();
                self.update_fps_measurement(now);
            }
        }

        // 4. Request next frame (this drives the uncapped update loop)
        gfx.window().request_redraw();

        // OPTIMIZED: Use put_graphics() helper
        self.state.put_graphics(gfx);
    }

    /// Render a single frame (only called when frame pacing allows)
    fn render_frame(&mut self, gfx: &mut Graphics, now: std::time::Instant) {
        // Update scene render data (queues rendering commands)
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "scene_render").entered();
            if let Some(scene) = self.game_scene.as_mut() {
                // OPTIMIZED: Pass now to avoid duplicate Instant::now() call
                scene.render(gfx, now);
            }
        }

        // Begin frame
        let (frame, view, mut encoder) = {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "begin_frame").entered();
            gfx.begin_frame()
        };

        // Main render pass WITH DEPTH
        {
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: self.cached_operations,
                depth_slice: None,
            };

            let depth_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &gfx.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0), // Clear to max depth
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(RENDER_PASS_LABEL),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(depth_attachment), // ADD THIS
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Execute all batched draw calls
            {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "gfx_render").entered();
                gfx.render(&mut rpass);
            }
        }
        
        // Render egui UI (after main render pass, before end_frame)
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "egui_render").entered();
            gfx.render_egui(&mut encoder, &view);
        }

        // End frame
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "end_frame").entered();
            gfx.end_frame(frame, encoder);
        }
    }

    fn resized(&mut self, size: PhysicalSize<u32>) {
        match &mut self.state {
            State { graphics: Some(gfx) } => {
                gfx.resize(size);
            }
            _ => {}
        }
    }
}

impl<P: ScriptProvider> ApplicationHandler<Graphics> for App<P> {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Graphics are always created synchronously before App creation
        // No async initialization needed
    }

    #[inline(always)]
    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Handle input events
        if let Some(scene) = self.game_scene.as_mut() {
            use crate::input::manager::MouseButton;
            use crate::structs2d::vector2::Vector2;
            use winit::event::{ElementState, MouseButton as WinitMouseButton};
            
            // Check if a text input element is focused (for text capture)
            let has_focused_text_input = scene.has_focused_text_input();

            if let Some(input_mgr) = scene.get_input_manager() {
                let mut input_mgr = input_mgr.lock().unwrap();

                match &event {
                    WindowEvent::KeyboardInput { event, .. } => {
                        // Handle physical key press/release (always)
                        if let winit::keyboard::PhysicalKey::Code(keycode) = event.physical_key {
                            match event.state {
                                ElementState::Pressed => {
                                    input_mgr.handle_key_press(keycode);
                                }
                                ElementState::Released => {
                                    input_mgr.handle_key_release(keycode);
                                }
                            }
                        }
                        
                        // Handle text input from logical key ONLY if a text field is focused
                        if has_focused_text_input && event.state == ElementState::Pressed {
                            // Debug: print the actual key to see what space generates
                            println!("[DEBUG] Logical key: {:?}", event.logical_key);
                            
                            match &event.logical_key {
                                winit::keyboard::Key::Character(text) => {
                                    // Handle all printable characters
                                    println!("[DEBUG] Character input: {:?} (len: {})", text, text.len());
                                    input_mgr.handle_text_input(text.to_string());
                                }
                                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) => {
                                    // Space might be a Named key
                                    println!("[DEBUG] SPACE key (Named) pressed!");
                                    input_mgr.handle_text_input(" ".to_string());
                                }
                                _ => {
                                    // Debug: show what key was pressed
                                    println!("[DEBUG] Other key: {:?}", event.logical_key);
                                }
                            }
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        let mouse_btn = match button {
                            WinitMouseButton::Left => MouseButton::Left,
                            WinitMouseButton::Right => MouseButton::Right,
                            WinitMouseButton::Middle => MouseButton::Middle,
                            WinitMouseButton::Back => MouseButton::Other(4),
                            WinitMouseButton::Forward => MouseButton::Other(5),
                            WinitMouseButton::Other(id) => MouseButton::Other(*id),
                        };
                        match state {
                            ElementState::Pressed => {
                                input_mgr.handle_mouse_button_press(mouse_btn);
                            }
                            ElementState::Released => {
                                input_mgr.handle_mouse_button_release(mouse_btn);
                            }
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        input_mgr
                            .handle_mouse_move(Vector2::new(position.x as f32, position.y as f32));
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        use winit::event::MouseScrollDelta;
                        let scroll_delta = match delta {
                            MouseScrollDelta::LineDelta(_, y) => *y,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01, // Scale pixel delta
                        };
                        input_mgr.handle_scroll(scroll_delta);
                    }
                    WindowEvent::Ime(ime) => {
                        match ime {
                            winit::event::Ime::Commit(text) => {
                                println!("[DEBUG] IME Commit: {:?}", text);
                                input_mgr.handle_text_input(text.to_string());
                            }
                            winit::event::Ime::Preedit(text, _) => {
                                if !text.is_empty() {
                                    input_mgr.handle_text_input(text.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => self.process_game(), // This drives uncapped updates
            WindowEvent::CloseRequested => process::exit(0),
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut graphics: Graphics) {
        // First clear pass to ensure clean buffer
        {
            let (frame, view, mut encoder) = graphics.begin_frame();
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: self.cached_operations,
                depth_slice: None,
            };

            let depth_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &graphics.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            {
                let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("First Clear Pass"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: Some(depth_attachment), // ADD THIS
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }
            graphics.end_frame(frame, encoder);
        }

        // Render the actual first game frame before showing window
        if let Some(scene) = self.game_scene.as_mut() {
            // Do initial update
            let now = std::time::Instant::now();
            scene.update(&mut graphics, now);

            // Queue rendering
            scene.render(&mut graphics, now);

            // Render the frame
            let (frame, view, mut encoder) = graphics.begin_frame();
            let color_attachment = wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: self.cached_operations,
                depth_slice: None,
            };

            let depth_attachment = wgpu::RenderPassDepthStencilAttachment {
                view: &graphics.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Initial Game Frame"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: Some(depth_attachment), // ADD THIS
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                graphics.render(&mut rpass);
            }
            graphics.end_frame(frame, encoder);

            // Mark that first frame is done
            self.first_frame = false;
            self.total_frames_rendered = 1;
            self.fps_frames = 1;
            self.last_frame_time = Some(now);
        }

        // Now make window visible with content already rendered
        graphics.window().set_visible(true);
        graphics.window().request_redraw();

        // Initialize timing systems
        let now = std::time::Instant::now();
        self.start_time = now;
        self.fps_measurement_start = now;
        self.ups_measurement_start = now;

        self.state = State { graphics: Some(graphics) };
    }
}