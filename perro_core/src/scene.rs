use crate::{
    Graphics, Node, api::ScriptApi, app_command::AppCommand, apply_fur::{build_ui_elements_from_fur, parse_fur_file}, asset_io::{ProjectRoot, get_project_root, load_asset, save_asset}, ast::{FurElement, FurNode}, lang::transpiler::script_path_to_identifier, manifest::Project, node_registry::SceneNode, node_registry::BaseNode, script::{CreateFn, SceneAccess, Script, ScriptObject, ScriptProvider, UpdateOp, Var}, ui_element::{BaseElement, UIElement}, ui_renderer::render_ui// NEW import
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wgpu::RenderPass;
use std::{
    any::Any, cell::RefCell, collections::HashMap, io, path::PathBuf, rc::Rc, sync::mpsc::Sender, time::{Duration, Instant} // NEW import
};
use uuid::Uuid;

//
// ---------------- SceneData ----------------
//

/// Pure serializable scene data (no runtime state)
#[derive(Serialize, Deserialize)]
pub struct SceneData {
    pub root_id: Uuid,
    pub nodes: IndexMap<Uuid, SceneNode>,
}

impl SceneData {
    /// Create a new data scene with a root node
    pub fn new(root: SceneNode) -> Self {
        let root_id = *root.get_id();
        let mut nodes = IndexMap::new();
        nodes.insert(root_id, root);
        Self { root_id, nodes }
    }

    /// Save scene data to disk (res:// or user://)
    pub fn save(&self, res_path: &str) -> io::Result<()> {
        let data = serde_json::to_vec_pretty(&self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        save_asset(res_path, &data)
    }

    /// Load scene data from disk or pak
    pub fn load(res_path: &str) -> io::Result<Self> {
        let bytes = load_asset(res_path)?;
        let mut data: SceneData = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Self::fix_relationships(&mut data);
        Ok(data)
    }

    fn fix_relationships(data: &mut SceneData) {
        // Fix IDs and parent/child relationships
        for (&key, node) in data.nodes.iter_mut() {
            node.set_id(key);
            node.get_children_mut().clear();
        }

        let child_parent_pairs: Vec<(Uuid, Uuid)> = data
            .nodes
            .iter()
            .filter_map(|(&child_id, node)| node.get_parent().map(|pid| (child_id, pid)))
            .collect();

        for (child_id, parent_id) in child_parent_pairs {
            if let Some(parent) = data.nodes.get_mut(&parent_id) {
                parent.add_child(child_id);
            }
        }
    }
}

//
// ---------------- Scene ----------------
//

/// Runtime scene, parameterized by a script provider
/// Now holds a reference to the project via Rc<RefCell<Project>>
pub struct Scene<P: ScriptProvider> {
    data: SceneData,
    pub signals: SignalBus,
    queued_signals: Vec<(String, Vec<Value>)>,
    pub scripts: HashMap<Uuid, Rc<RefCell<Box<dyn ScriptObject>>>>,
    pub provider: P,
    pub project: Rc<RefCell<Project>>,
    pub app_command_tx: Option<Sender<AppCommand>>, // NEW field
}

#[derive(Default)]
pub struct SignalBus {
    // "Hit" → [ (script_id, "on_hit"), ... ]
    pub connections: HashMap<String, Vec<SignalConnection>>,
}

#[derive(Clone)]
pub struct SignalConnection {
    pub target_script_id: Uuid,
    pub function_name: String,
}

impl<P: ScriptProvider> Scene<P> {
    /// Create a runtime scene from a root node
    pub fn new(root: SceneNode, provider: P, project: Rc<RefCell<Project>>) -> Self {
        let data = SceneData::new(root);
        Self {
            data,
            signals: SignalBus::default(),
            queued_signals: Vec::new(),
            scripts: HashMap::new(),
            provider,
            project,
            app_command_tx: None,
        }
    }

    /// Create a runtime scene from serialized data
    pub fn from_data(data: SceneData, provider: P, project: Rc<RefCell<Project>>) -> Self {
        Self {
            data,
            signals: SignalBus::default(),
            queued_signals: Vec::new(),
            scripts: HashMap::new(),
            provider,
            project,
            app_command_tx: None,
        }
    }

    /// Load a runtime scene from disk or pak
    pub fn load(res_path: &str, provider: P, project: Rc<RefCell<Project>>) -> io::Result<Self> {
        let data = SceneData::load(res_path)?;
        Ok(Scene::from_data(data, provider, project))
    }

    /// Build a runtime scene from a project with a given provider
    /// Used for StaticScriptProvider (export builds) and also DLL provider (via delegation)
   pub fn from_project_with_provider(
    project: Rc<RefCell<Project>>,
    provider: P,
) -> anyhow::Result<Self> {
    let root_node = SceneNode::Node(Node::new("Root", None));
    let mut game_scene = Scene::new(root_node, provider, project.clone());

    println!("Building scene from project manifest...");

    // ✅ root script first
    let root_script_opt: Option<String> = {
        let proj_ref = game_scene.project.borrow();
        proj_ref.root_script().map(|s| s.to_string())
    };

    println!("Root script path: {:?}", root_script_opt);

    if let Some(root_script_path) = root_script_opt {
        if let Ok(identifier) = script_path_to_identifier(&root_script_path) {
            if let Ok(ctor) = game_scene.provider.load_ctor(&identifier) {
                let root_id = *game_scene.get_root().get_id();
                let handle = Scene::instantiate_script(ctor, root_id, &mut game_scene);
                game_scene.scripts.insert(root_id, handle);
            }
        }
    }

    println!("About to graft main scene...");

    // ✅ main scene second
    let main_scene_path: String = {
        let proj_ref = game_scene.project.borrow();
        proj_ref.main_scene().to_string()
    };

    let loaded_data = SceneData::load(&main_scene_path)?;
    let game_root = *game_scene.get_root().get_id();
    game_scene.graft_data(loaded_data, game_root)?;

    Ok(game_scene)
}


    /// Graft a data scene into this runtime scene
    pub fn graft_data(&mut self, other: SceneData, parent_id: Uuid) -> anyhow::Result<()> {
        for (id, mut node) in other.nodes {
            if id == other.root_id {
                node.set_parent(Some(parent_id));
                self.data.nodes.get_mut(&parent_id).unwrap().add_child(id);
            }
            self.create_node(node)?;
        }
        Ok(())
    }

    fn ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.provider.load_ctor(short)
    }

    pub fn render(&mut self, gfx: &mut Graphics) {
        let dirty_nodes = self.get_dirty_nodes();
        if dirty_nodes.is_empty() {
            return;
        }

        self.traverse_and_render(dirty_nodes, gfx);
    }

    pub fn update(&mut self, delta: f32) {
        // Collect script IDs to avoid borrow checker issues
        let script_ids: Vec<Uuid> = self.scripts.keys().cloned().collect();
        
        for id in script_ids {
            // Borrow project first, before borrowing self
            let project_ref = self.project.clone();
            let mut project_borrow = project_ref.borrow_mut();
            let mut api = ScriptApi::new(delta, self, &mut *project_borrow);
            api.call_update(id);
        }

        self.process_queued_signals();
    }

    
    fn connect_signal(&mut self, signal: &str, target_id: Uuid, function: &str) {
    println!("🔗 Registering connection: signal '{}' → script {} → fn {}()", 
        signal, target_id, function);

        let entry = self.signals.connections
            .entry(signal.to_string())
            .or_default();
        entry.push(SignalConnection {
            target_script_id: target_id,
            function_name: function.to_string(),
        });

         println!("   Total listeners for '{}': {}", signal, entry.len());
    }

    fn queue_signal(&mut self, signal: &str, params: Vec<Value>) {
        println!("📤 Queuing signal '{}' for next frame", signal);
        self.queued_signals.push((signal.to_string(), params));
    }
    
    // Process all queued signals
    fn process_queued_signals(&mut self) {
        if self.queued_signals.is_empty() {
            return;
        }
        
        println!("\n🔄 Processing {} queued signal(s)", self.queued_signals.len());
        
        // Take the queue to avoid borrow issues
        let signals_to_process = std::mem::take(&mut self.queued_signals);
        
        for (signal, params) in signals_to_process {
            self.emit_signal(&signal, params);
        }
    }
    
    // Actually emit the signal (renamed from old emit_signal)
    fn emit_signal(&mut self, signal: &str, params: Vec<Value>) {
        println!("📢 Signal '{}' firing with {} params", signal, params.len());

        let calls_to_make: Vec<SignalConnection> = self.signals.connections
            .get(signal)
            .map(|listeners| listeners.clone())
            .unwrap_or_default();
        
        println!("   Found {} listener(s)", calls_to_make.len());

        for conn in calls_to_make {
            println!("   → Calling {}() on script {}", conn.function_name, conn.target_script_id);
            
            if let Some(script_rc) = self.scripts.get(&conn.target_script_id).cloned() {
                let project_ref = self.project.clone();
                let mut project_borrow = project_ref.borrow_mut();
                let mut api = ScriptApi::new(0f32, self, &mut *project_borrow);
                
                {
                    let mut script = script_rc.borrow_mut();
                    script.call_function(&conn.function_name, &mut api, &params);
                }
                
                println!("   ✓ Successfully called {}()", conn.function_name);
            } else {
                println!("   ✗ Script {} not found!", conn.target_script_id);
            }
        }
        println!("📢 Signal '{}' completed\n", signal);
    }


   pub fn instantiate_script(
        ctor: CreateFn,
        node_id: Uuid,
        scene: &mut Scene<P>,
    ) -> Rc<RefCell<Box<dyn ScriptObject>>> {
        let raw = ctor();
        let mut boxed: Box<dyn ScriptObject> = unsafe { Box::from_raw(raw) };
        boxed.set_node_id(node_id);

        let handle: Rc<RefCell<Box<dyn ScriptObject>>> = Rc::new(RefCell::new(boxed));

        handle
    }

    pub fn create_node(&mut self, mut node: SceneNode) -> anyhow::Result<()> {
        let id = *node.get_id();

        // Handle UI nodes with .fur files
        if let SceneNode::UINode(ref mut ui_node) = node {
            if let Some(fur_path) = &ui_node.fur_path {
                match parse_fur_file(fur_path) {
                    Ok(ast) => {
                        let fur_elements: Vec<FurElement> = ast
                            .into_iter()
                            .filter_map(|fur_node| {
                                if let FurNode::Element(el) = fur_node {
                                    Some(el)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        build_ui_elements_from_fur(ui_node, &fur_elements);
                    }
                    Err(err) => {
                        println!("Error parsing .fur file: {}", err);
                    }
                }
            }
        }

     // Handle script attachment
  // Handle script attachment
// Handle script attachment


    node.mark_dirty();
    self.data.nodes.insert(id, node);
    println!("✅ Node {} fully created\n", id);

   // node is moved already, so get it back immutably from scene
if let Some(node_ref) = self.data.nodes.get(&id) {
    if let Some(script_path) = node_ref.get_script_path().cloned() {
        println!("   ✅ Found script_path: {}", script_path);
        
        let identifier = script_path_to_identifier(&script_path)
            .map_err(|e| anyhow::anyhow!("Invalid script path {}: {}", script_path, e))?;
        let ctor = self.ctor(&identifier)?;

        // Create the script
        let handle = Scene::instantiate_script(ctor, id, self);
        self.scripts.insert(id, handle);

        // Initialize now that node exists
        let project_ref = self.project.clone();
        let mut project_borrow = project_ref.borrow_mut();
        let mut api = ScriptApi::new(0.0, self, &mut *project_borrow);
        api.call_init(id);

        println!("   ✅ Script initialized");
    }
}

    Ok(())
}

    pub fn set_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        val: Value,
    ) -> Option<()> {
        let rc_script = self.scripts.get(node_id)?;
        let mut script = rc_script.borrow_mut();

        script.set_var(name, val)?;
        Some(())
    }



    pub fn get_root(&self) -> &SceneNode {
        &self.data.nodes[&self.data.root_id]
    }

    pub fn get_node<T: 'static>(&self, id: &Uuid) -> Option<&T> {
        self.data
            .nodes
            .get(id)
            .and_then(|node| node.as_any().downcast_ref::<T>())
    }

    pub fn get_node_mut<T: BaseNode + 'static>(&mut self, id: &Uuid) -> Option<&mut T> {
        self.data
            .nodes
            .get_mut(id)
            .and_then(|node| {
                let typed = node.as_any_mut().downcast_mut::<T>()?;
                Some(typed)
            })
    }

    // Remove node and stop rendering
    pub fn remove_node(&mut self, node_id: Uuid, gfx: &mut Graphics) {
        // Stop rendering this node and all its children
        self.stop_rendering_recursive(node_id, gfx);
        
        // Remove from scene
        self.data.nodes.remove(&node_id);
        
        // Remove scripts
        self.scripts.remove(&node_id);
    }
    
    fn stop_rendering_recursive(&self, node_id: Uuid, gfx: &mut Graphics) {
        if let Some(node) = self.data.nodes.get(&node_id) {
            // Stop rendering this node
            gfx.stop_rendering(node_id);
            
            // If it's a UI node, stop rendering all its elements
            if let SceneNode::UINode(ui_node) = node {
                for element in &ui_node.elements {
                    gfx.stop_rendering(*element.0);
                }
            }
            
            // Recursively stop rendering children
            for &child_id in node.get_children() {
                self.stop_rendering_recursive(child_id, gfx);
            }
        }
    }

    // Get dirty nodes for rendering
    fn get_dirty_nodes(&self) -> Vec<Uuid> {
        self.data.nodes
            .iter()
            .filter_map(|(id, node)| {
                if node.is_dirty() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    
    fn traverse_and_render(&mut self, dirty_nodes: Vec<Uuid>, gfx: &mut Graphics) {
    for node_id in dirty_nodes {
        if let Some(node) = self.data.nodes.get_mut(&node_id) {
            match node {
                SceneNode::Sprite2D(sprite) => {
                    if let Some(tex) = &sprite.texture_path {
                        // gfx.draw_texture(
                        //     node_id,
                        //     tex,
                        //     sprite.transform.clone(),
                        //     Vector2::new(0.5, 0.5),
                        // );
                    }
                }
                SceneNode::UINode(ui_node) => {
                    // UI renderer handles layout + rendering internally
                    render_ui(ui_node, gfx);
                }
                _ => {}
            }
            node.set_dirty(false); // Set the dirty flag to false after rendering
        }
    }
}
}

//
// ---------------- SceneAccess impl ----------------
//

impl<P: ScriptProvider> SceneAccess for Scene<P> {
    fn get_scene_node(&mut self, id: Uuid) -> Option<&mut SceneNode> {
        self.data.nodes.get_mut(&id)
    }

        fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.provider.load_ctor(short)
    }

     fn instantiate_script(&mut self, ctor: CreateFn, node_id: Uuid) -> Rc<RefCell<Box<dyn ScriptObject>>> {
        Self::instantiate_script(ctor, node_id, self)
    }

    fn merge_nodes(&mut self, nodes: Vec<SceneNode>) {
        for mut node in nodes {
    
            let id = *node.get_id();
            node.mark_dirty();
        
           if let Some(existing_node) = self.data.nodes.get_mut(&id) {
                *existing_node = node;
            } else {
                println!("Inserting new node with ID {}: {:?} during merge", id, node);
                self.data.nodes.insert(id, node);
            }




        }
    }


    fn set_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        val: Value,
    ) -> Option<()> {
        self.set_script_var(node_id, name, val)
    }

    fn connect_signal(&mut self, signal: &str, target_id: Uuid, function: &str) {
        self.connect_signal(signal, target_id, function);
    }

    fn queue_signal(&mut self, signal: &str, params: Vec<Value>) {
        self.queue_signal(signal, params);
    }

    fn get_script(&self, id: Uuid) -> Option<Rc<RefCell<Box<dyn ScriptObject>>>> {
        self.scripts.get(&id).cloned()
    }

    // NEW method implementation
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>> {
        self.app_command_tx.as_ref()
    }
}

//
// ---------------- Specialization for DllScriptProvider ----------------
//

use libloading::Library;
use crate::registry::DllScriptProvider;

pub fn default_perro_rust_path() -> io::Result<PathBuf> {
    match get_project_root() {
        ProjectRoot::Disk { root, .. } => {
            let profile = "hotreload";

            let mut path = root;
            path.push(".perro");
            path.push("scripts");
            path.push("target");
            path.push(profile);

            let filename = if cfg!(target_os = "windows") {
                "scripts.dll"
            } else if cfg!(target_os = "macos") {
                "libscripts.dylib"
            } else {
                "libscripts.so"
            };

            path.push(filename);
            Ok(path)
        }
        ProjectRoot::Brk { .. } => Err(io::Error::new(
            io::ErrorKind::Other,
            "default_perro_rust_path is not available in release/export mode",
        )),
    }
}


impl Scene<DllScriptProvider> {
    pub fn from_project(project: Rc<RefCell<Project>>) -> anyhow::Result<Self> {
        let root_node = SceneNode::Node(Node::new("Root", None));

        // Load DLL
        let lib_path = default_perro_rust_path()?;
        println!("Loading script library from {:?}", lib_path);
        let lib = unsafe { Library::new(&lib_path)? };
        let provider = DllScriptProvider::new(Some(lib));

        // Borrow project briefly to clone root_path & name
        let root = {
            let project_ref = project.borrow();

            let root_path = project_ref
                .root()
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Project root path not set"))?
                .to_path_buf();

            let project_name = project_ref.name().to_owned();

            ProjectRoot::Disk {
                root: root_path,
                name: project_name,
            }
        };

        // Inject project root into DLL
        provider.inject_project_root(&root)?;

        // Now move `project` into Scene
        let mut game_scene = Scene::new(root_node, provider, project);

        println!("About to graft main scene...");
        let main_scene_path = game_scene.project.borrow().main_scene().to_string();
        let loaded_data = SceneData::load(&main_scene_path)?;
        let game_root = *game_scene.get_root().get_id();
        game_scene.graft_data(loaded_data, game_root)?;

        println!("Building scene from project manifest...");
        // Borrow separately to avoid conflicts with mutable game_scene borrow
        let root_script_path_opt = {
            let project_ref = game_scene.project.borrow();
            project_ref.root_script().map(|s| s.to_string())
        };

        if let Some(root_script_path) = root_script_path_opt {
            if let Ok(identifier) = script_path_to_identifier(&root_script_path) {
                if let Ok(ctor) = game_scene.provider.load_ctor(&identifier) {
                    let root_id = *game_scene.get_root().get_id();
                    let handle = Scene::instantiate_script(ctor, root_id, &mut game_scene);
                    game_scene.scripts.insert(root_id, handle);
                }
                else {
                    println!("❌ Could not find symbol for {}", identifier);
                }
            }
        }

        Ok(game_scene)
    }
}
