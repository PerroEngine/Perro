use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use chrono::Utc;

use crate::asset_io::set_key;
use crate::asset_io::{ProjectRoot, set_project_root};
use crate::graphics::{Graphics, create_graphics_sync};
use crate::manifest::Project;
use crate::rendering::app::App;
use crate::scene::{Scene, SceneData};
use crate::script::{CreateFn, ScriptProvider};
use crate::rendering::static_mesh::StaticMeshData;
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
    pub meshes: &'static Lazy<HashMap<&'static str, &'static StaticMeshData>>,
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
    /// Global script identifiers in deterministic order (Root = 1, first global = 2, etc.)
    pub global_registry_order: &'static [&'static str],
    /// Global display names from @global Name (same order as global_registry_order)
    pub global_registry_names: &'static [&'static str],
}

/// Generic static script provider that lives in core
pub struct StaticScriptProvider {
    ctors: &'static Map<&'static str, CreateFn>,
    pub scenes: &'static Lazy<HashMap<&'static str, &'static SceneData>>,
    pub fur: &'static Lazy<HashMap<&'static str, &'static [FurElement]>>,
    global_registry_order: &'static [&'static str],
    global_registry_names: &'static [&'static str],
}

impl StaticScriptProvider {
    pub fn new(data: &RuntimeData) -> Self {
        Self {
            ctors: data.script_registry,
            scenes: data.static_assets.scenes,
            fur: data.static_assets.fur,
            global_registry_order: data.global_registry_order,
            global_registry_names: data.global_registry_names,
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

    fn get_global_registry_order(&self) -> &[&str] {
        self.global_registry_order
    }

    fn get_global_registry_names(&self) -> &[&str] {
        self.global_registry_names
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

static STATIC_MESHES: AtomicPtr<
    std::collections::HashMap<&'static str, &'static StaticMeshData>,
> = AtomicPtr::new(std::ptr::null_mut());

/// Initialize static meshes (called once at startup in runtime mode)
pub fn set_static_meshes(
    meshes: &'static Lazy<HashMap<&'static str, &'static StaticMeshData>>,
) {
    let map_ref = &**meshes;
    STATIC_MESHES.store(map_ref as *const _ as *mut _, Ordering::Release);
}

/// Get static meshes map (returns None if not initialized or in dev mode)
pub fn get_static_meshes()
-> Option<&'static std::collections::HashMap<&'static str, &'static StaticMeshData>> {
    let ptr = STATIC_MESHES.load(Ordering::Acquire);
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
            eprintln!(
                "   Or manually convert: flamegraph {:?} > {:?}",
                folded_path, svg_path
            );
        }
        Err(_) => {
            eprintln!("⚠️  flamegraph command not found. Install with: cargo install flamegraph");
            eprintln!(
                "   Or manually convert: flamegraph {:?} > {:?}",
                folded_path, svg_path
            );
        }
    }
}

/// Helper to append errors to errors.log
/// In release mode with windows_subsystem, console output is hidden,
/// so we MUST write to a file that the user can check.
fn log_error(msg: &str) {
    let timestamp = if cfg!(not(target_arch = "wasm32")) {
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    } else {
        "N/A".to_string()
    };

    let full_msg = format!("[{}] {}\n", timestamp, msg);

    // Try to print to stderr (won't show in release mode with windows_subsystem, but doesn't hurt)
    eprintln!("{}", full_msg.trim());

    // Try multiple locations to ensure we can write somewhere
    let mut log_paths = Vec::new();

    // 1. Exe directory (most likely location for standalone builds)
    if let Ok(exe_path) = env::current_exe() {
        if let Some(folder) = exe_path.parent() {
            log_paths.push(folder.join("errors.log"));
        }
    }

    // 2. Current working directory
    if let Ok(cwd) = env::current_dir() {
        log_paths.push(cwd.join("errors.log"));
    }

    // 3. Temp directory (fallback - always accessible)
    let temp_dir = env::temp_dir();
    log_paths.push(temp_dir.join("perro_errors.log"));

    // 4. User's home directory (another fallback)
    if let Ok(home) = env::var("HOME") {
        log_paths.push(PathBuf::from(&home).join("perro_errors.log"));
    }
    if let Ok(home) = env::var("USERPROFILE") {
        log_paths.push(PathBuf::from(&home).join("perro_errors.log"));
    }

    // Try to write to each location until one succeeds
    let mut written = false;
    let mut successful_path = None;

    for log_path in log_paths {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(mut file) => {
                if writeln!(file, "{}", full_msg).is_ok() {
                    // Force sync to ensure data is written to disk immediately
                    let _ = file.sync_all();
                    written = true;
                    successful_path = Some(log_path);
                    break;
                }
            }
            Err(_) => {
                // Try next location
                continue;
            }
        }
    }

