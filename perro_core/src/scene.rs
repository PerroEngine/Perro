use crate::{
    Graphics,
    Node,
    RenderLayer,
    ShapeType2D,
    Transform3D,
    Vector2,
    api::ScriptApi,
    app_command::AppCommand,
    apply_fur::{build_ui_elements_from_fur, parse_fur_file},
    asset_io::{ProjectRoot, get_project_root, load_asset, save_asset},
    fur_ast::{FurElement, FurNode},
    input::joycon::ControllerManager,
    input::manager::InputManager,
    manifest::Project,
    node_registry::{BaseNode, SceneNode},
    physics::physics_2d::PhysicsWorld2D,
    prelude::string_to_u64,
    script::{CreateFn, SceneAccess, Script, ScriptObject, ScriptProvider, Var},
    transpiler::script_path_to_identifier,
    ui_element::{BaseElement, UIElement},
    ui_renderer::render_ui, // NEW import
};
use std::sync::Mutex;

use glam::{Mat4, Vec3};
use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeStruct};
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    any::Any,
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    io,
    path::PathBuf,
    rc::Rc,
    str::FromStr,
    sync::mpsc::Sender,
    time::{Duration, Instant}, // NEW import
};
use uuid::Uuid;
use wgpu::RenderPass;

//
// ---------------- SceneData ----------------
//

/// Pure serializable scene data (no runtime state)
#[derive(Debug)]
pub struct SceneData {
    pub root_id: Uuid,
    pub nodes: IndexMap<Uuid, SceneNode>,
}

impl Clone for SceneData {
    fn clone(&self) -> Self {
        Self {
            root_id: self.root_id,
            nodes: self.nodes.iter().map(|(id, node)| {
                (*id, node.clone())
            }).collect(),
        }
    }
}

impl Serialize for SceneData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut state = serializer.serialize_struct("SceneData", 2)?;
        state.serialize_field("root_id", &self.root_id)?;
        
        struct NodesMap<'a> {
            nodes: &'a IndexMap<Uuid, SceneNode>,
        }
        
        impl<'a> Serialize for NodesMap<'a> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(self.nodes.len()))?;
                for (id, node) in self.nodes.iter() {
                    map.serialize_entry(&node.get_local_id(), node)?;
                }
                map.end()
            }
        }
        
        state.serialize_field("nodes", &NodesMap { nodes: &self.nodes })?;
        state.end()
    }
}
impl<'de> Deserialize<'de> for SceneData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawSceneData {
            root_id: Uuid,
            nodes: IndexMap<Uuid, SceneNode>,
            #[serde(default)]
            node_count: Option<usize>,
        }

        let raw = RawSceneData::deserialize(deserializer)?;
        let capacity = raw.node_count.unwrap_or(raw.nodes.len());

        // We’ll use this helper closure so the logic is consistent between small/large scenes
        let process_nodes = |raw_nodes: IndexMap<Uuid, SceneNode>| -> IndexMap<Uuid, SceneNode> {
            let mut nodes = IndexMap::with_capacity(capacity);
            let mut parent_children: IndexMap<Uuid, Vec<Uuid>> =
                IndexMap::with_capacity(capacity / 4);

            for (local_id, mut node) in raw_nodes {
                // Treat map key (the serialized Uuid) as this node's *local id*,
                // not its runtime UUID. This preserves deterministic structure.
                node.set_local_id(local_id);
                node.clear_children();
                
                // Mark transform as dirty for Node2D nodes after deserialization
                // (transform_dirty is skipped during serialization, so it defaults to false)
                node.mark_transform_dirty_if_node2d();

                let parent_local = node.get_parent();
                if !parent_local.is_nil() {
                    parent_children
                        .entry(parent_local)
                        .or_default()
                        .push(local_id);
                }

                nodes.insert(local_id, node);
            }

            // Rebuild relationships deterministically
            for (parent_id, children) in parent_children {
                if let Some(parent) = nodes.get_mut(&parent_id) {
                    for child_id in children {
                        parent.add_child(child_id);
                    }
                }
            }

            nodes
        };

        // If large enough, parallelize basic deserialization (not relationships)
        let nodes = if raw.nodes.len() > 100 {
            use rayon::prelude::*;
            let nodes_vec: Vec<(Uuid, SceneNode)> = raw.nodes.into_par_iter().collect();
            let mut nodes = IndexMap::with_capacity(capacity);
            for (local_id, mut node) in nodes_vec {
                node.set_local_id(local_id);
                node.clear_children();
                // Mark transform as dirty for Node2D nodes after deserialization
                node.mark_transform_dirty_if_node2d();
                nodes.insert(local_id, node);
            }
            nodes
        } else {
            process_nodes(raw.nodes)
        };

        Ok(SceneData {
            root_id: raw.root_id,
            nodes,
        })
    }
}

impl SceneData {
    /// Create a new data scene with a root node
    pub fn new(root: SceneNode) -> Self {
        let root_id = root.get_id();
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
        let data: SceneData =
            serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(data)
    }

