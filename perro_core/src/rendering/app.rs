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
const DEFAULT_TARGET_FPS: f32 = 500.0;

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


    // Check static textures first (runtime mode)
    let img = if let Some(static_textures) = get_static_textures() {
        if let Some(static_data) = static_textures.get(path) {
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

    // Unified update/render loop timing
    start_time: std::time::Instant,

    // Unified FPS tracking (update and render are unified)
    frames: u32,
    measurement_start: std::time::Instant,
    total_update_duration: std::time::Duration, // Accumulated update time for averaging

    // Update pacing (limits updates/renders to FPS cap)
    fps_cap: f32,
    last_update_time: Option<std::time::Instant>,

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
        fps_cap: f32,
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

            // Unified update/render loop timing
            start_time: now,

            // Unified FPS tracking
            frames: 0,
            measurement_start: now,
            total_update_duration: std::time::Duration::ZERO,

            // Update pacing (capped)
            fps_cap,
            last_update_time: None,

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
                AppCommand::SetFpsCap(fps) => {
                    self.fps_cap = fps;
                    println!("FPS cap changed to: {}", fps);
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

    fn update_fps_measurement(&mut self, now: std::time::Instant, update_duration: std::time::Duration) {
        // Accumulate update duration for averaging
        self.total_update_duration += update_duration;
        
        let measurement_interval = (now - self.measurement_start).as_secs_f32();
        if measurement_interval >= FPS_MEASUREMENT_INTERVAL {
            let actual_fps = self.frames as f32 / measurement_interval;
            let avg_update_ms = self.total_update_duration.as_secs_f64() * 1000.0 / self.frames as f64;
            let current_update_ms = update_duration.as_secs_f64() * 1000.0;
            
            println!("FPS: {:.1} | Update: {:.2}ms (avg: {:.2}ms)", actual_fps, current_update_ms, avg_update_ms);

            self.frames = 0;
            self.total_update_duration = std::time::Duration::ZERO;
            self.measurement_start = now;
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

        let update_start = std::time::Instant::now();

        // 1. Process app commands
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "process_commands").entered();
            self.process_commands(&gfx);
        }

        // 2. UNIFIED UPDATE/RENDER - scene.update() now handles both updating and rendering
        if let Some(scene) = self.game_scene.as_mut() {
            // OPTIMIZED: Only reset scroll delta if input manager exists (avoids Option check overhead)
            if let Some(input_mgr) = scene.get_input_manager() {
                // OPTIMIZED: Use try_lock() to avoid blocking (very rare case where it would block)
                if let Ok(mut input_mgr) = input_mgr.try_lock() {
                    input_mgr.reset_scroll_delta();
                }
            }

            {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "scene_update").entered();
                scene.update(&mut gfx, update_start);
            }
        }

        // 3. RENDER FRAME (scene.update() queues rendering, we execute it here)
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "render_frame").entered();
            self.render_frame(&mut gfx, update_start);
        }

        // 4. Measure frame time and request next redraw immediately
        let frame_end = std::time::Instant::now();
        let frame_duration = frame_end.duration_since(update_start);
        
        // Always request redraw immediately (don't wait)
        gfx.window().request_redraw();
        
        // 5. SLEEP-BASED PACING - cap at fps_cap
        // Only apply compensation if we're running fast enough (update finished early)
        // If update takes longer than target, don't compensate - just run at that speed
        const COMPENSATION_FACTOR: f32 = 1.02; // 2% compensation for event loop overhead
        let base_target_frame_time = std::time::Duration::from_secs_f32(1.0 / self.fps_cap);
        let target_frame_time = if frame_duration < base_target_frame_time {
            // We finished early - apply fixed compensation to account for event loop overhead
            std::time::Duration::from_secs_f32(1.0 / (self.fps_cap * COMPENSATION_FACTOR))
        } else {
            // We're running slow - no compensation, just use base target
            base_target_frame_time
        };
        
        if frame_duration < target_frame_time {
            // Sleep for the remainder if we finished early
            let sleep_duration = target_frame_time - frame_duration;
            
            // Always use spin-wait for frame pacing (more precise, especially on Windows)
            // Windows thread::sleep has poor precision (~15ms minimum), so spin-wait is necessary
            // for accurate high FPS. This uses a bit more CPU but ensures precise timing.
            let sleep_end = frame_end + sleep_duration;
            while std::time::Instant::now() < sleep_end {
                std::hint::spin_loop();
            }
        }
        // If we took longer than target_frame_time, just continue (no catch-up)

        // 6. Track FPS
        self.frames += 1;
        self.update_fps_measurement(frame_end, frame_duration);
        self.last_update_time = Some(frame_end);

        // OPTIMIZED: Use put_graphics() helper
        self.state.put_graphics(gfx);
    }

    /// Render a single frame (scene.update() already queued rendering commands)
    fn render_frame(&mut self, gfx: &mut Graphics, _now: std::time::Instant) {
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
            // Do initial update (unified update/render)
            let now = std::time::Instant::now();
            scene.update(&mut graphics, now);

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
            self.frames = 1;
            self.last_update_time = Some(now);
        }

        // Now make window visible with content already rendered
        graphics.window().set_visible(true);
        graphics.window().request_redraw();

        // Initialize timing systems
        let now = std::time::Instant::now();
        self.start_time = now;
        self.measurement_start = now;

        self.state = State { graphics: Some(graphics) };
    }
}