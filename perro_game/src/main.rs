use std::sync::Arc;

use perro_core::compiler::{BuildProfile, Compiler};
use perro_core::scene_node::{BaseNode, SceneNode};
use perro_core::ui_node::Ui;
use perro_core::{scene::Scene, Node2D, Vector2, graphics::Graphics};

use perro_core::{sprite2d, App, Node, Sprite2D};
use uuid::Uuid;
use winit::event_loop::{ControlFlow, EventLoop};


#[cfg(target_arch = "wasm32")]
fn run_app(event_loop: EventLoop<Graphics>, app: App) {
    // Sets up panics to go to the console.error in browser environments
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Error).expect("Couldn't initialize logger");

    // Runs the app async via the browsers event loop
    use winit::platform::web::EventLoopExtWebSys;
    wasm_bindgen_futures::spawn_local(async move {
        event_loop.spawn_app(app);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn run_app(event_loop: EventLoop<Graphics>, mut app: App) {
    // Allows the setting of the log level through RUST_LOG env var.
    // It also allows wgpu logs to be seen.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    // Runs the app on the current thread.
    let _ = event_loop.run_app(&mut app);
}

fn main() {

    let compiler = Compiler::new();
    let _ = compiler.compile(BuildProfile::Dev);
    // <T> (T -> AppEvent) extends regular platform specific events (resize, mouse, etc.).
    // This allows our app to inject custom events and handle them alongside regular ones.
    // let event_loop = EventLoop::<()>::new().unwrap();
    let event_loop = EventLoop::<Graphics>::with_user_event().build().unwrap();

    // Choose control flow based on environment variable: "PERRO_CONTROL_FLOW"
    // "poll" (default) or "wait"
    let control_flow = std::env::var("PERRO_CONTROL_FLOW").unwrap_or_else(|_| "poll".to_string());
    match control_flow.to_lowercase().as_str() {
        "wait" => event_loop.set_control_flow(ControlFlow::Wait),
        _ => event_loop.set_control_flow(ControlFlow::Poll),
    }

    let root_node = SceneNode::Node(Node::new("GameRoot", None));
    let mut game_scene = Scene::new(root_node, true).unwrap();
    let mut loaded_scene   = Scene::load("res://flat_scene.scn").unwrap();
    let game_root    = *game_scene.get_root().get_id();

    // merge every node from `flat_scene` underneath `game_root`
    game_scene
        .graft(loaded_scene, game_root)
        .unwrap();

    game_scene.traverse(game_root, &mut |node| {
    println!("  node: {:?}, type: {:?}", node.get_id(), node);
});

game_scene.save("res://gamescene.scn");

    let app = App::new(&event_loop, "Perro Game".to_string(), Some(game_scene));
    run_app(event_loop, app);
}


