use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::rc::Rc;

use crate::asset_io::set_key;
use crate::asset_io::{ProjectRoot, set_project_root};
use crate::graphics::Graphics;
use crate::manifest::Project;
use crate::rendering::app::App;
use crate::scene::{Scene, SceneData};
use crate::script::{CreateFn, ScriptProvider};
use crate::ui::ast::FurElement;
use once_cell::sync::Lazy;
use winit::event_loop::EventLoop;

/// Static assets that are bundled with the binary
pub struct StaticAssets {
    pub project: &'static Project,
    pub scenes: &'static Lazy<HashMap<&'static str, &'static SceneData>>,
    pub fur: &'static Lazy<HashMap<&'static str, &'static [FurElement]>>,
}

/// Project-specific data that needs to be passed from the binary
pub struct RuntimeData {
    /// Embedded assets.brk bytes
    pub assets_brk: &'static [u8],
    /// AES encryption key
    pub aes_key: [u8; 32],
    /// Static versions of scenes, ui, etc
    pub static_assets: StaticAssets,
    /// Script constructor registry
    pub script_registry: HashMap<String, CreateFn>,
}

/// Generic static script provider that lives in core
pub struct StaticScriptProvider {
    ctors: HashMap<String, CreateFn>,
    pub scenes: &'static Lazy<HashMap<&'static str, &'static SceneData>>,
    pub fur: &'static Lazy<HashMap<&'static str, &'static [FurElement]>>,
}

impl StaticScriptProvider {
    pub fn new(data: &RuntimeData) -> Self {
        Self {
            ctors: data.script_registry.clone(),
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

    // 4. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 5. Build runtime scene using StaticScriptProvider
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
        PathBuf::from(&args[i + 1])
    } else if args.contains(&"--editor".to_string()) {
        // Dev-only: hardcoded editor project path (relative to workspace root)
        let exe_dir = env::current_exe().unwrap();
        exe_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("perro_editor")
    } else {
        // Dev mode: default to editor project
        let exe_dir = env::current_exe().unwrap();
        exe_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("perro_editor")
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
