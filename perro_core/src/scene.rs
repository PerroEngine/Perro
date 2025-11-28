use crate::{
    Graphics, Node, RenderLayer, Transform3D, Vector2, api::ScriptApi, app_command::AppCommand, apply_fur::{build_ui_elements_from_fur, parse_fur_file}, asset_io::{ProjectRoot, get_project_root, load_asset, save_asset}, fur_ast::{FurElement, FurNode}, manifest::Project, node_registry::{BaseNode, SceneNode}, prelude::string_to_u64, script::{CreateFn, SceneAccess, Script, ScriptObject, ScriptProvider, UpdateOp, Var}, transpiler::script_path_to_identifier, ui_element::{BaseElement, UIElement}, ui_renderer::render_ui // NEW import
};

use glam::{Mat4, Vec3};
use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeStruct};
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    any::Any,
    cell::RefCell,
    collections::HashMap,
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
#[derive(Debug, Clone)]
pub struct SceneData {
    pub root_id: Uuid,
    pub nodes: IndexMap<Uuid, SceneNode>,
}

impl Serialize for SceneData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Create a plain serializable map of local_id → node
        let mut node_map: IndexMap<Uuid, &SceneNode> = IndexMap::with_capacity(self.nodes.len());
        for node in self.nodes.values() {
            node_map.insert(node.get_local_id(), node);
        }

        // Begin the struct
        let mut state = serializer.serialize_struct("SceneData", 2)?;
        state.serialize_field("root_id", &self.root_id)?;
        state.serialize_field("nodes", &node_map)?;
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

                if let Some(parent_local) = node.get_parent() {
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
            if let Some(parent_id) = node.get_parent() {
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
    pub test_val: Value,
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
            test_val: Value::Null,
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
            test_val: Value::Null,
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

        // Parallel UUID generation for large scenes
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
        // 2️⃣ PARALLEL NODE REMAPPING
        // ───────────────────────────────────────────────
        let remap_start = Instant::now();
        const NODE_PARALLEL_THRESHOLD: usize = 20;

        if other.nodes.len() >= NODE_PARALLEL_THRESHOLD {
            // Convert to Vec for parallel processing
            let mut nodes_vec: Vec<(Uuid, SceneNode)> = other.nodes.into_iter().collect();

            // Phase 2a: Parallel basic property remapping
            nodes_vec.par_iter_mut().for_each(|(local_id, node)| {
                let new_id = id_map[local_id];
                node.set_id(new_id);
                node.clear_children(); // Will be rebuilt in phase 2b

                // Handle script_exp_vars in parallel
                if let Some(mut script_vars) = node.get_script_exp_vars() {
                    Self::remap_script_exp_vars_uuids(&mut script_vars, &id_map);
                    node.set_script_exp_vars(Some(script_vars));
                }
            });

            // Phase 2b: Sequential relationship rebuilding
            let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

            for (local_id, node) in &mut nodes_vec {
                // Remap parent if it exists in the subscene
                if let Some(parent) = node.get_parent() {
                    if let Some(&mapped_parent) = id_map.get(&parent) {
                        node.set_parent(Some(mapped_parent));
                        parent_children
                            .entry(mapped_parent)
                            .or_default()
                            .push(id_map[local_id]);
                    }
                }
            }

            // Apply parent-child relationships
            for (local_id, node) in &mut nodes_vec {
                let new_id = id_map[local_id];
                if let Some(children) = parent_children.get(&new_id) {
                    node.get_children_mut().extend_from_slice(children);
                }
            }

            // Convert back to IndexMap
            other.nodes = nodes_vec.into_iter().collect();
        } else {
            // Sequential processing for small scenes
            let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

            for node in other.nodes.values_mut() {
                let new_id = id_map[&node.get_local_id()];
                node.set_id(new_id);
                node.clear_children();

                // Remap parent if it exists in the subscene
                if let Some(parent) = node.get_parent() {
                    if let Some(&mapped_parent) = id_map.get(&parent) {
                        node.set_parent(Some(mapped_parent));
                        parent_children
                            .entry(mapped_parent)
                            .or_default()
                            .push(new_id);
                    }
                }

                // Handle script_exp_vars
                if let Some(mut script_vars) = node.get_script_exp_vars() {
                    Self::remap_script_exp_vars_uuids(&mut script_vars, &id_map);
                    node.set_script_exp_vars(Some(script_vars));
                }
            }

            // Apply parent-child relationships
            for (parent_id, children) in parent_children {
                if let Some(parent) = other.nodes.get_mut(&parent_id) {
                    parent.get_children_mut().extend(children);
                }
            }
        }

        let remap_time = remap_start.elapsed();
        println!(
            "⏱ Node remapping: {:.2} ms",
            remap_time.as_secs_f64() * 1000.0
        );

        // ───────────────────────────────────────────────
        // 3️⃣ HANDLE ROOT NODE
        // ───────────────────────────────────────────────
        let root_start = Instant::now();
        let root_to_insert = if let Some(mut root) = other.nodes.remove(&other.root_id) {
            let new_root_id = id_map[&root.get_local_id()];
            root.set_id(new_root_id);
            root.set_parent(Some(parent_id));

            // Attach root to target parent
            if let Some(parent) = self.data.nodes.get_mut(&parent_id) {
                parent.add_child(new_root_id);
            }

            root.mark_dirty();
            Some(root)
        } else {
            eprintln!("⚠️ Merge root missing");
            None
        };

        let root_time = root_start.elapsed();

        // ───────────────────────────────────────────────
        // 4️⃣ INSERT ALL REMAPPED NODES INTO MAIN SCENE
        // ───────────────────────────────────────────────
        let insert_start = Instant::now();
        self.data.nodes.reserve(other.nodes.len() + 1);

        for mut node in other.nodes.into_values() {
            node.mark_dirty();
            self.data.nodes.insert(node.get_id(), node);
        }

        if let Some(root) = root_to_insert {
            self.data.nodes.insert(root.get_id(), root);
        }

        // Collect all new runtime IDs
        let new_ids: Vec<Uuid> = id_map.values().copied().collect();
        let insert_time = insert_start.elapsed();

        // ───────────────────────────────────────────────
        // 5️⃣ PARALLELIZED FUR LOADING (UI FILES)
        // ───────────────────────────────────────────────
        let fur_start = Instant::now();
        const FUR_PARALLEL_THRESHOLD: usize = 5;

        // Parallel collection of FUR paths
        let fur_paths: Vec<(Uuid, String)> = if new_ids.len() >= FUR_PARALLEL_THRESHOLD {
            new_ids
                .par_iter()
                .filter_map(|id| {
                    self.data.nodes.get(id).and_then(|node| {
                        if let SceneNode::UINode(u) = node {
                            u.fur_path.as_ref().map(|path| (*id, path.to_string())) // Changed: .to_string() instead of .clone()
                        } else {
                            None
                        }
                    })
                })
                .collect()
        } else {
            new_ids
                .iter()
                .filter_map(|id| {
                    self.data.nodes.get(id).and_then(|node| {
                        if let SceneNode::UINode(u) = node {
                            u.fur_path.as_ref().map(|path| (*id, path.to_string())) // Changed: .to_string() instead of .clone()
                        } else {
                            None
                        }
                    })
                })
                .collect()
        };
        // Parallel FUR data loading
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

        // Apply FUR results sequentially (needs mutable access to nodes)
        for (id, result) in fur_loads {
            if let Some(SceneNode::UINode(u)) = self.data.nodes.get_mut(&id) {
                match result {
                    Ok(fur_elements) => build_ui_elements_from_fur(u, &fur_elements),
                    Err(err) => eprintln!("⚠️ Error loading FUR for {}: {}", id, err),
                }
            }
        }

        let fur_time = fur_start.elapsed();

        // ───────────────────────────────────────────────
        // 6️⃣ PARALLEL SCRIPT PATH COLLECTION + SEQUENTIAL INITIALIZATION
        // ───────────────────────────────────────────────
        let script_start = Instant::now();

        // Parallel script path collection
        let script_targets: Vec<(Uuid, String)> = if new_ids.len() > 10 {
            new_ids
                .par_iter()
                .filter_map(|id| {
                    self.data
                        .nodes
                        .get(id)
                        .and_then(|n| n.get_script_path().map(|p| (*id, p.to_string())))
                })
                .collect()
        } else {
            new_ids
                .iter()
                .filter_map(|id| {
                    self.data
                        .nodes
                        .get(id)
                        .and_then(|n| n.get_script_path().map(|p| (*id, p.to_string())))
                })
                .collect()
        };

        // Sequential script initialization (needs mutable scene access)
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
                self.scripts.insert(id, handle);

                let mut api = ScriptApi::new(dt, self, &mut *project_borrow);
                api.call_init(id);
            }
        }

        let script_time = script_start.elapsed();
        let total_time = merge_start.elapsed();

        // ───────────────────────────────────────────────
        // 7️⃣ PERFORMANCE SUMMARY
        // ───────────────────────────────────────────────
        println!(
            "📦 Merge complete: {} total nodes (+{} new)",
            self.data.nodes.len(),
            new_ids.len()
        );

        println!(
            "⏱ Timing breakdown: total={:.2}ms | id_map={:.2}ms | remap={:.2}ms | root={:.2}ms | insert={:.2}ms | fur={:.2}ms | scripts={:.2}ms",
            total_time.as_secs_f64() * 1000.0,
            id_map_time.as_secs_f64() * 1000.0,
            remap_time.as_secs_f64() * 1000.0,
            root_time.as_secs_f64() * 1000.0,
            insert_time.as_secs_f64() * 1000.0,
            fur_time.as_secs_f64() * 1000.0,
            script_time.as_secs_f64() * 1000.0,
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
                ups, true_delta, self.test_val
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

    pub fn instantiate_script(ctor: CreateFn, node_id: Uuid) -> Rc<RefCell<Box<dyn ScriptObject>>> {
        let raw = ctor();
        let mut boxed: Box<dyn ScriptObject> = unsafe { Box::from_raw(raw) };
        boxed.set_node_id(node_id);

        let handle: Rc<RefCell<Box<dyn ScriptObject>>> = Rc::new(RefCell::new(boxed));

        handle
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
        println!("✅ Node {} added\n", id);

        // node is moved already, so get it back immutably from scene
        if let Some(node_ref) = self.data.nodes.get(&id) {
            if let Some(script_path) = node_ref.get_script_path() {
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
        const PARALLEL_THRESHOLD: usize = 50;

        if self.data.nodes.len() >= PARALLEL_THRESHOLD {
            self.data
                .nodes
                .par_iter()
                .filter_map(|(id, node)| if node.is_dirty() { Some(*id) } else { None })
                .collect()
        } else {
            self.data
                .nodes
                .iter()
                .filter_map(|(id, node)| if node.is_dirty() { Some(*id) } else { None })
                .collect()
        }
    }

    fn traverse_and_render(&mut self, dirty_nodes: Vec<Uuid>, gfx: &mut Graphics) {
        for node_id in dirty_nodes {
            if let Some(node) = self.data.nodes.get_mut(&node_id) {
                match node {
                    //2D Nodes
                    SceneNode::Sprite2D(sprite) => {
                        if let Some(tex) = &sprite.texture_path {
                            gfx.renderer_2d.queue_texture(
                                &mut gfx.renderer_prim,
                                &mut gfx.texture_manager,
                                &gfx.device,
                                &gfx.queue,
                                node_id,
                                tex,
                                sprite.transform,
                                sprite.pivot,
                                sprite.z_index,
                            );
                        }
                    }
                    SceneNode::Camera2D(camera) => {
                        if camera.active {
                            gfx.update_camera_2d(camera);
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
                                position: light.node_3d.transform.position.to_array(),
                                color: light.color.to_array(),
                                intensity: light.intensity,
                                ambient: [0.05, 0.05, 0.05],
                                ..Default::default()
                            },
                        );
                    }
                    SceneNode::DirectionalLight3D(light) => {
                        let dir = light.node_3d.transform.forward();
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
                        let dir = light.node_3d.transform.forward();
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

    fn instantiate_script(
        &mut self,
        ctor: CreateFn,
        node_id: Uuid,
    ) -> Rc<RefCell<Box<dyn ScriptObject>>> {
        Self::instantiate_script(ctor, node_id)
    }

    fn merge_nodes(&mut self, nodes: Vec<SceneNode>) {
        for mut node in nodes {
            let id = node.get_id();
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
        let game_root = game_scene.get_root().get_id();
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

        Ok(game_scene)
    }
}
