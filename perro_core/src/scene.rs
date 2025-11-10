use crate::{
    Graphics, Node, api::ScriptApi, app_command::AppCommand, apply_fur::{build_ui_elements_from_fur, parse_fur_file}, asset_io::{ProjectRoot, get_project_root, load_asset, save_asset}, ast::{FurElement, FurNode}, lang::transpiler::script_path_to_identifier, manifest::Project, node_registry::{BaseNode, SceneNode}, prelude::string_to_u64, script::{CreateFn, SceneAccess, Script, ScriptObject, ScriptProvider, UpdateOp, Var}, ui_element::{BaseElement, UIElement}, ui_renderer::render_ui// NEW import
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use smallvec::SmallVec;
use wgpu::RenderPass;
use std::{
    any::Any, cell::RefCell, collections::HashMap, io, path::PathBuf, rc::Rc, str::FromStr, sync::mpsc::Sender, time::{Duration, Instant} // NEW import
};
use uuid::Uuid;

//
// ---------------- SceneData ----------------
//

/// Pure serializable scene data (no runtime state)
#[derive(Serialize, Deserialize, Debug, Clone)]
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



    pub fn fix_relationships(data: &mut SceneData) {
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
    queued_signals: Vec<(u64, SmallVec<[Value; 3]>)>,
    pub scripts: HashMap<Uuid, Rc<RefCell<Box<dyn ScriptObject>>>>,
    pub provider: P,
    pub project: Rc<RefCell<Project>>,
    pub app_command_tx: Option<Sender<AppCommand>>, // NEW field

    pub last_scene_update: Option<Instant>,
    pub delta_accum: f32,
    pub true_updates: i32,
    pub test_val: Value
}

#[derive(Default)]
pub struct SignalBus {
    // signal_id → { script_uuid → SmallVec<[u64; 4]> (function_ids) }
    pub connections: HashMap<u64, HashMap<Uuid, SmallVec<[u64; 4]>>>,
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

            last_scene_update: Some(Instant::now()),
            delta_accum: 0.0,
            true_updates: 0,
            test_val: Value::Null
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

            last_scene_update: Some(Instant::now()),
            delta_accum: 0.0,
            true_updates: 0,
            test_val: Value::Null
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
                let handle = game_scene.instantiate_script(ctor, root_id);
                game_scene.scripts.insert(root_id, handle);

                let project_ref = game_scene.project.clone();
                 let mut project_borrow = project_ref.borrow_mut();

        let now = Instant::now();
        let true_delta = match game_scene.last_scene_update {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0,
        };


        let mut api = ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow);
        api.call_init(root_id);
                
            }
        }
    }

    println!("About to graft main scene...");

 // ✅ main scene second
let main_scene_path: String = {
    let proj_ref = game_scene.project.borrow();
    proj_ref.main_scene().to_string()
};

// measure load
let t_load_start = Instant::now();
let loaded_data = game_scene.provider.load_scene_data(&main_scene_path)?;
let load_time = t_load_start.elapsed();

// measure merge/graft
let t_graft_start = Instant::now();
let game_root = *game_scene.get_root().get_id();
game_scene.merge_scene_data(loaded_data, game_root)?; // <- was graft_data()
let graft_time = t_graft_start.elapsed();

println!(
    "⏱ main scene load: {:>6.2} ms | graft: {:>6.2} ms | total: {:>6.2} ms",
    load_time.as_secs_f64() * 1000.0,
    graft_time.as_secs_f64() * 1000.0,
    (load_time + graft_time).as_secs_f64() * 1000.0
);

    Ok(game_scene)
}

