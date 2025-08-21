use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use perro_core::App;
use perro_core::{graphics::Graphics, lang::transpiler::transpile, scene::Scene, scene_node::SceneNode, set_project_root, Node, Node2D, Sprite2D, Vector2};

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

fn bmain() {
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

    let root_node = Node::new("RootNode", None);

    let root_scene_node = SceneNode::Node(root_node);

    let game_scene = Scene::new(root_scene_node, true);
    let app = App::new(&event_loop, "Perro Game".to_string(), Some(game_scene.unwrap()));
    
    run_app(event_loop, app);
    println!("Scene created and saved to res://scene.scn");
}

fn main() {
    let scripts = [
        "res://scripts/editor.pup",
    ];

    set_project_root(r"c:\Users\super\OneDrive\Documents\Perro\perro\perro_editor".into());

    if let Err(e) = transpile(&scripts) {
        eprintln!("❌ Build failed: {}", e);
        return;
    }

    println!("🚀 All scripts transpiled and compiled successfully!");
}