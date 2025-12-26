use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::asset_io::set_key;
use crate::asset_io::{ProjectRoot, set_project_root};
use crate::graphics::{Graphics, create_graphics_sync};
use crate::manifest::Project;
use crate::rendering::app::App;
use crate::scene::{Scene, SceneData};
use crate::script::{CreateFn, ScriptProvider};
use crate::structs2d::texture::StaticTextureData;
use crate::ui::fur_ast::FurElement;
use once_cell::sync::Lazy;
use phf::Map;
use std::sync::atomic::{AtomicPtr, Ordering};
use winit::event_loop::EventLoop;
use winit::window::Window;

/// Static assets that are bundled with the binary
pub struct StaticAssets {
    pub project: &'static Project,
    pub scenes: &'static Lazy<HashMap<&'static str, &'static SceneData>>,
    pub fur: &'static Lazy<HashMap<&'static str, &'static [FurElement]>>,
    pub textures: &'static Lazy<HashMap<&'static str, &'static StaticTextureData>>,
}

/// Project-specific data that needs to be passed from the binary
pub struct RuntimeData {
    /// Embedded assets.brk bytes
    pub assets_brk: &'static [u8],
    /// AES encryption key
    pub aes_key: [u8; 32],
    /// Static versions of scenes, ui, etc
    pub static_assets: StaticAssets,
    /// Script constructor registry (compile-time perfect hash map)
    pub script_registry: &'static Map<&'static str, CreateFn>,
}

/// Generic static script provider that lives in core
pub struct StaticScriptProvider {
    ctors: &'static Map<&'static str, CreateFn>,
    pub scenes: &'static Lazy<HashMap<&'static str, &'static SceneData>>,
    pub fur: &'static Lazy<HashMap<&'static str, &'static [FurElement]>>,
}

impl StaticScriptProvider {
    pub fn new(data: &RuntimeData) -> Self {
        Self {
            ctors: data.script_registry,
            scenes: data.static_assets.scenes,
            fur: data.static_assets.fur,
        }
    }
}

// Safety: StaticScriptProvider is Sync because the &'static SceneData references
// are only accessed from the main thread. The RefCell<SceneNode> inside SceneData
// is not shared between threads, so it's safe to mark this as Sync.
unsafe impl Sync for StaticScriptProvider {}