    pub fn fix_relationships(data: &mut SceneData) {
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

        for (&node_id, node) in data.nodes.iter_mut() {
            node.set_id(node_id);
            node.get_children_mut().clear();

            // Collect relationships during same iteration
            let parent_id = node.get_parent();
            if !parent_id.is_nil() {
                parent_children.entry(parent_id).or_default().push(node_id);
            }
        }

        // Apply batched relationships
        for (parent_id, children) in parent_children {
            if let Some(parent) = data.nodes.get_mut(&parent_id) {
                for child_id in children {
                    parent.add_child(child_id);
                }
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
    pub(crate) data: SceneData,
    pub signals: SignalBus,
    queued_signals: Vec<(u64, SmallVec<[Value; 3]>)>,
    pub scripts: HashMap<Uuid, Box<dyn ScriptObject>>,
    pub provider: P,
    pub project: Rc<RefCell<Project>>,
    pub app_command_tx: Option<Sender<AppCommand>>, // NEW field
    pub controller_manager: Mutex<ControllerManager>, // Controller input manager
    pub input_manager: Mutex<InputManager>,         // Keyboard/mouse input manager

    pub last_scene_update: Option<Instant>,
    pub delta_accum: f32,
    pub true_updates: i32,

    // Fixed update timing
    pub fixed_update_accumulator: f32,
    pub last_fixed_update: Option<Instant>,
    // Optimize: Use HashSet for O(1) contains() checks (order doesn't matter for fixed updates)
    pub nodes_with_internal_fixed_update: HashSet<Uuid>,

    // Physics (wrapped in RefCell for interior mutability through trait objects)
    pub physics_2d: std::cell::RefCell<PhysicsWorld2D>,
    
    // OPTIMIZED: Cache script IDs to avoid Vec allocation every frame
    cached_script_ids: Vec<Uuid>,
    scripts_dirty: bool,
    
    // OPTIMIZED: Separate vectors for scripts with update/fixed_update to avoid checking all scripts
    scripts_with_update: Vec<Uuid>,
    scripts_with_fixed_update: Vec<Uuid>,
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
            controller_manager: Mutex::new(ControllerManager::new()),
            input_manager: Mutex::new(InputManager::new()),

            last_scene_update: Some(Instant::now()),
            delta_accum: 0.0,
            true_updates: 0,
            fixed_update_accumulator: 0.0,
            last_fixed_update: Some(Instant::now()),
            nodes_with_internal_fixed_update: HashSet::new(),
            physics_2d: std::cell::RefCell::new(PhysicsWorld2D::new()),
            
            // OPTIMIZED: Initialize script ID cache
            cached_script_ids: Vec::new(),
            scripts_dirty: true,
            
            // OPTIMIZED: Initialize separate vectors for update/fixed_update scripts
            scripts_with_update: Vec::new(),
            scripts_with_fixed_update: Vec::new(),
        }
    }

    /// Create a runtime scene from serialized data
    pub fn from_data(mut data: SceneData, provider: P, project: Rc<RefCell<Project>>) -> Self {
        // Mark all nodes as dirty and transform_dirty when loading from data
        for node in data.nodes.values_mut() {
            node.mark_dirty();
            node.mark_transform_dirty_if_node2d();
        }
        
        Self {
            data,
            signals: SignalBus::default(),
            queued_signals: Vec::new(),
            scripts: HashMap::new(),
            physics_2d: std::cell::RefCell::new(PhysicsWorld2D::new()),
            provider,
            project,
            app_command_tx: None,
            controller_manager: Mutex::new(ControllerManager::new()),
            input_manager: Mutex::new(InputManager::new()),

            last_scene_update: Some(Instant::now()),
            delta_accum: 0.0,
            true_updates: 0,
            fixed_update_accumulator: 0.0,
            last_fixed_update: Some(Instant::now()),
            nodes_with_internal_fixed_update: HashSet::new(),
            
            // OPTIMIZED: Initialize script ID cache
            cached_script_ids: Vec::new(),
            scripts_dirty: true,
            
            // OPTIMIZED: Initialize separate vectors for update/fixed_update scripts
            scripts_with_update: Vec::new(),
            scripts_with_fixed_update: Vec::new(),
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
        let mut root_node = Node::new();
        root_node.name = Cow::Borrowed("Root");
        let root_node = SceneNode::Node(root_node);
        let mut game_scene = Scene::new(root_node, provider, project.clone());

        // Initialize input manager with action map from project.toml
        {
            let project_ref = game_scene.project.borrow();
            let input_map = project_ref.get_input_map();
            let mut input_mgr = game_scene.input_manager.lock().unwrap();
            input_mgr.load_action_map(input_map);
        }

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
                    let root_id = game_scene.get_root().get_id();
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
        let game_root = game_scene.get_root().get_id();
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

    fn remap_uuids_in_json_value(value: &mut serde_json::Value, id_map: &HashMap<Uuid, Uuid>) {
        match value {
            serde_json::Value::String(s) => {
                if let Ok(uuid) = Uuid::parse_str(s) {
                    if let Some(&new_uuid) = id_map.get(&uuid) {
                        *s = new_uuid.to_string();
                    }
                }
            }
            serde_json::Value::Object(obj) => {
                // Parallel processing for large objects
                if obj.len() > 10 {
                    let mut entries: Vec<_> = obj.iter_mut().collect();
                    entries.par_iter_mut().for_each(|(_, v)| {
                        Self::remap_uuids_in_json_value(v, id_map);
                    });
                } else {
                    for (_, v) in obj.iter_mut() {
                        Self::remap_uuids_in_json_value(v, id_map);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                // Parallel processing for large arrays
                if arr.len() > 20 {
                    arr.par_iter_mut().for_each(|v| {
                        Self::remap_uuids_in_json_value(v, id_map);
                    });
                } else {
                    for v in arr.iter_mut() {
                        Self::remap_uuids_in_json_value(v, id_map);
                    }
                }
            }
            _ => {}
        }
    }

    fn remap_script_exp_vars_uuids(
        script_exp_vars: &mut HashMap<String, serde_json::Value>,
        id_map: &HashMap<Uuid, Uuid>,
    ) {
        if script_exp_vars.len() > 5 {
            let mut entries: Vec<_> = script_exp_vars.iter_mut().collect();
            entries.par_iter_mut().for_each(|(_, value)| {
                Self::remap_uuids_in_json_value(value, id_map);
            });
        } else {
            for (_, value) in script_exp_vars.iter_mut() {
                Self::remap_uuids_in_json_value(value, id_map);
            }
        }
    }
    pub fn merge_scene_data(
        &mut self,
        mut other: SceneData,
        parent_id: Uuid,
    ) -> anyhow::Result<()> {
        use std::time::Instant;
    
        let merge_start = Instant::now();
    
        // ───────────────────────────────────────────────
        // 1️⃣ BUILD LOCAL → NEW RUNTIME ID MAP
        // ───────────────────────────────────────────────
        let id_map_start = Instant::now();
        let mut id_map: HashMap<Uuid, Uuid> = HashMap::with_capacity(other.nodes.len() + 1);
    
        const ID_MAP_PARALLEL_THRESHOLD: usize = 50;
    
        if other.nodes.len() >= ID_MAP_PARALLEL_THRESHOLD {
            let local_ids: Vec<Uuid> = other.nodes.keys().copied().collect();
            let new_ids: Vec<(Uuid, Uuid)> = local_ids
                .par_iter()
                .map(|&local_id| (local_id, Uuid::new_v4()))
                .collect();
            id_map.extend(new_ids);
        } else {
            for node in other.nodes.values() {
                id_map.insert(node.get_local_id(), Uuid::new_v4());
            }
        }
    
        // Ensure root is included
        if let Some(root_node) = other.nodes.get(&other.root_id) {
            id_map
                .entry(root_node.get_local_id())
                .or_insert_with(Uuid::new_v4);
        }
    
        let id_map_time = id_map_start.elapsed();
        println!(
            "⏱ ID map creation: {:.2} ms",
            id_map_time.as_secs_f64() * 1000.0
        );
    
        // ───────────────────────────────────────────────
        // 2️⃣ REMAP NODES AND BUILD RELATIONSHIPS
        // ───────────────────────────────────────────────
        let remap_start = Instant::now();
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    
        // Get the subscene root's NEW runtime ID
        let subscene_root_local_id = other.root_id;
        let subscene_root_new_id = other
            .nodes
            .get(&subscene_root_local_id)
            .map(|n| id_map[&n.get_local_id()]);
        
        println!(
            "📍 Subscene root: local={}, new={:?}, will attach to parent={}",
            subscene_root_local_id, subscene_root_new_id, parent_id
        );
        
        // Check if root has is_root_of (determines if we skip the root later)
        let root_is_root_of = other
            .nodes
            .get(&other.root_id)
            .and_then(|n| Self::get_is_root_of(n));
    
        let skip_root_id: Option<Uuid> = if root_is_root_of.is_some() {
            println!("🔗 Root has is_root_of, will be skipped and replaced");
            subscene_root_new_id
        } else {
            None
        };
    
        // Remap all nodes
        for (_local_id, node) in other.nodes.iter_mut() {
            let local_id = node.get_local_id();
            let new_id = id_map[&local_id];
            node.set_id(new_id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Determine parent relationship
            let parent_local = node.get_parent();
            if !parent_local.is_nil() {
                // Parent is specified in JSON - remap it
                if let Some(&mapped_parent) = id_map.get(&parent_local) {
                    node.set_parent(Some(mapped_parent));
                    parent_children
                        .entry(mapped_parent)
                        .or_default()
                        .push(new_id);
                } else {
                    // Parent not in subscene - might be in main scene already
                    // Keep original parent reference
                    node.set_parent(Some(parent_local));
                    parent_children.entry(parent_local).or_default().push(new_id);
                }
            } else if Some(new_id) == subscene_root_new_id {
                // This is the subscene root with no parent - attach to game's parent_id
                // But only if we're NOT skipping it (is_root_of case)
                if skip_root_id.is_none() {
                    println!(
                        "🔗 Attaching subscene root {} to game parent {}",
                        new_id, parent_id
                    );
                    node.set_parent(Some(parent_id));
                    parent_children.entry(parent_id).or_default().push(new_id);
                }
            }
            // else: node has no parent and isn't root - leave as orphan (shouldn't happen normally)
    
            // Handle script_exp_vars
            if let Some(mut script_vars) = node.get_script_exp_vars() {
                Self::remap_script_exp_vars_uuids(&mut script_vars, &id_map);
                node.set_script_exp_vars(Some(script_vars));
            }
        }
    
        // Apply parent-child relationships to nodes in `other`
        for (parent_new_id, children) in &parent_children {
            // Skip if this parent is in the main scene (will handle after insertion)
            if *parent_new_id == parent_id {
                continue;
            }
    
            // Find parent in other.nodes by its new_id
            for (_, node) in other.nodes.iter_mut() {
                if node.get_id() == *parent_new_id {
                    for child_id in children {
                        if !node.get_children().contains(child_id) {
                            node.add_child(*child_id);
                        }
                    }
                    break;
                }
            }
        }
    
        let remap_time = remap_start.elapsed();
        println!(
            "⏱ Node remapping: {:.2} ms",
            remap_time.as_secs_f64() * 1000.0
        );
    
        // ───────────────────────────────────────────────
        // 3️⃣ INSERT NODES INTO MAIN SCENE
        // ───────────────────────────────────────────────
        let insert_start = Instant::now();
        self.data.nodes.reserve(other.nodes.len() + 1);
    
        let mut inserted_ids: Vec<Uuid> = Vec::with_capacity(other.nodes.len());
    
        for mut node in other.nodes.into_values() {
            let node_id = node.get_id();

            // Skip root if it has is_root_of (will be replaced by nested scene content)
            if let Some(skip_id) = skip_root_id {
                if node_id == skip_id {
                    println!("⏭️ Skipping root node (runtime_id={})", skip_id);
                    continue;
                }
            }

            node.mark_dirty();
            node.mark_transform_dirty_if_node2d();

            // Resolve name conflicts (check siblings AND parent/ancestor)
            let node_name = node.get_name();
            let parent_id = node.get_parent();
            
            // Check if name conflicts with siblings OR with parent/ancestors
            let has_sibling_conflict = self.has_sibling_name_conflict(parent_id, node_name);
            let has_parent_conflict = self.has_parent_or_ancestor_name_conflict(parent_id, node_name);
            
            if has_sibling_conflict || has_parent_conflict {
                let resolved_name = self.resolve_name_conflict(parent_id, node_name);
                Self::set_node_name(&mut node, resolved_name);
            }

            self.data.nodes.insert(node_id, node);
            inserted_ids.push(node_id);
    
            // Register node for internal fixed updates if needed
            if let Some(node_ref) = self.data.nodes.get(&node_id) {
                if node_ref.needs_internal_fixed_update() {
                    // Optimize: HashSet insert is O(1) and handles duplicates automatically
                    self.nodes_with_internal_fixed_update.insert(node_id);
                }
            }
        }
    
        // Update the GAME's parent node to include new children
        if let Some(children_of_game_parent) = parent_children.get(&parent_id) {
            if let Some(game_parent) = self.data.nodes.get_mut(&parent_id) {
                for child_id in children_of_game_parent {
                    if !game_parent.get_children().contains(child_id) {
                        game_parent.add_child(*child_id);
                        println!("✅ Added {} as child of game parent {}", child_id, parent_id);
                    }
                }
            }
        }
    
        // Mark transforms dirty for all inserted nodes
        for id in &inserted_ids {
            self.mark_transform_dirty_recursive(*id);
        }
    
        let insert_time = insert_start.elapsed();
        println!(
            "⏱ Node insertion: {:.2} ms ({} nodes)",
            insert_time.as_secs_f64() * 1000.0,
            inserted_ids.len()
        );
    
        // ───────────────────────────────────────────────
        // 4️⃣ HANDLE is_root_of SCENE REFERENCES (RECURSIVE)
        // ───────────────────────────────────────────────
        let nested_scene_start = Instant::now();
        let mut nodes_with_nested_scenes: Vec<(Uuid, String)> = Vec::new();
    
        // Collect nodes with is_root_of from newly inserted nodes
        for id in &inserted_ids {
            if let Some(node) = self.data.nodes.get(id) {
                if let Some(scene_path) = Self::get_is_root_of(node) {
                    nodes_with_nested_scenes.push((*id, scene_path));
                }
            }
        }
    
        // Recursively load and merge nested scenes
        let nested_scene_count = nodes_with_nested_scenes.len();
        for (parent_node_id, scene_path) in nodes_with_nested_scenes {
            println!(
                "🔗 Recursively merging nested scene: {} (parent={})",
                scene_path, parent_node_id
            );
    
            // Load the nested scene
            if let Ok(nested_scene_data) = self.provider.load_scene_data(&scene_path) {
                // Merge with the node as parent - nested scene's root becomes child of this node
                if let Err(e) = self.merge_scene_data_with_root_replacement(
                    nested_scene_data,
                    parent_node_id,
                ) {
                    eprintln!("⚠️ Error merging nested scene '{}': {}", scene_path, e);
                }
            } else {
                eprintln!("⚠️ Failed to load nested scene: {}", scene_path);
            }
        }
    
        let nested_scene_time = nested_scene_start.elapsed();
        if nested_scene_count > 0 {
            println!(
                "⏱ Nested scene loading: {:.2} ms ({} scene(s))",
                nested_scene_time.as_secs_f64() * 1000.0,
                nested_scene_count
            );
        }
    
        // ───────────────────────────────────────────────
        // 5️⃣ REGISTER COLLISION SHAPES WITH PHYSICS
        // ───────────────────────────────────────────────
        let physics_start = Instant::now();
        self.register_collision_shapes(&inserted_ids);
        let physics_time = physics_start.elapsed();
        println!(
            "⏱ Physics registration: {:.2} ms",
            physics_time.as_secs_f64() * 1000.0
        );
    
        // ───────────────────────────────────────────────
        // 6️⃣ FUR LOADING (UI FILES)
        // ───────────────────────────────────────────────
        let fur_start = Instant::now();
        const FUR_PARALLEL_THRESHOLD: usize = 5;
    
        // Collect FUR paths
        let fur_paths: Vec<(Uuid, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.data.nodes.get(id).and_then(|node| {
                    if let SceneNode::UINode(u) = node {
                        u.fur_path.as_ref().map(|path| (*id, path.to_string()))
                    } else {
                        None
                    }
                })
            })
            .collect();
    
        // Load FUR data
        let fur_loads: Vec<(Uuid, Result<Vec<FurElement>, _>)> =
            if fur_paths.len() >= FUR_PARALLEL_THRESHOLD {
                fur_paths
                    .par_iter()
                    .map(|(id, fur_path)| {
                        let result = self.provider.load_fur_data(fur_path);
                        (*id, result)
                    })
                    .collect()
            } else {
                fur_paths
                    .iter()
                    .map(|(id, fur_path)| {
                        let result = self.provider.load_fur_data(fur_path);
                        (*id, result)
                    })
                    .collect()
            };
    
        // Apply FUR results
        for (id, result) in fur_loads {
            if let Some(node) = self.data.nodes.get_mut(&id) {
                if let SceneNode::UINode(u) = node {
                    match result {
                        Ok(fur_elements) => build_ui_elements_from_fur(u, &fur_elements),
                        Err(err) => eprintln!("⚠️ Error loading FUR for {}: {}", id, err),
                    }
                }
            }
        }
    
        let fur_time = fur_start.elapsed();
    
        // ───────────────────────────────────────────────
        // 7️⃣ SCRIPT INITIALIZATION
        // ───────────────────────────────────────────────
        let script_start = Instant::now();
    
        // Collect script paths
        let script_targets: Vec<(Uuid, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.data
                    .nodes
                    .get(id)
                    .and_then(|n| n.get_script_path().map(|p| (*id, p.to_string())))
            })
            .collect();
    
        // Initialize scripts
        if !script_targets.is_empty() {
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
                
                // Check flags and add to appropriate vectors
                let flags = handle.script_flags();
                
                if flags.has_update() && !self.scripts_with_update.contains(&id) {
                    self.scripts_with_update.push(id);
                }
                if flags.has_fixed_update() && !self.scripts_with_fixed_update.contains(&id) {
                    self.scripts_with_fixed_update.push(id);
                }
                
                self.scripts.insert(id, handle);
                self.scripts_dirty = true;
    
                let mut api = ScriptApi::new(dt, self, &mut *project_borrow);
                api.call_init(id);
            }
        }
    
        let script_time = script_start.elapsed();
        let total_time = merge_start.elapsed();
    
        // ───────────────────────────────────────────────
        // 8️⃣ PERFORMANCE SUMMARY
        // ───────────────────────────────────────────────
        println!(
            "📦 Merge complete: {} total nodes (+{} new)",
            self.data.nodes.len(),
            inserted_ids.len()
        );
    
        println!(
            "⏱ Timing: total={:.2}ms | id_map={:.2}ms | remap={:.2}ms | insert={:.2}ms | fur={:.2}ms | scripts={:.2}ms",
            total_time.as_secs_f64() * 1000.0,
            id_map_time.as_secs_f64() * 1000.0,
            remap_time.as_secs_f64() * 1000.0,
            insert_time.as_secs_f64() * 1000.0,
            fur_time.as_secs_f64() * 1000.0,
            script_time.as_secs_f64() * 1000.0,
        );
    
        Ok(())
    }
    
    /// Merge a nested scene where the nested scene's root REPLACES an existing node
    /// (used for is_root_of scenarios)
    fn merge_scene_data_with_root_replacement(
        &mut self,
        mut other: SceneData,
        replacement_root_id: Uuid,
    ) -> anyhow::Result<()> {
        println!(
            "🔄 merge_scene_data_with_root_replacement: replacement_root={}",
            replacement_root_id
        );
    
        // Build a FRESH id_map for this nested scene
        let mut id_map: HashMap<Uuid, Uuid> = HashMap::with_capacity(other.nodes.len());
    
        // Generate UUIDs for all nodes EXCEPT the root
        let subscene_root_local_id = other.root_id;
    
        for node in other.nodes.values() {
            let local_id = node.get_local_id();
            if local_id == subscene_root_local_id {
                // Root maps to the replacement node (which already exists)
                id_map.insert(local_id, replacement_root_id);
            } else {
                id_map.insert(local_id, Uuid::new_v4());
            }
        }
    
        println!(
            "   ✅ Mapped nested scene root {} → {}",
            subscene_root_local_id, replacement_root_id
        );
    
        // Build parent-children relationships
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    
        // Remap all nodes
        for (_local_id, node) in other.nodes.iter_mut() {
            let local_id = node.get_local_id();
            let new_id = id_map[&local_id];
            node.set_id(new_id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Remap parent
            let parent_local = node.get_parent();
            if !parent_local.is_nil() {
                if let Some(&mapped_parent) = id_map.get(&parent_local) {
                    node.set_parent(Some(mapped_parent));
                    parent_children
                        .entry(mapped_parent)
                        .or_default()
                        .push(new_id);
                }
            }
            // If no parent (this is the subscene root), its parent is already set
            // in the main scene (the node with is_root_of)

            // Remap script_exp_vars
            if let Some(mut script_vars) = node.get_script_exp_vars() {
                Self::remap_script_exp_vars_uuids(&mut script_vars, &id_map);
                node.set_script_exp_vars(Some(script_vars));
            }
        }
    
        // Apply parent-child relationships within the subscene
        for (parent_new_id, children) in &parent_children {
            // If parent is the replacement root, update the existing node in main scene
            if *parent_new_id == replacement_root_id {
                if let Some(existing_node) = self.data.nodes.get_mut(&replacement_root_id) {
                    for child_id in children {
                        if !existing_node.get_children().contains(child_id) {
                            existing_node.add_child(*child_id);
                        }
                    }
                }
            } else {
                // Parent is in the subscene - find and update it
                for (_, node) in other.nodes.iter_mut() {
                    if node.get_id() == *parent_new_id {
                        for child_id in children {
                            if !node.get_children().contains(child_id) {
                                node.add_child(*child_id);
                            }
                        }
                        break;
                    }
                }
            }
        }
    
        // Insert all nodes EXCEPT the root (which already exists as replacement_root_id)
        let mut inserted_ids: Vec<Uuid> = Vec::new();
    
        for mut node in other.nodes.into_values() {
            let node_id = node.get_id();
    
            // Skip the root - it already exists in the main scene
            if node_id == replacement_root_id {
                println!("   ⏭️ Skipping nested scene root (already exists as replacement)");
                continue;
            }
    
            node.mark_dirty();
            node.mark_transform_dirty_if_node2d();
    
            // Resolve name conflicts (only check siblings - nodes with the same parent)
            let node_name = node.get_name();
            let parent_id = node.get_parent();
            if self.has_sibling_name_conflict(parent_id, node_name) {
                let resolved_name = self.resolve_name_conflict(parent_id, node_name);
                Self::set_node_name(&mut node, resolved_name);
            }
    
            self.data.nodes.insert(node_id, node);
            inserted_ids.push(node_id);
    
            // Register for internal fixed updates if needed
            if let Some(node_ref) = self.data.nodes.get(&node_id) {
                if node_ref.needs_internal_fixed_update() {
                    // Optimize: HashSet insert is O(1) and handles duplicates automatically
                    self.nodes_with_internal_fixed_update.insert(node_id);
                }
            }
        }
    
        // Mark transforms dirty
        for id in &inserted_ids {
            self.mark_transform_dirty_recursive(*id);
        }
    
        // Register collision shapes
        self.register_collision_shapes(&inserted_ids);
    
        // Load FUR files for UI nodes
        // Optimize: use as_ref() instead of clone() for Option<String>
        for id in &inserted_ids {
            if let Some(node) = self.data.nodes.get_mut(id) {
                if let SceneNode::UINode(ui_node) = node {
                    if let Some(fur_path) = ui_node.fur_path.as_ref() {
                        if let Ok(fur_elements) = self.provider.load_fur_data(fur_path) {
                            build_ui_elements_from_fur(ui_node, &fur_elements);
                        }
                    }
                }
            }
        }
    
        // Initialize scripts
        let script_targets: Vec<(Uuid, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.data
                    .nodes
                    .get(id)
                    .and_then(|n| n.get_script_path().map(|p: &str| (*id, p.to_string())))
            })
            .collect();
    
        if !script_targets.is_empty() {
            let project_ref = self.project.clone();
            let mut project_borrow = project_ref.borrow_mut();
            let now = std::time::Instant::now();
            let dt = self
                .last_scene_update
                .map(|prev| now.duration_since(prev).as_secs_f32())
                .unwrap_or(0.0);
    
            for (id, script_path) in script_targets {
                if let Ok(ident) = script_path_to_identifier(&script_path) {
                    if let Ok(ctor) = self.ctor(&ident) {
                        let handle = Self::instantiate_script(ctor, id);
                        
                        // Check flags and add to appropriate vectors
                        let flags = handle.script_flags();
                        
                        if flags.has_update() && !self.scripts_with_update.contains(&id) {
                            self.scripts_with_update.push(id);
                        }
                        if flags.has_fixed_update() && !self.scripts_with_fixed_update.contains(&id) {
                            self.scripts_with_fixed_update.push(id);
                        }
                        
                        self.scripts.insert(id, handle);
                        self.scripts_dirty = true;
    
                        let mut api = ScriptApi::new(dt, self, &mut *project_borrow);
                        api.call_init(id);
                    }
                }
            }
        }
    
        // ───────────────────────────────────────────────
        // HANDLE NESTED is_root_of SCENE REFERENCES (RECURSIVE)
        // ───────────────────────────────────────────────
        let mut nodes_with_nested_scenes: Vec<(Uuid, String)> = Vec::new();
    
        for id in &inserted_ids {
            if let Some(node) = self.data.nodes.get(id) {
                if let Some(scene_path) = Self::get_is_root_of(node) {
                    nodes_with_nested_scenes.push((*id, scene_path));
                }
            }
        }
    
        for (parent_node_id, scene_path) in nodes_with_nested_scenes {
            println!(
                "🔗 [Nested] Recursively merging scene: {} (parent={})",
                scene_path, parent_node_id
            );
    
            if let Ok(nested_scene_data) = self.provider.load_scene_data(&scene_path) {
                if let Err(e) =
                    self.merge_scene_data_with_root_replacement(nested_scene_data, parent_node_id)
                {
                    eprintln!("⚠️ Error merging nested scene '{}': {}", scene_path, e);
                }
            } else {
                eprintln!("⚠️ Failed to load nested scene: {}", scene_path);
            }
        }
    
        println!(
            "   ✅ Nested scene merge complete: {} nodes inserted",
            inserted_ids.len()
        );
    
        Ok(())
    }
    

    pub fn print_scene_tree(&self) {
        println!("\n🌳 Scene Tree Structure:");
        
        // Find all root nodes (nodes with no parent)
        let root_nodes: Vec<Uuid> = self.data.nodes
            .iter()
            .filter_map(|(id, node)| {
                if node.get_parent().is_nil() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        
        // Print each root and its children recursively
        for root_id in root_nodes {
            self.print_node_recursive(root_id, 0, true);
        }
        
        println!();
    }

    fn print_node_recursive(&self, node_id: Uuid, depth: usize, is_last: bool) {
        if let Some(node) = self.data.nodes.get(&node_id) {
            // Build the tree characters
            let prefix = if depth == 0 {
                String::new()
            } else {
                let mut p = String::new();
                for _ in 0..(depth - 1) {
                    p.push_str("│   ");
                }
                if is_last {
                    p.push_str("└── ");
                } else {
                    p.push_str("├── ");
                }
                p
            };
            
            // Get node info
            let name = node.get_name();
            let has_script = node.get_script_path().is_some();
            let script_tag = if has_script { " [HAS SCRIPT]" } else { "" };

            
            // Print this node with MORE DEBUG INFO
            println!("{}{}{}",
                prefix, 
                name, 
                script_tag,
            );
            
            // Print children recursively
            let children = node.get_children();
            let child_count = children.len();
            
            
            for (i, child_id) in children.iter().enumerate() {
                let is_last_child = i == child_count - 1;
                self.print_node_recursive(*child_id, depth + 1, is_last_child);
            }
        } else {
            println!("{}⚠️ Missing node: {}", "  ".repeat(depth), node_id);
        }
    }

    /// Helper to extract is_root_of from a SceneNode
    /// Uses BaseNode trait method
    fn get_is_root_of(node: &SceneNode) -> Option<String> {
        node.get_is_root_of().map(|s| s.to_string())
    }

    /// Helper to set the name on a SceneNode
    /// Uses BaseNode trait method
    fn set_node_name(node: &mut SceneNode, new_name: String) {
        node.set_name(new_name);
    }



    /// Check if a node name conflicts with any sibling (node with the same parent)
    fn has_sibling_name_conflict(&self, parent_id: Uuid, name: &str) -> bool {
        self.data.nodes.values().any(|n| {
            n.get_parent() == parent_id && n.get_name() == name
        })
    }

    /// Check if a node name conflicts with its parent or any ancestor
    fn has_parent_or_ancestor_name_conflict(&self, parent_id: Uuid, name: &str) -> bool {
        let mut current_id = parent_id;
        
        // Walk up the tree checking each ancestor
        loop {
            if let Some(ancestor) = self.data.nodes.get(&current_id) {
                if ancestor.get_name() == name {
                    return true;
                }
                
                // Move to parent
                let next_parent = ancestor.get_parent();
                if next_parent == current_id {
                    // Reached root (parent points to itself)
                    break;
                }
                current_id = next_parent;
            } else {
                // Parent doesn't exist in scene yet
                break;
            }
        }
        
        false
    }

    /// Resolve name conflicts by appending a digit suffix
    /// Checks for conflicts among siblings AND with parent/ancestors
    fn resolve_name_conflict(&self, parent_id: Uuid, base_name: &str) -> String {
        let mut counter = 1;
        let mut candidate = format!("{}{}", base_name, counter);
        
        // Check if candidate conflicts with siblings OR parent/ancestors
        while self.has_sibling_name_conflict(parent_id, &candidate) 
            || self.has_parent_or_ancestor_name_conflict(parent_id, &candidate) {
            counter += 1;
            candidate = format!("{}{}", base_name, counter);
        }
        
        candidate
    }

    fn ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.provider.load_ctor(short)
    }

    pub fn render(&mut self, gfx: &mut Graphics) {
        let dirty_nodes = {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "get_dirty_nodes").entered();
            self.get_dirty_nodes()
        };
        if dirty_nodes.is_empty() {
            return;
        }
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "traverse_and_render", count = dirty_nodes.len()).entered();
            self.traverse_and_render(dirty_nodes, gfx);
        }
    }

    pub fn update(&mut self) {
        #[cfg(feature = "profiling")]
        let _span = tracing::span!(tracing::Level::INFO, "Scene::update").entered();
        
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
            let avg_delta = self.delta_accum / self.true_updates as f32;
            println!(
                "🔹 UPS: {:.2}, Avg Delta: {:.6}, Current Delta: {:.6}",
                ups, avg_delta, true_delta
            );
            self.delta_accum = 0.0;
            self.true_updates = 0;
        }

        // Automatically poll Joy-Con 1 devices if polling is enabled
        // (Joy-Con 2 is polled automatically via async task)
        if let Some(mgr) = self.get_controller_manager() {
            let mgr = mgr.lock().unwrap();
            if mgr.is_polling_enabled() {
                mgr.poll_joycon1_sync();
            }
        }

        // Fixed update logic - runs at XPS rate from project manifest
        let xps = {
            let project_ref = self.project.borrow();
            project_ref.xps()
        };
        let fixed_delta = 1.0 / xps.max(1.0); // Time per fixed update

        self.fixed_update_accumulator += true_delta;

        // Check if we should run fixed update this frame
        let should_run_fixed_update = self.fixed_update_accumulator >= fixed_delta;

        if should_run_fixed_update {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "fixed_update").entered();
            
            // Calculate how many fixed updates to run (catch up if behind)
            let fixed_update_count = (self.fixed_update_accumulator / fixed_delta).floor() as u32;
            let clamped_count = fixed_update_count.min(5); // Cap at 5 to prevent spiral of death

            for _ in 0..clamped_count {
                // Update collider transforms before physics step
                {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "update_collider_transforms").entered();
                    self.update_collider_transforms();
                }
                
                // Step physics simulation
                {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "physics_step").entered();
                    self.physics_2d.borrow_mut().step(fixed_delta);
                }

