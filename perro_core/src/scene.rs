use crate::{
    api::ScriptApi, app_command::AppCommand, apply_fur::{build_ui_elements_from_fur, parse_fur_file}, asset_io::{get_project_root, load_asset, save_asset, ProjectRoot}, ast::{FurElement, FurNode}, lang::transpiler::script_path_to_identifier, manifest::Project, nodes::scene_node::SceneNode, scene_node::BaseNode, script::{CreateFn, SceneAccess, Script, UpdateOp, Var}, ui_element::{BaseElement, UIElement}, ui_renderer::{render_ui, update_ui_layout}, Graphics, Node, ScriptProvider, Sprite2D, Vector2 // NEW import
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
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
    pub scripts: HashMap<Uuid, Rc<RefCell<Box<dyn Script>>>>,
    pub provider: P,
    pub project: Rc<RefCell<Project>>,
    pub app_command_tx: Option<Sender<AppCommand>>, // NEW field
}

impl<P: ScriptProvider> Scene<P> {
    /// Create a runtime scene from a root node
    pub fn new(root: SceneNode, provider: P, project: Rc<RefCell<Project>>) -> Self {
        let data = SceneData::new(root);
        Self {
            data,
            scripts: HashMap::new(),
            provider,
            project,
            app_command_tx: None, // NEW field
        }
    }

    /// Create a runtime scene from serialized data
    pub fn from_data(data: SceneData, provider: P, project: Rc<RefCell<Project>>) -> Self {
        Self {
            data,
            scripts: HashMap::new(),
            provider,
            project,
            app_command_tx: None, // NEW field
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
    }

   pub fn instantiate_script(
        ctor: CreateFn,
        node_id: Uuid,
        scene: &mut Scene<P>,
    ) -> Rc<RefCell<Box<dyn Script>>> {
        let raw = ctor();
        let mut boxed: Box<dyn Script> = unsafe { Box::from_raw(raw) };
        boxed.set_node_id(node_id);

        let handle: Rc<RefCell<Box<dyn Script>>> = Rc::new(RefCell::new(boxed));

        {
            // Clone the Rc before borrowing scene
            let project_ref = scene.project.clone();
            let mut project_borrow = project_ref.borrow_mut();
            let mut api = ScriptApi::new(0.0, scene, &mut *project_borrow);
            handle.borrow_mut().init(&mut api);
        }

        handle
    }

    pub fn create_node(&mut self, mut node: SceneNode) -> anyhow::Result<()> {
        let id = *node.get_id();

        // Handle UI nodes with .fur files
        if let SceneNode::UI(ref mut ui_node) = node {
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
        if let Some(script_path) = node.get_script_path().cloned() {
            let identifier = script_path_to_identifier(&script_path)
                .map_err(|e| anyhow::anyhow!("Invalid script path {}: {}", script_path, e))?;

            let ctor = self.ctor(&identifier)?;
            let handle = Scene::instantiate_script(ctor, id, self);
            self.scripts.insert(id, handle);
        }

        // Mark new nodes as dirty so they get rendered
        node.mark_dirty();
        self.data.nodes.insert(id, node);
        Ok(())
    }

    pub fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()> {
        let rc_script = self.scripts.get(node_id)?;
        let mut script = rc_script.borrow_mut();
        let current = script.get_var(name)?;

        let new_val = match op {
            UpdateOp::Set => val,
            UpdateOp::Add => current + val,
            UpdateOp::Sub => current - val,
            UpdateOp::Mul => current * val,
            UpdateOp::Div => current / val,
            UpdateOp::Rem => current % val,
            UpdateOp::And => current & val,
            UpdateOp::Or => current | val,
            UpdateOp::Xor => current ^ val,
            UpdateOp::Shl => current << val,
            UpdateOp::Shr => current >> val,
        };

        script.set_var(name, new_val)?;
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
            if let SceneNode::UI(ui_node) = node {
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
                    SceneNode::UI(ui_node) => {
                        // UI renderer handles layout + rendering internally
                        render_ui(ui_node, gfx);
                    }
                    _ => {}
                }
                
                // Mark as clean after processing
                node.set_dirty(false);
            }
        }
    }
}

//
// ---------------- SceneAccess impl ----------------
//

impl<P: ScriptProvider> SceneAccess for Scene<P> {
    fn get_scene_node(&mut self, id: &Uuid) -> Option<&mut SceneNode> {
        self.data.nodes.get_mut(id)
    }


    fn merge_nodes(&mut self, nodes: Vec<SceneNode>) {
        for mut node in nodes {
    
            let id = *node.get_id();
            node.mark_dirty();
        
            if let Some(existing_node) = self.data.nodes.get_mut(&id) {
                // Replace the inner data completely
                *existing_node = node;
            } else {
                // Insert new node if it doesn't exist
                self.data.nodes.insert(id, node);
            }
        }
    }


    fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()> {
        self.update_script_var(node_id, name, op, val)
    }

    fn get_script(&self, id: Uuid) -> Option<Rc<RefCell<Box<dyn Script>>>> {
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
        let mut game_scene = Scene::new(root_node, provider, project);
        
        println!("About to graft main scene...");
        // Graft in normal main scene FIRST
        let main_scene_path = game_scene.project.borrow().main_scene().to_string();
        let loaded_data = SceneData::load(&main_scene_path)?;
        let game_root = *game_scene.get_root().get_id();
        game_scene.graft_data(loaded_data, game_root)?;
        
        println!("Building scene from project manifest...");
        // NOW instantiate root script after scene exists
        let root_script_opt = game_scene.project.borrow().root_script().map(|s| s.to_string());
        if let Some(root_script_path) = root_script_opt {
            if let Ok(identifier) = script_path_to_identifier(&root_script_path) {
                if let Ok(ctor) = game_scene.provider.load_ctor(&identifier) {
                    let root_id = *game_scene.get_root().get_id();
                    let handle = Scene::instantiate_script(ctor, root_id, &mut game_scene);
                    game_scene.scripts.insert(root_id, handle);
                }
            }
        }
        
        Ok(game_scene)
    }
}