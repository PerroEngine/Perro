use std::env;
use std::path::PathBuf;

use perro_core::asset_io::{set_project_root, ProjectRoot};
use perro_core::manifest::Project;
use perro_core::registry::DllScriptProvider;
use perro_core::scene::Scene;
#[cfg(not(target_arch = "wasm32"))]
use perro_core::ScriptProvider;
use perro_core::{graphics::Graphics};
use perro_core::rendering::app::App;
use winit::event_loop::EventLoop;

#[cfg(target_arch = "wasm32")]
fn run_app<P: ScriptProvider>(event_loop: EventLoop<Graphics>, app: App<P>) {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Error).expect("Couldn't initialize logger");

    use winit::platform::web::EventLoopExtWebSys;
    wasm_bindgen_futures::spawn_local(async move {
        event_loop.spawn_app(app);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn run_app<P: ScriptProvider>(event_loop: EventLoop<Graphics>, mut app: App<P>) {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    let _ = event_loop.run_app(&mut app);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // 1. Determine project root path (disk or exe dir)
    let project_root: PathBuf = if let Some(i) = args.iter().position(|a| a == "--path") {
        PathBuf::from(&args[i + 1])
    } else if args.contains(&"--editor".to_string()) {
        // Dev-only: hardcoded editor project path (relative to workspace root)
        let exe_dir = env::current_exe().unwrap();
        exe_dir.parent().unwrap().parent().unwrap().parent().unwrap().join("perro_editor")
    } else {
            // Dev mode: default to editor project
            let exe_dir = env::current_exe().unwrap();
            exe_dir.parent().unwrap().parent().unwrap().parent().unwrap().join("perro_editor")
        
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

    // 5. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 6. Build runtime scene
    let game_scene = Scene::<DllScriptProvider>::from_project(&project)
        .expect("Failed to build game scene");

    // 7. Run app
    let app = App::new(&event_loop, project.name().to_string(), Some(game_scene), project.target_fps());
    run_app(event_loop, app);
}