pub fn merge_scene_data(
    &mut self,
    mut other: SceneData,
    parent_id: Uuid,
) -> anyhow::Result<()> {
    // ✅ Super optimized root handling with deferred insertion
    let root_to_insert = {
        let nodes = &mut self.data.nodes;
        
        if let Some(mut root) = other.nodes.remove(&other.root_id) {
            if let Some(parent) = nodes.get_mut(&parent_id) {
                root.set_parent(Some(parent_id));
                parent.add_child(*root.get_id());
            }
            root.mark_dirty();
            Some(root)
        } else {
            eprintln!("⚠️ Merge root missing");
            None
        }
    }; // ← Borrow scope ends here

    // ✅ Include root in processing list
    let mut new_ids: Vec<Uuid> = other.nodes.keys().copied().collect();
    if let Some(ref root) = root_to_insert {
        new_ids.push(*root.get_id());
    }

    // ✅ Super optimized extend
    self.data.nodes.reserve(other.nodes.len() + 1);
    self.data.nodes.extend(other.nodes);
    
    // ✅ Insert root AFTER extend (critical for rendering)
    if let Some(root) = root_to_insert {
        self.data.nodes.insert(*root.get_id(), root);
    }

    // ✅ All your other super optimizations...
    let provider = &self.provider;
    for id in &new_ids {
        if let Some(SceneNode::UINode(u)) = self.data.nodes.get_mut(id) {
            if let Some(fur_path) = &u.fur_path {
                match provider.load_fur_data(fur_path) {
                    Ok(fur_elements) => {
                        build_ui_elements_from_fur(u, &fur_elements);
                        println!(
                            "✅ Loaded FUR for '{}': {} UI elements",
                            u.node.get_name(),
                            u.elements.as_ref().map(|e| e.len()).unwrap_or(0)
                        );
                    }
                    Err(err) => {
                        eprintln!("⚠️ Error loading FUR {:?}: {}", fur_path, err);
                    }
                }
            }
        }
    }

    // ───────────────────────────────────────────────
    // 5️⃣  Attach and initialize scripts for nodes with a script_path
    // ───────────────────────────────────────────────
    let script_targets: Vec<(Uuid, String)> = new_ids
        .iter()
        .filter_map(|id| {
            self.data
                .nodes
                .get(id)
                .and_then(|n| n.get_script_path().map(|p| (*id, p.to_string())))
        })
        .collect();

    // ✅ Single borrow of project for all script initializations
    let project_ref = self.project.clone();
    let mut project_borrow = project_ref.borrow_mut();
    let now = Instant::now();
    let dt = self
        .last_scene_update
        .map(|prev| now.duration_since(prev).as_secs_f32())
        .unwrap_or(0.0);

    for (id, script_path) in script_targets {
        let ident = script_path_to_identifier(&script_path)
            .map_err(|e| anyhow::anyhow!("Invalid script path {}: {}", script_path, e))?;
        let ctor = self.ctor(&ident)?;
        let handle = Self::instantiate_script(ctor, id);
        self.scripts.insert(id, handle);

        // ✅ Reuse same ScriptApi instance across all initializations
        let mut api = ScriptApi::new(dt, self, &mut *project_borrow);
        api.call_init(id);
        println!("✅ Script initialized for node {:?}", id);
    }

    // ───────────────────────────────────────────────
    // 6️⃣  Mark ONLY imported nodes as dirty (so renderer repaints them)
    // ───────────────────────────────────────────────
    for id in new_ids {
        if let Some(node) = self.data.nodes.get_mut(&id) {
            node.mark_dirty();
        }
    }

    println!(
        "📦 Merge complete: now have {} total nodes in scene",
        self.data.nodes.len()
    );

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

       pub fn update(&mut self) {
        let now = Instant::now();
        let true_delta = match self.last_scene_update {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0, // first update
        };
        self.last_scene_update = Some(now);

        // store this dt somewhere for global stats
        self.delta_accum += true_delta;
        self.true_updates += 1;

        if self.delta_accum >= 1.0 {
            let ups = self.true_updates as f32 / self.delta_accum;
            println!(
                "🔹 UPS: {:.2}, Delta: {:.12}, Script Updates: {:.12}",
                ups,
                true_delta,
                self.test_val
            );
            self.delta_accum = 0.0;
            self.true_updates = 0;
        }

        // now use `true_delta` instead of external_delta
        let script_ids: Vec<Uuid> = self.scripts.keys().cloned().collect();

        for id in script_ids {
            let project_ref = self.project.clone();
            let mut project_borrow = project_ref.borrow_mut();
            let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow);
            api.call_update(id);
            let uuid = Uuid::from_str("4f6c6c9c-4e44-4e34-8a9c-0c0f0464fd48").unwrap();
            self.test_val = api.get_script_var(uuid, "script_updates").into();
        }

        self.process_queued_signals();
    }

    
fn connect_signal(&mut self, signal: u64, target_id: Uuid, function_id: u64) {
    println!(
        "🔗 Registering connection: signal '{}' → script {} → fn {}()",
        signal, target_id, function_id
    );

    // Top-level map: signal_id → inner map (script → list of fn ids)
    let script_map = self.signals.connections.entry(signal).or_default();

    // Inner: target script → function list
    let funcs = script_map.entry(target_id).or_default();

    // Avoid duplicate function connections
    if !funcs.iter().any(|&id| id == function_id) {
        funcs.push(function_id);
    }

    println!(
        "   Total listeners for signal '{}': {} script(s), {} total function(s)",
        signal,
        script_map.len(),
        script_map.values().map(|v| v.len()).sum::<usize>()
    );
}

  fn queue_signal(&mut self, signal: u64, params: SmallVec<[Value; 3]>) {
        self.queued_signals.push((signal, params));
    }
    
    // Process all queued signals
       // ✅ OPTIMIZED: Use drain() to reuse Vec allocation