    // If we wrote successfully, also try to create a marker file in exe dir with the path
    if written {
        if let (Some(path), Ok(exe_path)) = (successful_path.as_ref(), env::current_exe()) {
            if let Some(exe_dir) = exe_path.parent() {
                let marker_path = exe_dir.join("ERROR_LOG_LOCATION.txt");
                let _ = std::fs::write(
                    &marker_path,
                    format!(
                        "Error log written to:\n{}\n\nCheck this file for error details.",
                        path.display()
                    ),
                );
            }
        }
    }
}

/// Main entry point for running a Perro game
/// `runtime_params` should be parsed from command-line arguments in the format `--key value`
pub fn run_game(data: RuntimeData, runtime_params: HashMap<String, String>) {
    // Name the main thread
    crate::thread_utils::set_current_thread_name("Main");

    // Set up a comprehensive panic hook to capture all crashes
    std::panic::set_hook(Box::new(|panic_info| {
        let mut error_msg = String::new();
        error_msg.push_str(
            "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
        );
        error_msg.push_str("❌ PANIC occurred!\n");

        if let Some(location) = panic_info.location() {
            error_msg.push_str(&format!(
                "   Location: {}:{}:{}\n",
                location.file(),
                location.line(),
                location.column()
            ));
        }

        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            error_msg.push_str(&format!("   Message: {}\n", s));
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            error_msg.push_str(&format!("   Message: {}\n", s));
        } else {
            error_msg.push_str("   Message: (no message available)\n");
        }

        // Try to get backtrace if available
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::backtrace::Backtrace;
            let backtrace = Backtrace::capture();
            if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
                error_msg.push_str("\n   Backtrace:\n");
                let bt_str = format!("{}", backtrace);
                // Indent each line of the backtrace
                for line in bt_str.lines() {
                    error_msg.push_str(&format!("   {}\n", line));
                }
            }
        }

        error_msg.push_str(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n",
        );

        // Print to stderr (visible in debug mode)
        eprintln!("{}", error_msg);

        // Also log to file (critical for release mode with windows_subsystem)
        log_error(&error_msg);
    }));

    {
        set_key(data.aes_key);
        set_project_root(ProjectRoot::Brk {
            data: data.assets_brk,
            name: data.static_assets.project.name().into(),
        });
    }

    // 2. Clone the static project so we can add runtime params
    let mut project = data.static_assets.project.clone();

    // 3. Add runtime parameters to project
    for (key, value) in runtime_params {
        project.set_runtime_param(&key, &value);
    }

    // 4. Initialize static textures (runtime mode only)
    set_static_textures(data.static_assets.textures);

    // 4b. Initialize static meshes (runtime mode only)
    set_static_meshes(data.static_assets.meshes);

    // 5. Initialize static FUR map (for Include tag resolution in release mode)
    crate::apply_fur::set_static_fur_map(data.static_assets.fur);

    // 6. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 6. Create window at scaled-down size so it fits typical monitors (e.g. 1080x1920 → 720x1280); resumed() may clamp further to primary monitor
    #[cfg(not(target_arch = "wasm32"))]
    let default_size = crate::rendering::app::initial_window_size_from_virtual(
        project.virtual_width(),
        project.virtual_height(),
    );

    let title = project.name().to_string();
    let mut window_attrs = Window::default_attributes()
        .with_title(title)
        .with_visible(false);

    #[cfg(not(target_arch = "wasm32"))]
    {
        window_attrs = window_attrs.with_inner_size(default_size);
        if let Some(icon_path) = project.icon() {
            if let Some(icon) = crate::rendering::app::load_icon(&icon_path) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        } else {
            // Use default icon if no custom icon specified
            if let Some(icon) = crate::rendering::app::load_default_icon() {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
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
        let w = std::rc::Rc::new(
            event_loop
                .create_window(window_attrs)
                .expect("create window"),
        );
        w.set_ime_allowed(true); // Enable IME for text input
        w
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window = {
        #[allow(deprecated)]
        let w = std::sync::Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("create window"),
        );
        w.set_ime_allowed(true); // Enable IME for text input
        w
    };

    // Create Graphics synchronously (MSAA from project.toml [graphics] msaa)
    let mut graphics = create_graphics_sync(
        window.clone(),
        project.virtual_width(),
        project.virtual_height(),
        project.msaa_samples(),
    );

    // 7. Build runtime scene using StaticScriptProvider (now with Graphics)
    let provider = StaticScriptProvider::new(&data);

    // Wrap project in Rc<RefCell>
    let project_rc = Rc::new(RefCell::new(project));

    let game_scene =
        match Scene::from_project_with_provider(project_rc.clone(), provider, &mut graphics) {
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
        project_rc.borrow().fps_cap(),
        graphics,
    );

    // Note: ups_divisor runtime parameter will be checked by the root script in init()
    // and applied via api.set_ups_divisor()

    // The panic hook set up earlier will capture any crashes and log them to errors.log
    run_app(event_loop, app);
}

/// Resolve project root path from command line arguments or environment
/// This is extracted from run_dev() to allow calling run_dev_with_path() directly
#[cfg(not(target_arch = "wasm32"))]
fn resolve_dev_project_path() -> Result<PathBuf, String> {
    let args: Vec<String> = env::args().collect();

    // 1. Determine project root path (disk or exe dir) - need this IMMEDIATELY
    if let Some(i) = args.iter().position(|a| a == "--path") {
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
        // These search for folders with those names that contain project.toml
        // NOTE: This only happens when explicitly requested via --path --editor or --path --test
        if path_arg.eq_ignore_ascii_case("--editor") || path_arg.eq_ignore_ascii_case("--test") {
            let search_name = if path_arg.eq_ignore_ascii_case("--editor") {
                "perro_editor"
            } else {
                "test"
            };

            // Helper to find a project folder by name
            let find_project_by_name = |dir: &Path, name: &str| -> Option<PathBuf> {
                let candidate = dir.join(name);
                if candidate.join("project.toml").exists() {
                    return Some(dunce::canonicalize(&candidate).unwrap_or(candidate));
                }
                // Also check in subdirectories
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let candidate = path.join(name);
                            if candidate.join("project.toml").exists() {
                                return Some(dunce::canonicalize(&candidate).unwrap_or(candidate));
                            }
                        }
                    }
                }
                None
            };

            // Search in workspace root and parent directories
            let mut search_dirs = vec![workspace_root.clone()];
            if let Some(parent) = workspace_root.parent() {
                search_dirs.push(parent.to_path_buf());
            }

            // Try to find the project folder
            let mut found_path = None;
            for search_dir in search_dirs {
                if let Some(found) = find_project_by_name(&search_dir, search_name) {
                    found_path = Some(found);
                    break;
                }
            }

            if let Some(found) = found_path {
                use dunce;
                return Ok(dunce::canonicalize(&found).unwrap_or(found));
            } else {
                // If not found, return error
                let error_msg = format!(
                    "ERROR: Could not find project folder '{}' with project.toml.\n\
                    Searched in workspace root and parent directories.",
                    search_name
                );
                return Err(error_msg);
            }
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
                return Ok(candidate);
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
                return Ok(dunce::canonicalize(&full_path).unwrap_or_else(|_| {
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
                }));
            }
        }
    } else if args.contains(&"--editor".to_string()) {
        // Search for perro_editor project folder (only when --editor flag is explicitly provided)
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().expect("Failed to get working directory"));

        // Helper to find a project folder by name
        let find_project_by_name = |dir: &Path, name: &str| -> Option<PathBuf> {
            let candidate = dir.join(name);
            if candidate.join("project.toml").exists() {
                return Some(dunce::canonicalize(&candidate).unwrap_or(candidate));
            }
            // Also check sibling folders
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.file_name().and_then(|n| n.to_str()) == Some(name) {
                        if path.join("project.toml").exists() {
                            return Some(dunce::canonicalize(&path).unwrap_or(path));
                        }
                    }
                }
            }
            None
        };

        // Search in exe directory, parent, and workspace root
        let mut search_dirs = vec![exe_dir.clone()];
        if let Some(parent) = exe_dir.parent() {
            search_dirs.push(parent.to_path_buf());
        }
        if let Some(ws_root) = exe_dir.ancestors().find(|p| p.join("Cargo.toml").exists()) {
            search_dirs.push(ws_root.to_path_buf());
        }

        // Try to find the project folder
        let mut found_path = None;
        for search_dir in search_dirs {
            if let Some(found) = find_project_by_name(&search_dir, "perro_editor") {
                found_path = Some(found);
                break;
            }
        }

        if let Some(found) = found_path {
            return Ok(dunce::canonicalize(&found).unwrap_or(found));
        } else {
            // If not found, return error
            let error_msg = "ERROR: Could not find 'perro_editor' project folder with project.toml.\n\
                             Searched in exe directory, parent, and workspace root.";
            return Err(error_msg.to_string());
        }
    } else {
        // Default behavior: look for ANY sibling folders with project.toml (no hardcoded names)
        // This does NOT search for perro_editor or test - only generic sibling folders
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().expect("Failed to get working directory"));

        // Helper function to find project in a directory
        let find_project_in_dir = |dir: &Path| -> Option<PathBuf> {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let project_toml = path.join("project.toml");
                        if project_toml.exists() {
                            return Some(dunce::canonicalize(&path).unwrap_or(path));
                        }
                    }
                }
            }
            None
        };

        // Look for sibling folders with project.toml
        // ONLY search in the exe directory - never go up to parent/workspace
        // Search order:
        // 1. Exe directory itself (if it contains project.toml)
        // 2. Sibling folders in exe directory (where exe is located)

        // First check if exe directory itself is a project
        if exe_dir.join("project.toml").exists() {
            println!("Found project.toml in exe directory: {:?}", exe_dir);
            return Ok(dunce::canonicalize(&exe_dir).unwrap_or(exe_dir));
        // Then check sibling folders in exe directory (where the exe actually is)
        } else if let Some(found) = find_project_in_dir(&exe_dir) {
            println!("Found project at sibling folder: {:?}", found);
            return Ok(found);
        } else {
            let error_msg = format!(
                "ERROR: No project.toml found.\n\
                Searched locations:\n\
                - Exe directory: {:?}\n\
                - Sibling folders in exe directory: {:?}\n\
                \n\
                Please specify --path <path> or place project.toml in a sibling folder in the same directory as the executable.",
                exe_dir, exe_dir
            );
            return Err(error_msg);
        }
    }
}

