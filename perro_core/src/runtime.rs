use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::asset_io::set_key;
use crate::asset_io::{ProjectRoot, set_project_root};
use crate::graphics::Graphics;
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
    // Setup error log file
    if let Ok(exe_path) = env::current_exe() {
        if let Some(folder) = exe_path.parent() {
            let log_path = folder.join("errors.log");
            let file = File::create(&log_path).expect("Failed to create error log file");

            let mut file = std::sync::Mutex::new(file);
            std::panic::set_hook(Box::new(move |info| {
                let _ = writeln!(file.lock().unwrap(), "PANIC: {}", info);
            }));

            println!("Logging errors to {:?}", log_path);
        }
    }

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

    // 6. Build runtime scene using StaticScriptProvider
    let provider = StaticScriptProvider::new(&data);

    // Wrap project in Rc<RefCell>
    let project_rc = Rc::new(RefCell::new(project));

    let game_scene = match Scene::from_project_with_provider(project_rc.clone(), provider) {
        Ok(scene) => scene,
        Err(e) => {
            log_error(&format!("Failed to build game scene: {e}"));
            return;
        }
    };

    // Build App
    let app = App::new(
        &event_loop,
        project_rc.borrow().name().to_string(),
        project_rc.borrow().icon(),
        Some(game_scene),
        project_rc.borrow().target_fps(),
    );

    run_app(event_loop, app);
}

/// Development entry point for running projects from disk with DLL hot-reloading
#[cfg(not(target_arch = "wasm32"))]
pub fn run_dev() {
    use crate::registry::DllScriptProvider;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    let args: Vec<String> = env::args().collect();
    let mut key: Option<String> = None;

    // 1. Determine project root path (disk or exe dir)
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

    // 2. Bootstrap project root with placeholder name
    set_project_root(ProjectRoot::Disk {
        root: project_root.clone(),
        name: "unknown".into(),
    });

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

    // 8. Build runtime scene with DllScriptProvider (uses the impl-specific from_project)
    let game_scene = Scene::<DllScriptProvider>::from_project(project_rc.clone())
        .expect("Failed to build game scene");

    // 9. Run app
    let app = App::new(
        &event_loop,
        project_rc.borrow().name().to_string(),
        project_rc.borrow().icon(),
        Some(game_scene),
        project_rc.borrow().target_fps(),
    );

    let mut app = app;
    let _ = event_loop.run_app(&mut app);
}
