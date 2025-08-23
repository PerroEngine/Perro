#![cfg_attr(windows, windows_subsystem = "windows")]

use std::env;
use std::path::PathBuf;
use perro_core::globals::set_project_root;
use perro_core::scene::Scene;
use perro_core::{App, Project, scene_node::{SceneNode, BaseNode}, Node, graphics::Graphics};
use winit::event_loop::EventLoop;

#[cfg(target_arch = "wasm32")]
fn run_app(event_loop: EventLoop<Graphics>, app: App) {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Error).expect("Couldn't initialize logger");

    use winit::platform::web::EventLoopExtWebSys;
    wasm_bindgen_futures::spawn_local(async move {
        event_loop.spawn_app(app);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn run_app(event_loop: EventLoop<Graphics>, mut app: App) {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();
    let _ = event_loop.run_app(&mut app);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // 1. Determine project root
   let project_root: PathBuf = if let Some(i) = args.iter().position(|a| a == "--path") {
        PathBuf::from(&args[i + 1])
    } else if args.contains(&"--editor".to_string()) {
        // Dev-only: hardcoded editor project path (relative to workspace root)
        let exe_dir = env::current_exe().unwrap();
        exe_dir.parent().unwrap().parent().unwrap().parent().unwrap().join("perro_editor")
    } else {
        if cfg!(debug_assertions) {
            // Dev mode: default to editor project
            let exe_dir = env::current_exe().unwrap();
            exe_dir.parent().unwrap().parent().unwrap().parent().unwrap().join("perro_editor")
        } else {
            // Release mode: assume project is the folder containing game.exe
            env::current_exe().unwrap().parent().unwrap().to_path_buf()
        }
    };

    println!("Running project at {:?}", project_root);

    // 2. Load project manifest
    let project = Project::load(&project_root);

    // 3. Set project root so res:// resolves correctly
    set_project_root(project_root);

    // 4. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 5. Create root scene
    let root_node = SceneNode::Node(Node::new("Root", None));
    let mut game_scene = Scene::new(root_node, true).unwrap();

    // 6. Load main scene from manifest
    let loaded_scene = Scene::load(project.main_scene()).unwrap();
    let game_root = *game_scene.get_root().get_id();
    game_scene.graft(loaded_scene, game_root).unwrap();

    // 7. Run app
    let app = App::new(&event_loop, project.name().to_string(), Some(game_scene));
    run_app(event_loop, app);
}