/// Development entry point for running projects from disk with DLL hot-reloading
#[cfg(not(target_arch = "wasm32"))]
pub fn run_dev() {
    match resolve_dev_project_path() {
        Ok(project_root) => run_dev_with_path(project_root),
        Err(e) => {
            log_error(&e);
            std::process::exit(1);
        }
    }
}

/// Development entry point for running projects from disk with DLL hot-reloading
/// This version accepts a project path directly instead of reading from command line args
#[cfg(not(target_arch = "wasm32"))]
pub fn run_dev_with_path(project_root: PathBuf) {
    use crate::registry::DllScriptProvider;

    // Name the main thread
    crate::thread_utils::set_current_thread_name("Main");

    // Try to initialize logger, but don't panic if it's already initialized (e.g., when called from editor)
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error"))
        .try_init();

    let args: Vec<String> = env::args().collect();

    // Check for profiling flag
    let enable_profiling =
        args.contains(&"--profile".to_string()) || args.contains(&"--flamegraph".to_string());

    // Parse runtime arguments (excluding --path which is already handled)
    let mut runtime_params = HashMap::new();
    let mut key: Option<String> = None;
    for arg in args.iter().skip(1) {
        // Skip --path and its value
        if arg == "--path" {
            key = None; // Clear any pending key
            continue;
        }
        if arg.starts_with("--") {
            let clean_key = arg.trim_start_matches("--").to_string();
            key = Some(clean_key);
        } else if let Some(k) = key.take() {
            runtime_params.insert(k, arg.clone());
        }
    }

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
        use std::process::Command;
        use tracing_flame::FlameLayer;
        use tracing_subscriber::{prelude::*, registry::Registry};

        // Create paths at project root
        let folded_path = project_root.join("flamegraph.folded");
        let svg_path = project_root.join("flamegraph.svg");

        let (flame_layer, guard) = FlameLayer::with_file(&folded_path).unwrap();
        let subscriber = Registry::default().with(flame_layer);
        tracing::subscriber::set_global_default(subscriber).unwrap();

        println!(
            "🔥 Profiling enabled! Flamegraph will be written to {:?}",
            folded_path
        );

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
        eprintln!(
            "   Build with: cargo run -p perro_core --features profiling -- --path <path> --profile"
        );
        eprintln!("   Or add to Cargo.toml: [features] default = [\"profiling\"]");
    }

    // 2. Load project manifest (project root already set above)

    // 3. Load project manifest (works in both disk + pak)
    let project = match Project::load(Some(&project_root)) {
        Ok(p) => p,
        Err(e) => {
            let error_msg = format!(
                "ERROR: Failed to load project.toml from {:?}\n\
                Error: {}\n\
                \n\
                Please ensure project.toml exists and is valid.",
                project_root, e
            );
            log_error(&error_msg);
            std::process::exit(1);
        }
    };

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
    let event_loop = match EventLoop::<Graphics>::with_user_event().build() {
        Ok(loop_) => loop_,
        Err(e) => {
            let error_msg = format!("Failed to create event loop: {}", e);
            log_error(&error_msg);
            std::process::exit(1);
        }
    };

    // 8. Create window at scaled-down size so it fits typical monitors; resumed() may clamp further to primary monitor
    #[cfg(not(target_arch = "wasm32"))]
    let default_size = crate::rendering::app::initial_window_size_from_virtual(
        project_rc.borrow().virtual_width(),
        project_rc.borrow().virtual_height(),
    );

    let title = project_rc.borrow().name().to_string();
    let mut window_attrs = Window::default_attributes()
        .with_title(title)
        .with_visible(false);

    #[cfg(not(target_arch = "wasm32"))]
    {
        window_attrs = window_attrs.with_inner_size(default_size);
        if let Some(icon_path) = project_rc.borrow().icon() {
            if let Some(icon) = crate::rendering::app::load_icon(&icon_path) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        } else {
            // Use default icon if no custom icon specified
            if let Some(icon) = crate::rendering::app::load_default_icon() {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
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
        let w = std::rc::Rc::new(
            event_loop
                .create_window(window_attrs)
                .expect("create window"),
        );
        w.set_ime_allowed(true); // Enable IME for text input
        w
    };
    #[cfg(not(target_arch = "wasm32"))]
    let window = {
        #[allow(deprecated)]
        let w = std::sync::Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("create window"),
        );
        w.set_ime_allowed(true); // Enable IME for text input
        w
    };

    // Create Graphics synchronously (virtual resolution and MSAA from project.toml [graphics])
    let mut graphics = create_graphics_sync(
        window.clone(),
        project_rc.borrow().virtual_width(),
        project_rc.borrow().virtual_height(),
        project_rc.borrow().msaa_samples(),
    );

    // 9. Build runtime scene with DllScriptProvider (now with Graphics)
    let mut game_scene =
        match Scene::<DllScriptProvider>::from_project(project_rc.clone(), &mut graphics) {
            Ok(scene) => scene,
            Err(e) => {
                let error_msg = format!("Failed to build game scene: {}", e);
                log_error(&error_msg);
                eprintln!("❌ Failed to build game scene: {}", e);
                eprintln!("   This usually means:");
                eprintln!("   1. The script DLL is missing or corrupted");
                eprintln!("   2. The DLL was built against a different version of perro_core");
                eprintln!("   3. There's a function signature mismatch");
                eprintln!("   4. The main scene is malformed");
                eprintln!(
                    "   Try rebuilding scripts: cargo run -p perro_core -- --path <path> --scripts"
                );
                std::process::exit(1);
            }
        };

    // 10. Render first frame before showing window (prevents black/white flash)
    // This mimics what user_event does when graphics are created asynchronously
    {
        // Do initial update (unified update/render)
        let now = std::time::Instant::now();
        game_scene.update(&mut graphics, now);

        // Render the frame (MSAA on: render to msaa_color_view then resolve; off: render to swap chain)
        let (frame, view, mut encoder) = graphics.begin_frame();
        let color_attachment = match &graphics.msaa_color_view {
            Some(msaa_view) => wgpu::RenderPassColorAttachment {
                view: msaa_view,
                resolve_target: Some(&view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            },
            None => wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            },
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
            });
            graphics.render(&mut rpass);
        }
        graphics.end_frame(frame, encoder);
    }

    // Now make window visible with content already rendered (no flash!)
    window.set_visible(true);

    // 11. Run app with pre-created Graphics
    let mut app = App::new(
        &event_loop,
        project_rc.borrow().name().to_string(),
        project_rc.borrow().icon(),
        Some(game_scene),
        project_rc.borrow().fps_cap(),
        graphics,
    );

    // Check for ups_divisor runtime parameter - the root script will apply it in init()
    // This is cleaner than trying to access the command sender here
    if let Some(ups_divisor_str) = project_rc.borrow().get_runtime_param("ups_divisor") {
        if let Ok(divisor) = ups_divisor_str.parse::<u32>() {
            eprintln!(
                "[runtime] UPS divisor runtime param set to {} - root script will apply it",
                divisor
            );
        }
    }

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
