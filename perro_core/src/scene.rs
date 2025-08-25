use crate::{
    api::ScriptApi, asset_io::{get_project_root, ProjectRoot}, ast::{FurElement, FurNode}, manifest::Project, nodes::scene_node::SceneNode, parse_fur::{build_ui_elements_from_fur, parse_fur_file}, scene_node::BaseNode, script::{CreateFn, SceneAccess, Script, UpdateOp, Var}, ui_element::{BaseElement, UIElement}, ui_renderer::render_ui, Graphics, Node, ScriptProvider, Sprite2D, Vector2
};
use crate::asset_io::{load_asset, save_asset}; // ✅ use asset_io

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs::create_dir_all,
    io,
    path::PathBuf,
    rc::Rc,
};
use uuid::Uuid;
use wgpu::RenderPass;

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
pub struct Scene<P: ScriptProvider> {
    data: SceneData,
    scripts: HashMap<Uuid, Rc<RefCell<Box<dyn Script>>>>,
    provider: P,
}

impl<P: ScriptProvider> Scene<P> {
    /// Create a runtime scene from a root node
    pub fn new(root: SceneNode, provider: P) -> Self {
        let data = SceneData::new(root);
        Self {
            data,
            scripts: HashMap::new(),
            provider,
        }
    }

    /// Create a runtime scene from serialized data
    pub fn from_data(data: SceneData, provider: P) -> Self {
        Self {
            data,
            scripts: HashMap::new(),
            provider,
        }
    }

    /// Load a runtime scene from disk or pak
    pub fn load(res_path: &str, provider: P) -> io::Result<Self> {
        let data = SceneData::load(res_path)?;
        Ok(Scene::from_data(data, provider))
    }

    /// Build a runtime scene from a project manifest with a given provider
    pub fn from_project_with_provider(project: &Project, provider: P) -> anyhow::Result<Self> {
        let root_node = SceneNode::Node(Node::new("Root", None));
        let mut game_scene = Scene::new(root_node, provider);

        // main_scene is a res:// path from project.toml
        let loaded_data = SceneData::load(project.main_scene())?;
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

    pub fn tick(&mut self, gfx: &mut Graphics, pass: &mut RenderPass<'_>, delta: f32) {
        self.process(delta);
        self.render(gfx, pass);
    }

    pub fn process(&mut self, delta: f32) {
        let ids: Vec<Uuid> = self.scripts.keys().cloned().collect();
        for id in ids {
            let mut api = ScriptApi::new(delta, self);
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
            let mut api = ScriptApi::new(0.0, scene);
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
        if let Some(pup_path) = node.get_script_path().cloned() {
            let short = std::path::Path::new(&pup_path)
                .file_stem()
                .unwrap()
                .to_string_lossy();
            println!("  wants script `{short}`");

            let ctor = self.ctor(&short)?;
            println!("  constructor loaded at {:p}", ctor as *const ());

            let handle = Scene::instantiate_script(ctor, id, self);
            self.scripts.insert(id, handle);
            println!("  script instance stored");
        }

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

    pub fn get_node_mut<T: 'static>(&mut self, id: &Uuid) -> Option<&mut T> {
        self.data
            .nodes
            .get_mut(id)
            .and_then(|node| node.as_any_mut().downcast_mut::<T>())
    }

    pub fn traverse<F>(&self, start: Uuid, visit: &mut F)
    where
        F: FnMut(&SceneNode),
    {
        if let Some(node) = self.data.nodes.get(&start) {
            visit(node);
            for &child in node.get_children() {
                self.traverse(child, visit);
            }
        }
    }

    pub fn render(&self, gfx: &mut Graphics, pass: &mut RenderPass<'_>) {
        if !self.data.nodes.contains_key(&self.data.root_id) {
            return;
        }

        self.traverse(self.data.root_id, &mut |node| match node {
            SceneNode::Sprite2D(sprite) if sprite.visible => {
                if let Some(tex) = &sprite.texture_path {
                    gfx.draw_image_in_pass(
                        pass,
                        tex,
                        sprite.transform.clone(),
                        Vector2::new(0.5, 0.5),
                    );
                }
            }
            SceneNode::UI(ui_node) => {
                render_ui(ui_node, gfx, pass);
            }
            _ => {}
        });
    }
}

//
// ---------------- SceneAccess impl ----------------
//

impl<P: ScriptProvider> SceneAccess for Scene<P> {
    fn get_node_mut_any(&mut self, id: &Uuid) -> Option<&mut dyn std::any::Any> {
        self.data.nodes.get_mut(id).map(|node| node.as_any_mut())
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
}

//
// ---------------- Specialization for DllScriptProvider ----------------
//

use libloading::Library;
use crate::registry::DllScriptProvider;

pub fn default_perro_rust_path() -> io::Result<PathBuf> {
    match get_project_root() {
        ProjectRoot::Disk { root, .. } => {
            // In dev/editor mode, we use hotreload profile
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
        ProjectRoot::Pak { .. } => Err(io::Error::new(
            io::ErrorKind::Other,
            "default_perro_rust_path is not available in release/export mode",
        )),
    }
}


impl Scene<DllScriptProvider> {
  pub fn from_project(project: &Project) -> anyhow::Result<Self> {
        let root_node = SceneNode::Node(Node::new("Root", None));

        // ✅ unwrap the Result<PathBuf>
        let lib_path = default_perro_rust_path()?;
        println!("Loading script library from {:?}", lib_path);

        let lib = unsafe { Library::new(&lib_path) }
            .map_err(|e| anyhow::anyhow!("Failed to load script library {:?}: {}", lib_path, e))?;

        let provider = DllScriptProvider::new(Some(lib));
        let mut game_scene = Scene::new(root_node, provider);

        let loaded_data = SceneData::load(project.main_scene())?;
        let game_root = *game_scene.get_root().get_id();
        game_scene.graft_data(loaded_data, game_root)?;

        Ok(game_scene)
    }
}