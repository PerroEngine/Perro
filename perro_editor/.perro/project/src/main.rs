use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use perro_core::globals::set_project_root;
use perro_core::scene::Scene;
use perro_core::{Project, graphics::Graphics};
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

            // Redirect panic messages to the file
            let mut file = std::sync::Mutex::new(file);
            std::panic::set_hook(Box::new(move |info| {
                let _ = writeln!(file.lock().unwrap(), "PANIC: {}", info);
            }));

            println!("Logging errors to {:?}", log_path);
        }
    }

    let args: Vec<String> = env::args().collect();

    // 1. Determine project root
    let project_root: PathBuf = env::current_exe().unwrap().parent().unwrap().to_path_buf();

    println!("Running project at {:?}", project_root);

    // 2. Load project manifest
    let project = Project::load(&project_root);
    set_project_root(project_root);

    // 3. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 4. Build runtime scene with StaticScriptProvider
    let provider = StaticScriptProvider::new();
    let game_scene = match Scene::from_project_with_provider(&project, provider) {
        Ok(scene) => scene,
        Err(e) => {
            log_error(&format!("Failed to build game scene: {e}"));
            return;
        }
    };

    // 5. Run app
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