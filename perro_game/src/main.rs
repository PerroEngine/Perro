use std::path::PathBuf;
use winit::event_loop::{ControlFlow, EventLoop};

use perro_core::{
    graphics::Graphics, scene::Scene, scene_node::{BaseNode, SceneNode}, App, Node, Project
};
// Import set_project_root from its actual location
use perro_core::globals::set_project_root;

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
    // 1. Determine project root (from --path or cwd)
    let args: Vec<String> = std::env::args().collect();
    let project_root = if let Some(i) = args.iter().position(|a| a == "--path") {
        PathBuf::from(&args[i + 1])
    } else {
        std::env::current_dir().unwrap()
    };

    // 2. Load project manifest (game.toml or settings.toml)
    let project = Project::load(&project_root);

    // 3. Set project root so res:// resolves correctly
    set_project_root(project_root);

    // 4. Create event loop
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // 5. Create root scene
    let root_node = SceneNode::Node(Node::new("GameRoot", None));
    let mut game_scene = Scene::new(root_node, true).unwrap();

    // 6. Load main scene from manifest
    let loaded_scene = Scene::load(project.main_scene()).unwrap();
    let game_root = *game_scene.get_root().get_id();
    game_scene.graft(loaded_scene, game_root).unwrap();

    // 7. Run app
    let app = App::new(&event_loop, project.name().to_string(), Some(game_scene));
    run_app(event_loop, app);
}