                // Run fixed update for all scripts
                {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "script_fixed_updates").entered();
                    
                    // OPTIMIZED: Rebuild cached_script_ids when dirty (update/fixed_update vectors are maintained incrementally)
                    if self.scripts_dirty {
                        self.cached_script_ids.clear();
                        self.cached_script_ids.extend(self.scripts.keys().copied());
                        self.scripts_dirty = false;
                    }
                    
                    // OPTIMIZED: Use pre-filtered vector of scripts with fixed_update
                    // Collect script IDs to avoid borrow checker issues
                    let script_ids: Vec<Uuid> = self.scripts_with_fixed_update.iter().copied().collect();
                    
                    // Clone project reference before loop to avoid borrow conflicts
                    let project_ref = self.project.clone();
                    for id in script_ids {
                        #[cfg(feature = "profiling")]
                        let _span = tracing::span!(tracing::Level::INFO, "script_fixed_update", id = %id).entered();
                        
                        // Rc::clone() is cheap (just increments ref count), but we need it per call
                        // because ScriptApi::new takes &mut self
                        let mut project_borrow = project_ref.borrow_mut();
                        let mut api = ScriptApi::new(fixed_delta, self, &mut *project_borrow);
                        api.call_fixed_update(id);
                    }
                }

                // Run internal fixed update for nodes that need it
                {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "node_internal_fixed_updates").entered();
                    
                    // Optimize: collect first to avoid borrow checker issues (HashSet iteration order is non-deterministic but that's fine)
                    let node_ids: Vec<Uuid> = self.nodes_with_internal_fixed_update.iter().copied().collect();
                    // OPTIMIZED: Clone project once before loop instead of per node
                    let project_ref = self.project.clone();
                    for node_id in node_ids {
                        let mut project_borrow = project_ref.borrow_mut();
                        let mut api = ScriptApi::new(fixed_delta, self, &mut *project_borrow);
                        api.call_node_internal_fixed_update(node_id);
                    }
                }
            }

            // Subtract the time we consumed
            self.fixed_update_accumulator -= fixed_delta * clamped_count as f32;
        }

        // Regular update - runs every frame
        // OPTIMIZED: update/fixed_update vectors are maintained incrementally at insertion/removal time
        // Only rebuild cached_script_ids when dirty (if not already done in fixed_update section)
        if self.scripts_dirty {
            self.cached_script_ids.clear();
            self.cached_script_ids.extend(self.scripts.keys().copied());
            self.scripts_dirty = false;
        }

        {
            // OPTIMIZED: Use pre-filtered vector of scripts with update
            // Collect script IDs to avoid borrow checker issues
            let script_ids: Vec<Uuid> = self.scripts_with_update.iter().copied().collect();
            
            // Early return if no scripts to update (avoids unnecessary work)
            if script_ids.is_empty() {
                self.process_queued_signals();
                return;
            }
            
            // Clone project reference before loop to avoid borrow conflicts
            let project_ref = self.project.clone();
            
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "script_updates", count = script_ids.len()).entered();
            
            for id in script_ids {
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "script_update", id = %id).entered();
                
                // OPTIMIZED: Borrow project once per script (RefCell borrow_mut is fast but still has overhead)
                // Note: We can't borrow project once for all scripts because ScriptApi needs &mut self (scene)
                let mut project_borrow = project_ref.borrow_mut();
                let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow);
                api.call_update(id);
            }
        }

        // Global transforms are now calculated lazily when needed (in traverse_and_render)

        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "process_queued_signals").entered();
            self.process_queued_signals();
        }
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

    /// Emit signal deferred - queue for processing at end of frame
    fn emit_signal_id_deferred(&mut self, signal: u64, params: &[Value]) {
        // Convert slice to SmallVec for stack-allocated storage (≤3 params = no heap allocation)
        let mut smallvec = SmallVec::new();
        smallvec.extend(params.iter().cloned());
        self.queued_signals.push((signal, smallvec));
    }

    // ✅ OPTIMIZED: Use drain() to collect, then process (avoids borrow checker issues)
    fn process_queued_signals(&mut self) {
        if self.queued_signals.is_empty() {
            return;
        }

        // Collect drained items first to release the borrow, then process
        let signals: Vec<_> = self.queued_signals.drain(..).collect();
        for (signal, params) in signals {
            self.emit_signal_impl(signal, &params);
        }
    }

    /// Emit signal instantly - zero allocation, direct function call
    /// Params are passed as compile-time static slice, never stored
    fn emit_signal_id(&mut self, signal: u64, params: &[Value]) {
        self.emit_signal_impl(signal, params);
    }

    /// Internal implementation - emits signal immediately to all connected handlers
    /// Params passed as slice reference - zero allocation when called from emit_signal_id
    /// OPTIMIZED: Uses SmallVec for stack allocation when listener count is small
    fn emit_signal_impl(&mut self, signal: u64, params: &[Value]) {
        let start_time = Instant::now();
        
        // Copy out listeners before mutable borrow
        let script_map_opt = self.signals.connections.get(&signal);
        if script_map_opt.is_none() {
            return;
        }

        // OPTIMIZED: Use SmallVec with inline capacity of 4 listeners
        // Most signals have 1-3 listeners, so this avoids heap allocation in common case
        // For signals with >4 listeners, only allocates once
        let script_map = script_map_opt.unwrap();
        let mut call_list = SmallVec::<[(Uuid, u64); 4]>::new();
        for (uuid, fns) in script_map.iter() {
            for &fn_id in fns.iter() {
                call_list.push((*uuid, fn_id));
            }
        }

        let setup_time = start_time.elapsed();
        
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

        let call_start = Instant::now();
        
        // OPTIMIZED: Set context once and use fast-path calls
        api.set_context();
        
        // OPTIMIZED: Use internal fast-path that skips redundant context operations
        for (target_id, fn_id) in call_list.iter() {
            api.call_function_id_fast(*target_id, *fn_id, params);
        }
        
        // Clear context once after all calls
        ScriptApi::clear_context();
        
        let call_time = call_start.elapsed();
        let total_time = start_time.elapsed();
        
        eprintln!("[SIGNAL TIMING - Scene] Signal ID: {} | Listeners: {} | Setup: {:?} | Calls: {:?} | Total: {:?}", 
            signal, call_list.len(), setup_time, call_time, total_time);
    }

    pub fn instantiate_script(ctor: CreateFn, node_id: Uuid) -> Box<dyn ScriptObject> {
        let raw = ctor();
        let mut boxed: Box<dyn ScriptObject> = unsafe { Box::from_raw(raw) };
        boxed.set_id(node_id);
        boxed
    }

    pub fn add_node_to_scene(&mut self, mut node: SceneNode) -> anyhow::Result<()> {
        let id = node.get_id();

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
        self.data.nodes.insert(id, node);
        // Mark transform as dirty for Node2D nodes (after insertion)
        self.mark_transform_dirty_recursive(id);
        println!("✅ Node {} added\n", id);

        // Register node for internal fixed updates if needed
        if let Some(node_ref) = self.data.nodes.get(&id) {
            if node_ref.needs_internal_fixed_update() {
                // Optimize: HashSet insert is O(1) and handles duplicates automatically
                self.nodes_with_internal_fixed_update.insert(id);
            }
        }

        // node is moved already, so get it back immutably from scene
        let script_path_opt = self.data.nodes.get(&id)
            .and_then(|node_ref| node_ref.get_script_path().map(|s| s.to_string()));
        
        if let Some(script_path) = script_path_opt {
            println!("   ✅ Found script_path: {}", script_path);

            let identifier = script_path_to_identifier(&script_path)
                .map_err(|e| anyhow::anyhow!("Invalid script path {}: {}", script_path, e))?;
            let ctor = self.ctor(&identifier)?;

            // Create the script
            let handle = self.instantiate_script(ctor, id);
            
            // Check flags and add to appropriate vectors
            let flags = handle.script_flags();
            
            if flags.has_update() && !self.scripts_with_update.contains(&id) {
                self.scripts_with_update.push(id);
            }
            if flags.has_fixed_update() && !self.scripts_with_fixed_update.contains(&id) {
                self.scripts_with_fixed_update.push(id);
            }
            
            self.scripts.insert(id, handle);
            self.scripts_dirty = true;

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

        Ok(())
    }

    pub fn get_root(&self) -> &SceneNode {
        &self.data.nodes[&self.data.root_id]
    }

    // Remove node and stop rendering
    pub fn remove_node(&mut self, node_id: Uuid, gfx: &mut Graphics) {
        // Stop rendering this node and all its children
        self.stop_rendering_recursive(node_id, gfx);

        // Remove from scene
        self.data.nodes.remove(&node_id);

        // Remove scripts - actually delete them from scene
        if self.scripts.remove(&node_id).is_some() {
            // Remove from update/fixed_update vectors (actual deletion)
            self.scripts_with_update.retain(|&script_id| script_id != node_id);
            self.scripts_with_fixed_update.retain(|&script_id| script_id != node_id);
            self.scripts_dirty = true;
        }
    }

    /// Get the global transform for a node, calculating it lazily if dirty
    /// This recursively traverses up the parent chain until it finds a clean transform
    pub fn get_global_transform(&mut self, node_id: Uuid) -> Option<crate::structs2d::Transform2D> {
        // First, check if this node exists and get its parent
        let (parent_id, local_transform, is_dirty) = if let Some(node) = self.data.nodes.get(&node_id) {
            let node2d = match node.as_node2d() {
                Some(n2d) => n2d,
                None => {
                    // Not a Node2D-based node - can't get global transform
                    return None;
                }
            };
            let local = match node.get_node2d_transform() {
                Some(t) => t,
                None => {
                    return None;
                }
            };
            (node2d.base.parent_id, local, node2d.transform_dirty)
        } else {
            return None;
        };

        // If not dirty, return cached global transform
        if !is_dirty {
            if let Some(node) = self.data.nodes.get(&node_id) {
                return node.as_node2d().map(|n2d| n2d.global_transform);
            }
        }

        // Need to recalculate - get parent's global transform (recursively)
        // If parent is not Node2D-based, use identity transform
        let parent_global = if !parent_id.is_nil() {
            // Try to get parent's global transform, but if parent is not Node2D-based, use identity
            self.get_global_transform(parent_id).unwrap_or_else(|| {
                // Parent exists but is not Node2D-based (e.g., regular Node) - use identity transform
                crate::structs2d::Transform2D::default()
            })
        } else {
            // No parent - use identity transform
            crate::structs2d::Transform2D::default()
        };

        // Calculate this node's global transform
        let mut global = crate::structs2d::Transform2D::default();
        global.scale.x = parent_global.scale.x * local_transform.scale.x;
        global.scale.y = parent_global.scale.y * local_transform.scale.y;
        global.position.x = parent_global.position.x + (local_transform.position.x * parent_global.scale.x);
        global.position.y = parent_global.position.y + (local_transform.position.y * parent_global.scale.y);
        global.rotation = parent_global.rotation + local_transform.rotation;

        // Cache the result and mark as clean
        if let Some(node) = self.data.nodes.get_mut(&node_id) {
            if let Some(node2d) = node.as_node2d_mut() {
                node2d.global_transform = global;
                node2d.transform_dirty = false;
            }
        }

        Some(global)
    }

    /// Set the global transform for a node (marks it as dirty)
    pub fn set_global_transform(&mut self, node_id: Uuid, transform: crate::structs2d::Transform2D) -> Option<()> {
        if let Some(node) = self.data.nodes.get_mut(&node_id) {
            if let Some(node2d) = node.as_node2d_mut() {
                node2d.global_transform = transform;
                node2d.transform_dirty = true;
                Some(())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Mark a node's transform as dirty (and all its children)
    /// Also marks nodes as dirty for rendering so they get picked up by get_dirty_nodes()
    pub fn mark_transform_dirty_recursive(&mut self, node_id: Uuid) {
        // Collect children first before any mutable borrows
        let children: Vec<Uuid> = self.data.nodes
            .get(&node_id)
            .map(|node| node.get_children().iter().copied().collect())
            .unwrap_or_default();
        
        // Now mark this node as dirty (mutable borrow)
        if let Some(node) = self.data.nodes.get_mut(&node_id) {
            // Mark this node as dirty for rendering
            node.mark_dirty();
            
            // Mark this node's transform as dirty if it's a Node2D-based node
            if let Some(node2d) = node.as_node2d_mut() {
                node2d.transform_dirty = true;
            }
        }
        
        // Recurse on children (mutable borrow of self is now released)
        for child_id in children {
            self.mark_transform_dirty_recursive(child_id);
        }
    }

   

    /// Update collider transforms to match node transforms
    fn update_collider_transforms(&mut self) {
        // First collect node IDs that need updating
        let node_ids: Vec<Uuid> = self.data.nodes
            .iter()
            .filter_map(|(node_id, node)| {
                if let SceneNode::CollisionShape2D(cs) = node {
                    if cs.collider_handle.is_some() {
                        Some(*node_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        
        // Mark all collision shapes as dirty to force recalculation
        // This ensures their global transforms are recalculated even if the dirty flag
        // wasn't set (e.g., if parent moved but child wasn't marked dirty)
        for node_id in &node_ids {
            if let Some(node) = self.data.nodes.get_mut(node_id) {
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.transform_dirty = true;
                }
            }
        }
        
        // Get global transforms (requires mutable access)
        // Now that we've marked them dirty, get_global_transform will recalculate
        let mut to_update: Vec<(Uuid, [f32; 2], f32)> = Vec::new();
        for node_id in node_ids {
            if let Some(global) = self.get_global_transform(node_id) {
                let position = [global.position.x, global.position.y];
                let rotation = global.rotation;
                to_update.push((node_id, position, rotation));
            }
        }
        
        // Update physics colliders (after releasing all borrows)
        let mut physics = self.physics_2d.borrow_mut();
        for (node_id, position, rotation) in to_update {
            physics.update_collider_transform(node_id, position, rotation);
        }
    }

    /// Register CollisionShape2D nodes with the physics world
    fn register_collision_shapes(&mut self, node_ids: &[Uuid]) {
        // First, collect all the data we need (shape info, transforms, parent info)
        let mut to_register: Vec<(Uuid, crate::physics::physics_2d::ColliderShape, Option<Uuid>)> = Vec::new();
        
        for &node_id in node_ids {
            if let Some(node) = self.data.nodes.get(&node_id) {
                if let SceneNode::CollisionShape2D(collision_shape) = node {
                    // Only register if it has a shape defined
                    if let Some(shape) = collision_shape.shape {
                        let parent_id = if collision_shape.parent_id.is_nil() {
                            None
                        } else {
                            Some(collision_shape.parent_id)
                        };
                        to_register.push((node_id, shape, parent_id));
                    }
                }
            }
        }
        
        // Get global transforms for all nodes (requires mutable access)
        let mut global_transforms: HashMap<Uuid, ([f32; 2], f32)> = HashMap::new();
        for (node_id, _, _) in &to_register {
            if let Some(global) = self.get_global_transform(*node_id) {
                global_transforms.insert(*node_id, ([global.position.x, global.position.y], global.rotation));
            }
        }
        
        // Now register with physics (after releasing all node borrows)
        let mut physics = self.physics_2d.borrow_mut();
        let mut handles_to_store: Vec<(Uuid, rapier2d::prelude::ColliderHandle, Option<Uuid>)> = Vec::new();
        
        for (node_id, shape, parent_id) in to_register {
            // Use global transform if available, otherwise use default (for first frame)
            let (world_position, world_rotation) = global_transforms
                .get(&node_id)
                .copied()
                .unwrap_or(([0.0, 0.0], 0.0));
            
            // Create the sensor collider in physics world with world transform
            let collider_handle = physics.create_sensor_collider(
                node_id,
                shape,
                world_position,
                world_rotation,
            );
            
            // If this collision shape is a child of an Area2D, register it
            if let Some(pid) = parent_id {
                if let Some(parent) = self.data.nodes.get(&pid) {
                    if matches!(parent, SceneNode::Area2D(_)) {
                        physics.register_area_collider(pid, collider_handle);
                    }
                }
            }
            
            handles_to_store.push((node_id, collider_handle, parent_id));
        }
        
        // Drop physics borrow before mutating nodes
        drop(physics);
        
        // Store handles in collision shapes
        for (node_id, collider_handle, _) in handles_to_store {
            if let Some(node) = self.data.nodes.get_mut(&node_id) {
                if let SceneNode::CollisionShape2D(cs) = node {
                    cs.collider_handle = Some(collider_handle);
                }
            }
        }
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
    // Also includes visible Node2D nodes that need their transforms calculated
    fn get_dirty_nodes(&self) -> Vec<Uuid> {
        const PARALLEL_THRESHOLD: usize = 50;

        if self.data.nodes.len() >= PARALLEL_THRESHOLD {
            // Note: IndexMap doesn't implement IntoParallelRefIterator, so we use regular iter
            self.data
                .nodes
                .iter()
                .filter_map(|(id, node)| {
                    // Include if dirty (for rendering changes)
                    // OR if it's a Node2D with a dirty transform (needs recalculation)
                    if node.is_dirty() {
                        Some(*id)
                    } else if let Some(node2d) = node.as_node2d() {
                        // Include Node2D nodes with dirty transforms so they get recalculated and rendered
                        if node2d.transform_dirty {
                            Some(*id)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            self.data
                .nodes
                .iter()
                .filter_map(|(id, node)| {
                    // Include if dirty (for rendering changes)
                    // OR if it's a Node2D with a dirty transform (needs recalculation)
                    if node.is_dirty() {
                        Some(*id)
                    } else if let Some(node2d) = node.as_node2d() {
                        // Include Node2D nodes with dirty transforms so they get recalculated and rendered
                        if node2d.transform_dirty {
                            Some(*id)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        }
    }


    fn traverse_and_render(&mut self, dirty_nodes: Vec<Uuid>, gfx: &mut Graphics) {
        for node_id in dirty_nodes {
            // Get global transform first (before borrowing node mutably)
            // Only try to get transform for Node2D-based nodes
            let global_transform_opt = if let Some(node) = self.data.nodes.get(&node_id) {
                if node.as_node2d().is_some() {
                    self.get_global_transform(node_id)
                } else {
                    // Not a Node2D node - skip transform calculation
                    None
                }
            } else {
                None
            };
            
            if let Some(node) = self.data.nodes.get_mut(&node_id) {
                // Get timestamp from the borrowed node
                let timestamp = node.get_created_timestamp();
                match node {
                    //2D Nodes
                    SceneNode::Sprite2D(sprite) => {
                        if sprite.visible {
                            if let Some(tex) = &sprite.texture_path {
                                // Use the global transform we got earlier
                                if let Some(global_transform) = global_transform_opt {
                                    gfx.renderer_2d.queue_texture(
                                        &mut gfx.renderer_prim,
                                        &mut gfx.texture_manager,
                                        &gfx.device,
                                        &gfx.queue,
                                        node_id,
                                        tex,
                                        global_transform,
                                        sprite.pivot,
                                        sprite.z_index,
                                        timestamp,
                                    );
                                }
                            }
                        }
                    }
                    SceneNode::Camera2D(camera) => {
                        if camera.active {
                            gfx.update_camera_2d(camera);
                        }
                    }
                    SceneNode::Shape2D(shape) => {
                        if shape.visible {
                            if let Some(shape_type) = shape.shape_type {
                                // Use the global transform we got earlier
                                if let Some(transform) = global_transform_opt {
                                    let pivot = shape.pivot;
                                    let z_index = shape.z_index;
                                    let color = shape.color.unwrap_or(crate::Color::new(255, 255, 255, 200));
                                    let border_thickness = if shape.filled { 0.0 } else { 2.0 };
                                    let is_border = !shape.filled;

                                    match shape_type {
                                        ShapeType2D::Rectangle { width, height } => {
                                            gfx.renderer_2d.queue_rect(
                                                &mut gfx.renderer_prim,
                                                node_id,
                                                transform,
                                                crate::Vector2::new(width, height),
                                                pivot,
                                                color,
                                                None, // No corner radius
                                                border_thickness,
                                                is_border,
                                                z_index,
                                                timestamp,
                                            );
                                        }
                                        ShapeType2D::Circle { radius } => {
                                            // For circles, render as a square with corner radius = radius
                                            let size = radius * 2.0;
                                            gfx.renderer_2d.queue_rect(
                                                &mut gfx.renderer_prim,
                                                node_id,
                                                transform,
                                                crate::Vector2::new(size, size),
                                                pivot,
                                                color,
                                                Some(crate::ui_elements::ui_container::CornerRadius {
                                                    top_left: radius,
                                                    top_right: radius,
                                                    bottom_left: radius,
                                                    bottom_right: radius,
                                                }),
                                                border_thickness,
                                                is_border,
                                                z_index,
                                                timestamp,
                                            );
                                        }
                                        ShapeType2D::Square { size } => {
                                            gfx.renderer_2d.queue_rect(
                                                &mut gfx.renderer_prim,
                                                node_id,
                                                transform,
                                                crate::Vector2::new(size, size),
                                                pivot,
                                                color,
                                                None,
                                                border_thickness,
                                                is_border,
                                                z_index,
                                                timestamp,
                                            );
                                        }
                                        ShapeType2D::Triangle { base, height } => {
                                            // Triangles rendered as rectangles for now
                                            gfx.renderer_2d.queue_rect(
                                                &mut gfx.renderer_prim,
                                                node_id,
                                                transform,
                                                crate::Vector2::new(base, height),
                                                pivot,
                                                color,
                                                None,
                                                border_thickness,
                                                is_border,
                                                z_index,
                                                timestamp,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    SceneNode::UINode(ui_node) => {
                        // UI renderer handles layout + rendering internally
                        render_ui(ui_node, gfx);
                    }

                    //3D Nodes
                    SceneNode::Camera3D(camera) => {
                        if camera.active {
                            gfx.update_camera_3d(camera);
                        }
                    }
                    SceneNode::MeshInstance3D(mesh) => {
                        if mesh.visible {
                            if let Some(path) = &mesh.mesh_path {
                                gfx.renderer_3d.queue_mesh(
                                    node_id,
                                    path,
                                    mesh.transform,
                                    mesh.material_path.as_deref(),
                                    &mut gfx.mesh_manager,
                                    &mut gfx.material_manager,
                                    &mut gfx.device,
                                    &mut gfx.queue,
                                );
                            }
                        }
                    }

                    SceneNode::OmniLight3D(light) => {
                        gfx.renderer_3d.queue_light(
                            light.id,
                            crate::renderer_3d::LightUniform {
                                position: light.transform.position.to_array(),
                                color: light.color.to_array(),
                                intensity: light.intensity,
                                ambient: [0.05, 0.05, 0.05],
                                ..Default::default()
                            },
                        );
                    }
                    SceneNode::DirectionalLight3D(light) => {
                        let dir = light.transform.forward();
                        gfx.renderer_3d.queue_light(
                            light.id,
                            crate::renderer_3d::LightUniform {
                                position: [dir.x, dir.y, dir.z],
                                color: light.color.to_array(),
                                intensity: light.intensity,
                                ambient: [0.05, 0.05, 0.05],
                                ..Default::default()
                            },
                        );
                    }
                    SceneNode::SpotLight3D(light) => {
                        let dir = light.transform.forward();
                        gfx.renderer_3d.queue_light(
                            light.id,
                            crate::renderer_3d::LightUniform {
                                position: [dir.x, dir.y, dir.z],
                                color: light.color.to_array(),
                                intensity: light.intensity,
                                ambient: [0.05, 0.05, 0.05],
                                ..Default::default()
                            },
                        );
                    }
                    _ => {}
                }
                // Only set dirty to false if transform is also clean (both need to be clean to skip rendering)
                // If transform is still dirty, keep node dirty so it renders again next frame
                if let Some(node2d) = node.as_node2d() {
                    if !node2d.transform_dirty {
                        node.set_dirty(false);
                    }
                } else {
                    node.set_dirty(false);
                }
            }
        }
    }
}

//
// ---------------- SceneAccess impl ----------------
//

impl<P: ScriptProvider> SceneAccess for Scene<P> {
    fn get_scene_node_ref(&self, id: Uuid) -> Option<&SceneNode> {
        self.data.nodes.get(&id)
    }
    
    fn get_scene_node_mut(&mut self, id: Uuid) -> Option<&mut SceneNode> {
        self.data.nodes.get_mut(&id)
    }
    
    fn get_scene_node(&mut self, id: Uuid) -> Option<&mut SceneNode> {
        self.get_scene_node_mut(id)
    }

    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.provider.load_ctor(short)
    }

    fn instantiate_script(
        &mut self,
        ctor: CreateFn,
        node_id: Uuid,
    ) -> Box<dyn ScriptObject> {
        Self::instantiate_script(ctor, node_id)
    }

    fn add_node_to_scene(&mut self, node: SceneNode) -> anyhow::Result<()> {
        self.add_node_to_scene(node)
    }

    fn merge_nodes(&mut self, nodes: &[SceneNode]) {
        // Process nodes and mark them dirty immediately
        for node in nodes {
            let id = node.get_id(); // Get ID without cloning
            
            if let Some(existing) = self.data.nodes.get_mut(&id) {
                // Node exists: mark dirty and replace
                existing.mark_dirty();
                *existing = node.clone();
            } else {
                // Node doesn't exist: clone and insert
                let mut new_node = node.clone();
                new_node.mark_dirty();
                println!("Inserting new node with ID {}: {:?} during merge", id, new_node);
                self.data.nodes.insert(id, new_node);
            }

            // Register node for internal fixed updates if needed (check after insertion)
            if let Some(node_ref) = self.data.nodes.get(&id) {
                if node_ref.needs_internal_fixed_update() {
                    if !self.nodes_with_internal_fixed_update.contains(&id) {
                        self.nodes_with_internal_fixed_update.insert(id);
                    }
                }
            }
            
            // Mark transform dirty immediately - no need to collect IDs first!
            self.mark_transform_dirty_recursive(id);
        }
    }

    fn connect_signal_id(&mut self, signal: u64, target_id: Uuid, function: u64) {
        self.connect_signal(signal, target_id, function);
    }

    fn get_signal_connections(&self, signal: u64) -> Option<&HashMap<Uuid, SmallVec<[u64; 4]>>> {
        self.signals.connections.get(&signal)
    }

    fn emit_signal_id(&mut self, signal: u64, params: &[Value]) {
        self.emit_signal_id(signal, params);
    }

    fn emit_signal_id_deferred(&mut self, signal: u64, params: &[Value]) {
        self.emit_signal_id_deferred(signal, params);
    }

    fn get_script(&mut self, id: Uuid) -> Option<&mut Box<dyn ScriptObject>> {
        self.scripts.get_mut(&id)
    }
    
    fn get_script_mut(&mut self, id: Uuid) -> Option<&mut Box<dyn ScriptObject>> {
        self.scripts.get_mut(&id)
    }
    
    fn take_script(&mut self, id: Uuid) -> Option<Box<dyn ScriptObject>> {
        // Just remove from HashMap, DON'T modify the filtered vectors
        // This is used for temporary borrowing during update calls
        self.scripts.remove(&id)
    }
    
    fn insert_script(&mut self, id: Uuid, script: Box<dyn ScriptObject>) {
        // Just insert into HashMap, DON'T modify the filtered vectors
        // This is used for temporary put-back during update calls
        self.scripts.insert(id, script);
    }

    // NEW method implementation
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>> {
        self.app_command_tx.as_ref()
    }

    fn get_controller_manager(&self) -> Option<&Mutex<ControllerManager>> {
        Some(&self.controller_manager)
    }

    fn get_input_manager(&self) -> Option<&Mutex<InputManager>> {
        Some(&self.input_manager)
    }

    fn get_physics_2d(&self) -> Option<&std::cell::RefCell<PhysicsWorld2D>> {
        Some(&self.physics_2d)
    }

    fn get_global_transform(&mut self, node_id: Uuid) -> Option<crate::structs2d::Transform2D> {
        Self::get_global_transform(self, node_id)
    }

    fn set_global_transform(&mut self, node_id: Uuid, transform: crate::structs2d::Transform2D) -> Option<()> {
        Self::set_global_transform(self, node_id, transform)
    }

    fn mark_transform_dirty_recursive(&mut self, node_id: Uuid) {
        Self::mark_transform_dirty_recursive(self, node_id)
    }
}

//
// ---------------- Specialization for DllScriptProvider ----------------
//

use crate::registry::DllScriptProvider;
use libloading::Library;

pub fn default_perro_rust_path() -> io::Result<PathBuf> {
    match get_project_root() {
        ProjectRoot::Disk { root, .. } => {
            let mut path = root;
            path.push(".perro");
            path.push("scripts");
            path.push("builds");

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
        let mut root_node = Node::new();
        root_node.name = Cow::Borrowed("Root");
        let root_node = SceneNode::Node(root_node);

        // Load DLL
        let lib_path = default_perro_rust_path()?;
        println!("Loading script library from {:?}", lib_path);
        
        // Check if DLL exists before trying to load it
        if !lib_path.exists() {
            return Err(anyhow::anyhow!(
                "Script DLL not found at {:?}. Please compile scripts first using: cargo run -p perro_core -- --path <path> --scripts",
                lib_path
            ));
        }
        
        let lib = unsafe { Library::new(&lib_path).map_err(|e| {
            anyhow::anyhow!("Failed to load DLL at {:?}: {}. The DLL might be corrupted or incompatible.", lib_path, e)
        })? };
        let provider = DllScriptProvider::new(Some(lib));

        // Borrow project briefly to clone root_path & name
        let _root = {
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

        // Inject project root into DLL (optional - only if DLL needs it)
        // NOTE: On Windows, this can cause STATUS_ACCESS_VIOLATION if the DLL was built
        // against a different version of perro_core, because DLLs have separate static
        // variable instances. Since the project root is already set in the main binary
        // before loading the DLL, this call is redundant and can be safely skipped.
        // 
        // If you're experiencing access violations, try commenting out this call:
        // provider.inject_project_root(&root)?;
        
        // For now, we'll skip it on Windows to avoid access violations
        #[cfg(not(windows))]
        {
            if let Err(e) = provider.inject_project_root(&_root) {
                eprintln!("⚠ Warning: Failed to inject project root into DLL (this is usually okay): {}", e);
            }
        }
        
        #[cfg(windows)]
        {
            // On Windows, skip the DLL call to avoid potential access violations
            // The project root is already set in the main binary
            eprintln!("ℹ Skipping DLL project root injection on Windows (already set in main binary)");
        }

        // Now move `project` into Scene
        let mut game_scene = Scene::new(root_node, provider, project);

        // Initialize input manager with action map from project.toml
        {
            let project_ref = game_scene.project.borrow();
            let input_map = project_ref.get_input_map();
            let mut input_mgr = game_scene.input_manager.lock().unwrap();
            input_mgr.load_action_map(input_map);
        }

        println!("Building scene from project manifest...");
        
        // ✅ root script first - load before merging main scene
        let root_script_path_opt = {
            let project_ref = game_scene.project.borrow();
            project_ref.root_script().map(|s| s.to_string())
        };

        if let Some(root_script_path) = root_script_path_opt {
            if let Ok(identifier) = script_path_to_identifier(&root_script_path) {
                if let Ok(ctor) = game_scene.provider.load_ctor(&identifier) {
                    let root_id = game_scene.get_root().get_id();
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
                } else {
                    println!("❌ Could not find symbol for {}", identifier);
                }
            }
        }

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
        let game_root = game_scene.get_root().get_id();
        game_scene.merge_scene_data(loaded_data, game_root)?;
        let graft_time = t_graft_begin.elapsed();
        println!(
            "⏱  Scene grafting completed in {:.03} sec ({} ms)",
            graft_time.as_secs_f32(),
            graft_time.as_millis()
        );

        // Debug: Print all node names after scene grafting
        println!("\n📋 All nodes after scene grafting ({} total):", game_scene.data.nodes.len());
        let mut node_list: Vec<_> = game_scene.data.nodes.iter().collect();
        node_list.sort_by_key(|(id, _)| *id);
        for (id, node) in node_list {
            let name = node.get_name();
            let script_info = if game_scene.scripts.contains_key(id) {
                " [HAS SCRIPT]"
            } else {
                ""
            };
            println!("  - {}{}", name, script_info);
        }
        println!();

        game_scene.print_scene_tree();  // or print_parent_child_info()


        Ok(game_scene)
    }
}