fn process_queued_signals(&mut self) {
    use std::time::Instant;

    if self.queued_signals.is_empty() {
        return;
    }

    let start_total = Instant::now();
    let count = self.queued_signals.len();

    // Drain instead of take – reuses Vec allocation
    let signals: Vec<_> = self.queued_signals.drain(..).collect();

    for (signal, params) in signals {
        self.emit_signal(signal, params);
    }

    let total_elapsed = start_total.elapsed();
    // println!(
    //     "🕓 Completed processing of {count} signal(s) in {:?}\n",
    //     total_elapsed
    // );
}
    
fn emit_signal(&mut self, signal: u64, params: SmallVec<[Value; 3]>) {
    // Copy out listeners before mutable borrow
    let script_map_opt = self.signals.connections.get(&signal);
    if script_map_opt.is_none() {
        return;
    }

    // Clone the minimal subset you need (script_id + function ids)
    let listeners: Vec<(Uuid, SmallVec<[u64; 4]>)> = script_map_opt
        .unwrap()
        .iter()
        .map(|(uuid, fns)| (*uuid, fns.clone()))
        .collect();

    // Now all borrows of self.signals are dropped ✅
    let now = Instant::now();
    let true_delta = self
        .last_scene_update
        .map(|prev| now.duration_since(prev).as_secs_f32())
        .unwrap_or(0.0);

    let project_ref = self.project.clone();
    let mut project_borrow = project_ref.borrow_mut();

    // Safe mutable borrow of self again
    let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow);

    // Emit to all registered scripts
    for (target_id, funcs) in listeners {
        for fn_id in funcs {
            api.call_function_id(target_id, fn_id, &params);
        }
    }
}


   pub fn instantiate_script(
        ctor: CreateFn,
        node_id: Uuid,
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
        let handle = self.instantiate_script(ctor, id);
        self.scripts.insert(id, handle);

        // Initialize now that node exists
        let project_ref = self.project.clone();
        let mut project_borrow = project_ref.borrow_mut();

        let now = Instant::now();
        let true_delta = match self.last_scene_update {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0,
        };


        let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow);
        api.call_init(id);

        println!("   ✅ Script initialized");
    }
}

    Ok(())
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
        // Stop rendering this node itself
        gfx.stop_rendering(node_id);

        // If it's a UI node, stop rendering all of its UI elements
        if let SceneNode::UINode(ui_node) = node {
            if let Some(elements) = &ui_node.elements {
                for (element_id, _) in elements {
                    gfx.stop_rendering(*element_id);
                }
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
        Self::instantiate_script(ctor, node_id)
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


    fn connect_signal_id(&mut self, signal: u64, target_id: Uuid, function: u64) {
        self.connect_signal(signal, target_id, function);
    }

    fn queue_signal_id(&mut self, signal: u64, params: SmallVec<[Value; 3]>) {
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
 let t_load_begin = Instant::now();
        let loaded_data = SceneData::load(&main_scene_path)?;
        let load_time = t_load_begin.elapsed();
        println!(
            "⏱  SceneData::load() completed in {:.03} sec ({} ms)",
            load_time.as_secs_f32(),
            load_time.as_millis()
        );

        // ────────────────────────────────────────────────
        // ⏱  Benchmark: Scene graft
        // ────────────────────────────────────────────────
        let t_graft_begin = Instant::now();
        let game_root = *game_scene.get_root().get_id();
        game_scene.merge_scene_data(loaded_data, game_root)?;
        let graft_time = t_graft_begin.elapsed();
        println!(
            "⏱  Scene grafting completed in {:.03} sec ({} ms)",
            graft_time.as_secs_f32(),
            graft_time.as_millis()
        );

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
                    let handle = game_scene.instantiate_script(ctor, root_id);
                    game_scene.scripts.insert(root_id, handle);

                    
                let project_ref = game_scene.project.clone();
                 let mut project_borrow = project_ref.borrow_mut();

        let now = Instant::now();
        let true_delta = match game_scene.last_scene_update {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0,
        };


        let mut api = ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow);
        api.call_init(root_id);
                }
                else {
                    println!("❌ Could not find symbol for {}", identifier);
                }
            }
        }

        Ok(game_scene)
    }
}