impl ScriptProvider for StaticScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.ctors
            .get(short)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("No static ctor for {short}"))
    }

    fn load_scene_data(&self, path: &str) -> io::Result<SceneData> {
        if let Some(scene) = self.scenes.get(path) {
            Ok((**scene).clone())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Scene not found: {}", path),
            ))
        }
    }

    fn load_fur_data(&self, path: &str) -> io::Result<Vec<FurElement>> {
        if let Some(fur) = self.fur.get(path) {
            Ok((*fur).to_vec())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("FUR not found: {}", path),
            ))
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run_app(event_loop: EventLoop<Graphics>, app: App<StaticScriptProvider>) {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Error).expect("Couldn't initialize logger");

    use winit::platform::web::EventLoopExtWebSys;
    wasm_bindgen_futures::spawn_local(async move {
        event_loop.spawn_app(app);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn run_app(event_loop: EventLoop<Graphics>, mut app: App<StaticScriptProvider>) {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    let _ = event_loop.run_app(&mut app);
}

/// Global static textures map (set once at startup, only in runtime mode)
static STATIC_TEXTURES: AtomicPtr<
    std::collections::HashMap<&'static str, &'static StaticTextureData>,
> = AtomicPtr::new(std::ptr::null_mut());

/// Initialize static textures (called once at startup in runtime mode)
pub fn set_static_textures(
    textures: &'static Lazy<HashMap<&'static str, &'static StaticTextureData>>,
) {
    // Get the inner HashMap reference
    let map_ref = &**textures;
    STATIC_TEXTURES.store(map_ref as *const _ as *mut _, Ordering::Release);
}

/// Get static textures map (returns None if not initialized or in dev mode)
pub fn get_static_textures()
-> Option<&'static std::collections::HashMap<&'static str, &'static StaticTextureData>> {
    let ptr = STATIC_TEXTURES.load(Ordering::Acquire);
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

/// Helper to convert flamegraph folded file to SVG
#[cfg(feature = "profiling")]
pub fn convert_flamegraph(folded_path: &Path, svg_path: &Path) {
    println!("📊 Converting flamegraph to SVG...");
    
    // Try to convert using inferno (Rust library) first
    use inferno::flamegraph;
    use std::fs::File;
    use std::io::{BufReader, BufWriter};
    use std::process::Command;
    
    match File::open(folded_path) {
        Ok(folded_file) => {
            let reader = BufReader::new(folded_file);
            match File::create(svg_path) {
                Ok(svg_file) => {
                    let writer = BufWriter::new(svg_file);
                    let mut options = flamegraph::Options::default();
                    match flamegraph::from_reader(&mut options, reader, writer) {
                        Ok(_) => {
                            println!("✅ Flamegraph saved to {:?}", svg_path);
                            // Remove the folded file after successful conversion
                            let _ = std::fs::remove_file(folded_path);
                            return;
                        }
                        Err(e) => {
                            eprintln!("⚠️  Failed to convert with inferno: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Failed to create SVG file: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to open folded file: {}", e);
        }
    }
    
    // Fallback: Try external flamegraph command
    let output = Command::new("flamegraph")
        .arg(folded_path)
        .arg("--output")
        .arg(svg_path)
        .output();
    
    match output {
        Ok(result) if result.status.success() => {
            println!("✅ Flamegraph saved to {:?}", svg_path);
            // Optionally remove the folded file
            let _ = std::fs::remove_file(folded_path);
        }
        Ok(_) => {
            eprintln!("⚠️  flamegraph command failed. Install with: cargo install flamegraph");
            eprintln!("   Or manually convert: flamegraph {:?} > {:?}", folded_path, svg_path);
        }
        Err(_) => {
            eprintln!("⚠️  flamegraph command not found. Install with: cargo install flamegraph");
            eprintln!("   Or manually convert: flamegraph {:?} > {:?}", folded_path, svg_path);
        }
    }
}

/// Helper to append errors to errors.log
fn log_error(msg: &str) {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(folder) = exe_path.parent() {
            let log_path = folder.join("errors.log");
            if let Ok(mut file) = File::options().append(true).create(true).open(&log_path) {
                let _ = writeln!(file, "{}", msg);
            }
        }
    }
    eprintln!("{}", msg);
}

/// Main entry point for running a Perro game
pub fn run_game(data: RuntimeData) {
    // Name the main thread
    crate::thread_utils::set_current_thread_name("Main");
    
    // Set up a basic panic hook IMMEDIATELY to catch any early panics
    // This will be replaced with a better one later, but at least we'll catch panics
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("❌ PANIC occurred (early hook)!");
        if let Some(location) = panic_info.location() {
            eprintln!("   Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("   Message: {}", s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("   Message: {}", s);
        }
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }));

    let args: Vec<String> = env::args().collect();
    let mut key: Option<String> = None;

    {
        set_key(data.aes_key);
        set_project_root(ProjectRoot::Brk {
            data: data.assets_brk,
            name: data.static_assets.project.name().into(),
        });
    }

    // 2. Clone the static project so we can add runtime params
    let mut project = data.static_assets.project.clone();

    // 3. Parse runtime arguments and add them to project
    for arg in args.iter().skip(1) {
        if arg.starts_with("--") {
            let clean_key = arg.trim_start_matches("--").to_string();
            key = Some(clean_key);
        } else if let Some(k) = key.take() {
            project.set_runtime_param(&k, arg);
        }
    }

    // 4. Initialize static textures (runtime mode only)
    set_static_textures(data.static_assets.textures);

    // 5. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 6. Create window and Graphics before building scene
    #[cfg(not(target_arch = "wasm32"))]
    let default_size = winit::dpi::PhysicalSize::new(1280, 720); // Default size, monitor info not available before window creation

    let title = project.name().to_string();
    let mut window_attrs = Window::default_attributes()
        .with_title(title)
        .with_visible(false);

    #[cfg(not(target_arch = "wasm32"))]
    {
        window_attrs = window_attrs.with_inner_size(default_size);
        if let Some(icon_path) = project.icon() {
            // Load icon if needed (simplified for now)
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowAttributesExtWebSys;
        window_attrs = window_attrs.with_append(true);
    }

    // Note: create_window is deprecated in winit 0.30 in favor of ActiveEventLoop::create_window,
    // but ActiveEventLoop is only available in event handlers. Since we need the window before
    // running the app, we use the deprecated method here.
    #[cfg(target_arch = "wasm32")]
    let window = {
        #[allow(deprecated)]
        std::rc::Rc::new(event_loop.create_window(window_attrs).expect("create window"))
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window = {
        #[allow(deprecated)]
        std::sync::Arc::new(event_loop.create_window(window_attrs).expect("create window"))
    };

    // Create Graphics synchronously
    let mut graphics = create_graphics_sync(window.clone());

    // 7. Build runtime scene using StaticScriptProvider (now with Graphics)
    let provider = StaticScriptProvider::new(&data);

    // Wrap project in Rc<RefCell>
    let project_rc = Rc::new(RefCell::new(project));

    let game_scene = match Scene::from_project_with_provider(project_rc.clone(), provider, &mut graphics) {
        Ok(scene) => scene,
        Err(e) => {
            log_error(&format!("Failed to build game scene: {e}"));
            return;
        }
    };
    window.set_visible(true);

    // Build App with pre-created Graphics
    let app = App::new(
        &event_loop,
        project_rc.borrow().name().to_string(),
        project_rc.borrow().icon(),
        Some(game_scene),
        project_rc.borrow().target_fps(),
        graphics,
    );

    run_app(event_loop, app);
}

/// Development entry point for running projects from disk with DLL hot-reloading
#[cfg(not(target_arch = "wasm32"))]
pub fn run_dev() {
    use crate::registry::DllScriptProvider;

    // Name the main thread
    crate::thread_utils::set_current_thread_name("Main");
    
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    
    let args: Vec<String> = env::args().collect();
    let mut key: Option<String> = None;
    
    // Check for profiling flag
    let enable_profiling = args.contains(&"--profile".to_string()) || args.contains(&"--flamegraph".to_string());
    
    // 1. Determine project root path (disk or exe dir) - need this IMMEDIATELY
    let project_root: PathBuf = if let Some(i) = args.iter().position(|a| a == "--path") {
        let path_arg = &args[i + 1];

        // Get the directory where cargo executed the compiled binary
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().expect("Failed to get working directory"));

        // Step upward to crate workspace root if we're inside target/
        let workspace_root: PathBuf = exe_dir
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| {
                exe_dir
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            });

        // Handle special flags like --editor and --test
        if path_arg.eq_ignore_ascii_case("--editor") {
            let editor_path = workspace_root.join("perro_editor");
            use dunce;
            dunce::canonicalize(&editor_path).unwrap_or(editor_path)
        } else if path_arg.eq_ignore_ascii_case("--test") {
            let test_path = workspace_root.join("test_projects/test");
            use dunce;
            dunce::canonicalize(&test_path).unwrap_or(test_path)
        } else {
            // Handle the input path
            let candidate = PathBuf::from(path_arg);

            // On Windows, paths starting with / are not valid absolute paths
            // Treat them as relative to workspace root instead
            let is_valid_absolute = {
                #[cfg(windows)]
                {
                    if path_arg.starts_with('/') {
                        false // Unix-style path on Windows - treat as relative
                    } else {
                        candidate.is_absolute()
                    }
                }
                #[cfg(not(windows))]
                {
                    candidate.is_absolute()
                }
            };

            if is_valid_absolute {
                candidate
            } else {
                // If it starts with / on Windows, treat as relative to workspace root
                let base_path: PathBuf = {
                    #[cfg(windows)]
                    {
                        if path_arg.starts_with('/') {
                            workspace_root.clone()
                        } else {
                            env::current_dir().expect("Failed to get current dir")
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        env::current_dir().expect("Failed to get current dir")
                    }
                };

                let full_path = if path_arg.starts_with('/') {
                    // Strip leading / and join to base
                    base_path.join(&path_arg[1..])
                } else {
                    base_path.join(&candidate)
                };

                // Try to canonicalize, but if it fails, ensure we have a proper absolute path
                use dunce;
                dunce::canonicalize(&full_path).unwrap_or_else(|_| {
                    if full_path.is_absolute() {
                        full_path
                    } else {
                        env::current_dir()
                            .expect("Failed to get current dir")
                            .join(&full_path)
                            .canonicalize()
                            .unwrap_or_else(|_| {
                                env::current_dir()
                                    .expect("Failed to get current dir")
                                    .join(&full_path)
                            })
                    }
                })
            }
        }
    } else if args.contains(&"--editor".to_string()) {
        // Dev-only: editor project path (relative to workspace root)
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().expect("Failed to get working directory"));

        // Step upward to crate workspace root if we're inside target/
        let workspace_root = exe_dir
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists())
            .unwrap_or_else(|| exe_dir.parent().unwrap_or_else(|| Path::new(".")));

        let editor_path = workspace_root.join("perro_editor");
        dunce::canonicalize(&editor_path).unwrap_or(editor_path)
    } else {
        // Dev mode: default to editor project
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().expect("Failed to get working directory"));

        // Step upward to crate workspace root if we're inside target/
        let workspace_root = exe_dir
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists())
            .unwrap_or_else(|| exe_dir.parent().unwrap_or_else(|| Path::new(".")));

        let editor_path = workspace_root.join("perro_editor");
        dunce::canonicalize(&editor_path).unwrap_or(editor_path)
    };

    println!("Running project at {:?}", project_root);

    // CRITICAL: Set project root IMMEDIATELY after determining it, before ANY other operations
    // that might try to load assets (like profiling setup, graphics initialization, etc.)
    set_project_root(ProjectRoot::Disk {
        root: project_root.clone(),
        name: "unknown".into(),
    });

    // Initialize profiling if requested (after project_root is determined)
    #[cfg(feature = "profiling")]
    let _profiler_guard = if enable_profiling {
        use tracing_flame::FlameLayer;
        use tracing_subscriber::{prelude::*, registry::Registry};
        use std::process::Command;
        
        // Create paths at project root
        let folded_path = project_root.join("flamegraph.folded");
        let svg_path = project_root.join("flamegraph.svg");
        
        let (flame_layer, guard) = FlameLayer::with_file(&folded_path).unwrap();
        let subscriber = Registry::default()
            .with(flame_layer);
        tracing::subscriber::set_global_default(subscriber).unwrap();
        
        println!("🔥 Profiling enabled! Flamegraph will be written to {:?}", folded_path);
        
        // Create a guard that converts to SVG on exit
        struct ProfilerGuard {
            guard: tracing_flame::FlushGuard<std::io::BufWriter<std::fs::File>>,
            folded_path: PathBuf,
            svg_path: PathBuf,
        }
        
        impl ProfilerGuard {
            fn convert(&self) {
                // Use the module-level conversion function
                convert_flamegraph(&self.folded_path, &self.svg_path);
            }
        }
        
        impl Drop for ProfilerGuard {
            fn drop(&mut self) {
                // Also try to convert on drop as a fallback
                // Note: This might be called before the file is fully flushed,
                // but the explicit conversion after event_loop should handle it
                self.convert();
            }
        }
        
        Some(ProfilerGuard { 
            guard,
            folded_path,
            svg_path,
        })
    } else {
        None
    };
    
    #[cfg(not(feature = "profiling"))]
    if enable_profiling {
        eprintln!("⚠️  Profiling requested but not enabled!");
        eprintln!("   Build with: cargo run -p perro_core --features profiling -- --path <path> --profile");
        eprintln!("   Or add to Cargo.toml: [features] default = [\"profiling\"]");
    }

    // 2. Load project manifest (project root already set above)

    // 3. Load project manifest (works in both disk + pak)
    let project = Project::load(Some(&project_root)).expect("Failed to load project.toml");

    // 4. Update project root with real project name
    set_project_root(ProjectRoot::Disk {
        root: project_root.clone(),
        name: project.name().into(),
    });

    // 5. Wrap project in Rc<RefCell<>> for shared mutable access
    let project_rc = Rc::new(RefCell::new(project));

    // 6. Parse runtime arguments
    for arg in args.iter().skip(1) {
        if arg.starts_with("--") {
            let clean_key = arg.trim_start_matches("--").to_string();
            key = Some(clean_key);
        } else if let Some(k) = key.take() {
            project_rc.borrow_mut().set_runtime_param(&k, arg);
        }
    }

    // 7. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 8. Create window and Graphics before building scene
    #[cfg(not(target_arch = "wasm32"))]
    let default_size = winit::dpi::PhysicalSize::new(1280, 720); // Default size, monitor info not available before window creation

    let title = project_rc.borrow().name().to_string();
    let mut window_attrs = Window::default_attributes()
        .with_title(title)
        .with_visible(false);

    #[cfg(not(target_arch = "wasm32"))]
    {
        window_attrs = window_attrs.with_inner_size(default_size);
        if let Some(icon_path) = project_rc.borrow().icon() {
            // Load icon if needed (simplified for now)
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowAttributesExtWebSys;
        window_attrs = window_attrs.with_append(true);
    }

    // Note: create_window is deprecated in winit 0.30 in favor of ActiveEventLoop::create_window,
    // but ActiveEventLoop is only available in event handlers. Since we need the window before
    // running the app, we use the deprecated method here.
    #[cfg(target_arch = "wasm32")]
    let window = {
        #[allow(deprecated)]
        std::rc::Rc::new(event_loop.create_window(window_attrs).expect("create window"))
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window = {
        #[allow(deprecated)]
        std::sync::Arc::new(event_loop.create_window(window_attrs).expect("create window"))
    };

    // Create Graphics synchronously
    let mut graphics = create_graphics_sync(window.clone());

    // 9. Build runtime scene with DllScriptProvider (now with Graphics)
    let mut game_scene = match Scene::<DllScriptProvider>::from_project(project_rc.clone(), &mut graphics) {
        Ok(scene) => scene,
        Err(e) => {
            eprintln!("❌ Failed to build game scene: {}", e);
            eprintln!("   This usually means:");
            eprintln!("   1. The script DLL is missing or corrupted");
            eprintln!("   2. The DLL was built against a different version of perro_core");
            eprintln!("   3. There's a function signature mismatch");
            eprintln!("   4. The main scene is malformed");
            eprintln!("   Try rebuilding scripts: cargo run -p perro_core -- --path <path> --scripts");
            std::process::exit(1);
        }
    };

    // 10. Render first frame before showing window (prevents black/white flash)
    // This mimics what user_event does when graphics are created asynchronously
    {
        // Do initial update
        game_scene.update(&mut graphics);
        
        // Queue rendering
        game_scene.render(&mut graphics);
        
        // Render the frame
        let (frame, view, mut encoder) = graphics.begin_frame();
        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
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
                label: Some("Initial Frame (Pre-Visible)"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(depth_attachment),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            graphics.render(&mut rpass);
        }
        graphics.end_frame(frame, encoder);
    }
    
    // Now make window visible with content already rendered (no flash!)
    window.set_visible(true);

    // 11. Run app with pre-created Graphics
    let app = App::new(
        &event_loop,
        project_rc.borrow().name().to_string(),
        project_rc.borrow().icon(),
        Some(game_scene),
        project_rc.borrow().target_fps(),
        graphics,
    );

    let mut app = app;
    let _ = event_loop.run_app(&mut app);
    println!("Event loop exited.");
    
    // Explicitly convert flamegraph after event loop exits
    // Drop the guard first to flush the file, then convert
    #[cfg(feature = "profiling")]
    if enable_profiling {
        if let Some(guard) = _profiler_guard {
            // Extract paths before dropping
            let folded_path = guard.folded_path.clone();
            let svg_path = guard.svg_path.clone();
            // Drop the entire guard (which drops FlushGuard and flushes the file)
            drop(guard);
            // Now convert the flushed file using the extracted paths
            convert_flamegraph(&folded_path, &svg_path);
        }
    }
}
