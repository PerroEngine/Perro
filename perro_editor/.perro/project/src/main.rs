#![cfg_attr(windows, windows_subsystem = "windows")] // no console on Windows

// ✅ Embed res.zip built by build.rs
static RES_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/res.zip"));

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use perro_core::asset_io::{set_project_root, get_project_root, ProjectRoot};
use perro_core::manifest::Project;
use perro_core::scene::Scene;
use perro_core::graphics::Graphics;
use perro_core::rendering::app::App;
use winit::event_loop::EventLoop;

mod registry;
use registry::StaticScriptProvider;

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

fn main() {
    // 🔑 Setup error log file
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

    // 1. Set project root
    #[cfg(not(debug_assertions))]
    {
        // ✅ Release mode: use embedded res.zip
        set_project_root(ProjectRoot::Pak { data: RES_ZIP, name: "unknown".into() });
    }

    #[cfg(debug_assertions)]
    {
        // ✅ Dev mode: use disk
        let project_root: PathBuf = env::current_exe().unwrap().parent().unwrap().to_path_buf();
        set_project_root(ProjectRoot::Disk { root: project_root.clone(), name: "unknown".into() });
    }

    // 2. Load project manifest (works in both disk + pak)
    #[cfg(not(debug_assertions))]
    let project = Project::load(None::<PathBuf>).expect("Failed to load project.toml");

    #[cfg(debug_assertions)]
    let project = {
        let project_root: PathBuf = env::current_exe().unwrap().parent().unwrap().to_path_buf();
        Project::load(Some(&project_root)).expect("Failed to load project.toml")
    };

    // 3. Update project root with real name
    match get_project_root() {
        ProjectRoot::Disk { root, .. } => {
            set_project_root(ProjectRoot::Disk { root, name: project.name().into() });
        }
        ProjectRoot::Pak { data, .. } => {
            set_project_root(ProjectRoot::Pak { data, name: project.name().into() });
        }
    }

    // 4. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 5. Build runtime scene
    let provider = StaticScriptProvider::new();
    let game_scene = match Scene::from_project_with_provider(&project, provider) {
        Ok(scene) => scene,
        Err(e) => {
            log_error(&format!("Failed to build game scene: {e}"));
            return;
        }
    };

    // 6. Run app
    let app = App::new(&event_loop, project.name().to_string(), Some(game_scene));
    run_app(event_loop, app);
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