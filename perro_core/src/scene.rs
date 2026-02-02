use crate::{
    Graphics,
    Node,
    Shape2D,
    Transform3D,
    Vector2,
    api::ScriptApi,
    app_command::AppCommand,
    apply_fur::{build_ui_elements_from_fur, parse_fur_file},
    asset_io::{ProjectRoot, get_project_root, load_asset, save_asset, set_project_root},
    fur_ast::{FurElement, FurNode},
    input::joycon::ControllerManager,
    input::manager::InputManager,
    manifest::Project,
    node_arena::NodeArena,
    node_registry::{BaseNode, SceneNode},
    physics::physics_2d::PhysicsWorld2D,
    script::{CreateFn, SceneAccess, ScriptObject, ScriptProvider},
    transpiler::script_path_to_identifier,
    ui_renderer::render_ui, // NEW import
};
use once_cell::sync::OnceCell;
use std::sync::Mutex;

use crate::ids::{MeshID, NodeID, SignalID, TextureID};
use cow_map::CowMap;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    ser::{Error as SerdeError, SerializeStruct},
};
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    cell::{RefCell, UnsafeCell},
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::mpsc::Sender,
    time::Instant, // NEW import
};

//
// ---------------- SceneData ----------------
//

/// Pure serializable scene data (no runtime state)
/// Uses numeric node ids (keys) for the root and nodes map. No guaranteed order; use scene_key_order when order matters.
/// Nodes and key_to_node_id use CowMap so static scene data can be const-constructed.
#[derive(Debug)]
pub struct SceneData {
    pub root_key: u32,
    pub nodes: CowMap<u32, SceneNode>,
    /// Mapping from scene key to NodeID (used during deserialization).
    /// Not serialized - handled manually in Serialize/Deserialize impls.
    key_to_node_id: CowMap<u32, NodeID>,
}

/// Root first, then remaining keys in sorted order (deterministic for serialization/merge).
#[inline]
fn scene_key_order(root_key: u32, keys: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut rest: Vec<u32> = keys.filter(|&k| k != root_key).collect();
    rest.sort_unstable();
    std::iter::once(root_key).chain(rest).collect()
}

impl Clone for SceneData {
    fn clone(&self) -> Self {
        Self {
            root_key: self.root_key,
            nodes: CowMap::from(
                self.nodes
                    .iter()
                    .map(|(key, node)| (*key, node.clone()))
                    .collect::<HashMap<_, _>>(),
            ),
            key_to_node_id: CowMap::from(
                self.key_to_node_id
                    .iter()
                    .map(|(k, v)| (*k, *v))
                    .collect::<HashMap<_, _>>(),
            ),
        }
    }
}

impl Serialize for SceneData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SceneData", 2)?;
        // Serialize as root_id (id/key terminology); readers still accept root_index for backward compat
        state.serialize_field("root_id", &self.root_key)?;

        // Build reverse mapping: NodeID -> key (for converting parent NodeIDs to scene keys)
        let node_id_to_key: HashMap<NodeID, u32> = self
            .key_to_node_id
            .iter()
            .map(|(key, node_id)| (*node_id, *key))
            .collect();

        // Serialize nodes with u32 keys as identifiers
        struct NodesMap<'a> {
            nodes: &'a CowMap<u32, SceneNode>,
            node_id_to_key: &'a HashMap<NodeID, u32>,
        }

        impl<'a> Serialize for NodesMap<'a> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(self.nodes.len()))?;
                for (key, node) in self.nodes.iter() {
                    // Serialize node, but convert parent NodeID to scene key
                    let mut node_value: Value = serde_json::to_value(node).map_err(|e| {
                        S::Error::custom(format!("Failed to serialize node: {}", e))
                    })?;

                    // Convert parent NodeID to scene key if present
                    if let Some(obj) = node_value.as_object_mut() {
                        if let Some(parent_value) = obj.get_mut("parent") {
                            if let Some(parent_obj) = parent_value.as_object_mut() {
                                if let Some(id_value) = parent_obj.get("id") {
                                    if let Some(uid_str) = id_value.as_str() {
                                        if let Ok(node_id) = NodeID::parse_str(uid_str) {
                                            if let Some(&parent_key) =
                                                self.node_id_to_key.get(&node_id)
                                            {
                                                // Replace parent object with just the scene key
                                                *parent_value =
                                                    serde_json::Value::Number(parent_key.into());
                                            }
                                        }
                                    }
                                }
                            } else if let Some(uid_str) = parent_value.as_str() {
                                // Parent is a NodeID hex string, convert to scene key
                                if let Ok(node_id) = NodeID::parse_str(uid_str) {
                                    if let Some(&parent_key) = self.node_id_to_key.get(&node_id) {
                                        *parent_value =
                                            serde_json::Value::Number(parent_key.into());
                                    }
                                }
                            }
                        }
                    }

                    map.serialize_entry(key, &node_value)?;
                }
                map.end()
            }
        }

        state.serialize_field(
            "nodes",
            &NodesMap {
                nodes: &self.nodes,
                node_id_to_key: &node_id_to_key,
            },
        )?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SceneData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        // Deserialize as raw JSON first to extract parent scene keys
        let raw_value: Value = Value::deserialize(deserializer)?;

        // Accept root_id, root_key, or root_index (legacy) for compatibility
        let root_key = raw_value
            .get("root_id")
            .or_else(|| raw_value.get("root_key"))
            .or_else(|| raw_value.get("root_index"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                D::Error::custom("root_id, root_key, or root_index must be a number (u32)")
            })? as u32;

        let nodes_obj = raw_value
            .get("nodes")
            .and_then(|v| v.as_object())
            .ok_or_else(|| D::Error::custom("nodes must be an object"))?;

        let capacity = nodes_obj.len();

        // Create scene key -> NodeID mapping using deterministic NodeIDs based on keys
        // Use the key directly to generate a deterministic NodeID (with a small offset to avoid nil)
        let mut key_to_node_id: HashMap<u32, NodeID> = HashMap::with_capacity(capacity);
        for key_str in nodes_obj.keys() {
            if let Ok(key) = key_str.parse::<u32>() {
                // Generate deterministic NodeID from scene key (add 1 to avoid nil)
                let node_id = NodeID::from_u32(key.wrapping_add(1));
                key_to_node_id.insert(key, node_id);
            }
        }

        // Deserialize nodes, handling parent scene keys
        let mut nodes = HashMap::with_capacity(capacity);
        let mut parent_children: HashMap<u32, Vec<u32>> = HashMap::with_capacity(capacity / 4);

        // Helper function to recursively find "parent" field in nested JSON
        fn find_parent_recursive(value: &Value) -> Option<u32> {
            if let Some(obj) = value.as_object() {
                // Check if "parent" exists at this level (number or string scene key)
                if let Some(parent_val) = obj.get("parent") {
                    if let Some(n) = parent_val.as_u64() {
                        return Some(n as u32);
                    }
                    if let Some(s) = parent_val.as_str() {
                        if let Ok(n) = s.parse::<u32>() {
                            return Some(n);
                        }
                    }
                }
                // Recursively search in nested objects
                for (_, v) in obj {
                    if let Some(parent_key) = find_parent_recursive(v) {
                        return Some(parent_key);
                    }
                }
            } else if let Some(arr) = value.as_array() {
                for item in arr {
                    if let Some(parent_key) = find_parent_recursive(item) {
                        return Some(parent_key);
                    }
                }
            }
            None
        }

        for (key_str, node_value) in nodes_obj {
            let key = key_str.parse::<u32>().map_err(|_| {
                D::Error::custom(format!(
                    "Node key must be a u32 scene identifier, got: {}",
                    key_str
                ))
            })?;

            // Extract parent scene key if present (recursively search nested objects)
            let parent_key = find_parent_recursive(node_value);

            // Deserialize node without parent field (we'll set it later)
            // Need to recursively remove "parent" from nested "base" objects
            let mut node_json = node_value.clone();
            fn remove_parent_recursive(value: &mut Value) {
                if let Some(obj) = value.as_object_mut() {
                    obj.remove("parent");
                    // Recursively check nested objects (like "base")
                    for (_, v) in obj.iter_mut() {
                        remove_parent_recursive(v);
                    }
                } else if let Some(arr) = value.as_array_mut() {
                    for item in arr.iter_mut() {
                        remove_parent_recursive(item);
                    }
                }
            }
            remove_parent_recursive(&mut node_json);

            let mut node: SceneNode = serde_json::from_value(node_json)
                .map_err(|e| D::Error::custom(format!("Failed to deserialize node: {}", e)))?;

            // Set node ID to deterministic NodeID based on scene key
            if let Some(&node_id) = key_to_node_id.get(&key) {
                node.set_id(node_id);
            }

            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Store parent relationship for later
            if let Some(pkey) = parent_key {
                if key_to_node_id.contains_key(&pkey) {
                    parent_children.entry(pkey).or_default().push(key);
                }
            }

            nodes.insert(key, node);
        }

        // Second pass: set parent relationships with proper types and NodeIDs
        for (parent_key, child_keys) in parent_children {
            if let Some(&parent_node_id) = key_to_node_id.get(&parent_key) {
                if let Some(parent_node) = nodes.get(&parent_key) {
                    let parent_type_enum = parent_node.get_type();

                    for child_key in child_keys {
                        if let Some(child) = nodes.get_mut(&child_key) {
                            let parent_type = crate::nodes::node::ParentType::new(
                                parent_node_id,
                                parent_type_enum,
                            );
                            child.set_parent(Some(parent_type));
                        }
                        // Add to parent's children list (using NodeID)
                        // Only add if not already present to avoid duplicates
                        if let Some(parent) = nodes.get_mut(&parent_key) {
                            if let Some(&child_node_id) = key_to_node_id.get(&child_key) {
                                if !parent.get_children().contains(&child_node_id) {
                                    parent.add_child(child_node_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // script_exp_vars deserialize via ScriptExpVarValue: {"@node": 8} → NodeRef(NodeID::from_u32(8)), not a string.
        // to_runtime_nodes() and merge_scene_data() look for NodeRef (node IDs) and remap them to runtime IDs.

        // Store key_to_node_id mapping for later use when converting to runtime
        let key_to_node_id_map: HashMap<u32, NodeID> = key_to_node_id.into_iter().collect();

        Ok(SceneData {
            root_key,
            nodes: CowMap::from(nodes),
            key_to_node_id: CowMap::from(key_to_node_id_map),
        })
    }
}

impl SceneData {
    /// Get the scene key to NodeID mapping
    pub fn key_to_node_id(&self) -> &CowMap<u32, NodeID> {
        &self.key_to_node_id
    }

    /// Create a new data scene with a root node
    pub fn new(root: SceneNode) -> Self {
        let root_id = root.get_id();
        let mut nodes = HashMap::new();
        let mut key_to_node_id = HashMap::with_capacity(0);
        key_to_node_id.insert(0, root_id);
        nodes.insert(0, root);
        Self {
            root_key: 0,
            nodes: CowMap::from(nodes),
            key_to_node_id: CowMap::from(key_to_node_id),
        }
    }

    /// Create SceneData from nodes and key_to_node_id (const-friendly; no allocation).
    /// Use this for static scene data built with cow_map!.
    pub const fn from_nodes(
        root_key: u32,
        nodes: CowMap<u32, SceneNode>,
        key_to_node_id: CowMap<u32, NodeID>,
    ) -> Self {
        Self {
            root_key,
            nodes,
            key_to_node_id,
        }
    }

    /// Create SceneData from a HashMap of nodes (runtime/deserialize path).
    /// Builds key_to_node_id and sets node IDs to deterministic NodeIDs based on scene keys, then converts to CowMap.
    pub fn from_nodes_with_hashmap(root_key: u32, mut nodes: HashMap<u32, SceneNode>) -> Self {
        let mut key_to_node_id = HashMap::with_capacity(nodes.len());

        // Use scene key as NodeID (key 72 → NodeID(72)); no transformation.
        for (&key, node) in nodes.iter_mut() {
            let node_id = NodeID::from_u32(key);
            node.set_id(node_id);
            key_to_node_id.insert(key, node_id);
        }

        // Now update parent and child NodeIDs to match key_to_node_id.
        for node in nodes.values_mut() {
            if let Some(parent) = node.get_parent() {
                let parent_uid = parent.id.index();
                let parent_key_opt = key_to_node_id
                    .contains_key(&parent_uid)
                    .then_some(parent_uid);

                if let Some(parent_key) = parent_key_opt {
                    if let Some(&correct_node_id) = key_to_node_id.get(&parent_key) {
                        if parent.id != correct_node_id {
                            // Update parent ID to match
                            let parent_type = crate::nodes::node::ParentType::new(
                                correct_node_id,
                                parent.node_type,
                            );
                            node.set_parent(Some(parent_type));
                        }
                    }
                }
            }

            // Update children NodeIDs (to_vec() so we don't hold a borrow of node)
            let children: Vec<NodeID> = node.get_children().to_vec();
            node.clear_children();
            for child_id in children {
                let child_uid = child_id.index();
                let child_key_opt = key_to_node_id.contains_key(&child_uid).then_some(child_uid);

                if let Some(child_key) = child_key_opt {
                    if let Some(&correct_node_id) = key_to_node_id.get(&child_key) {
                        node.add_child(correct_node_id);
                    }
                }
            }
        }

        Self::from_nodes(root_key, CowMap::from(nodes), CowMap::from(key_to_node_id))
    }

    /// Convert SceneData to runtime Scene format.
    /// Inserts nodes in key order; arena assigns slot+generation for each. Remaps parent references.
    pub fn to_runtime_nodes(self) -> (NodeArena, NodeID) {
        use crate::ids::NodeID;
        // Order: root first, then rest sorted (deterministic).
        let key_order = scene_key_order(self.root_key, self.nodes.keys().copied());
        let mut nodes_owned: HashMap<u32, SceneNode> =
            self.nodes.iter().map(|(k, v)| (*k, v.clone())).collect();
        let mut old_to_new_id: HashMap<NodeID, NodeID> = HashMap::with_capacity(nodes_owned.len());
        let mut runtime_nodes = NodeArena::new();
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);

        // Insert in key order; arena assigns id from lowest open slot. Build old->new id map and parent_children.
        for &key in &key_order {
            let mut node = nodes_owned.remove(&key).expect("key in key_order");
            let old_node_id = *self
                .key_to_node_id
                .get(&key)
                .expect("key in key_to_node_id");
            let parent_old_id = node.get_parent().map(|p| p.id);
            node.clear_children();

            let new_id = runtime_nodes.insert(node);
            old_to_new_id.insert(old_node_id, new_id);
            if key != self.root_key {
                println!(
                    "  Scene key {}: old_node_id={} -> new_id={}",
                    key, old_node_id, new_id
                );
            }

            if let Some(po) = parent_old_id {
                if let Some(&new_parent_id) = old_to_new_id.get(&po) {
                    parent_children
                        .entry(new_parent_id)
                        .or_default()
                        .push(new_id);
                } else {
                    eprintln!(
                        "⚠️ WARNING: Parent ID {} not in old_to_new_id yet for node {} (key={}, name={})",
                        po,
                        new_id,
                        key,
                        runtime_nodes
                            .get(new_id)
                            .map(|n| n.get_name())
                            .unwrap_or_default()
                    );
                }
            }
        }

        // Second pass: set parent relationships with proper types
        for (parent_id, child_ids) in parent_children {
            if let Some(parent_node) = runtime_nodes.get(parent_id) {
                let parent_type_enum = parent_node.get_type();

                for child_id in child_ids {
                    if let Some(child) = runtime_nodes.get_mut(child_id) {
                        let parent_type =
                            crate::nodes::node::ParentType::new(parent_id, parent_type_enum);
                        child.set_parent(Some(parent_type));
                        // Debug: verify parent was set
                        if child.get_parent().is_none() {
                            eprintln!(
                                "⚠️ WARNING: Failed to set parent on child {} -> parent {}",
                                child_id, parent_id
                            );
                        }
                    }
                    // Add to parent's children list
                    if let Some(parent) = runtime_nodes.get_mut(parent_id) {
                        parent.add_child(child_id);
                    }
                }
            }
        }

        // Remap script_exp_vars NodeRef(scene_key) → NodeRef(runtime_id)
        use crate::nodes::node::ScriptExpVarValue;
        for node in runtime_nodes.values_mut() {
            if let Some(script_vars) = node.get_script_exp_vars_raw_mut() {
                let keys_to_remap: Vec<_> = script_vars
                    .iter()
                    .filter_map(|(k, v)| {
                        if let ScriptExpVarValue::NodeRef(scene_key_id) = v {
                            let key = scene_key_id.index();
                            if let Some(&old_id) = self.key_to_node_id.get(&key) {
                                if let Some(&new_id) = old_to_new_id.get(&old_id) {
                                    #[cfg(debug_assertions)]
                                    eprintln!(
                                        "[perro scene] remap script_exp_vars NodeRef(key {}) -> runtime {}",
                                        key, new_id
                                    );
                                    return Some((*k, new_id));
                                }
                            }
                        }
                        None
                    })
                    .collect();
                for (k, new_id) in keys_to_remap {
                    script_vars.insert(k, ScriptExpVarValue::NodeRef(new_id));
                }
            }
        }

        // Get root ID
        let root_old_node_id = *self.key_to_node_id.get(&self.root_key).expect("root_key");
        let root_id = old_to_new_id[&root_old_node_id];

        (runtime_nodes, root_id)
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
        // This function is kept for compatibility but relationships are now
        // handled during deserialization. Parent relationships in SceneData
        // use IDs from key_to_id, and children are already set.
        // This function can be used to verify/rebuild relationships if needed.

        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);

        // Collect parent node types
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_types: HashMap<NodeID, crate::node_registry::NodeType> =
            HashMap::with_capacity(0);

        for (&_key, node) in data.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a NodeID, find which scene key it corresponds to
                let parent_key_opt = data
                    .key_to_node_id
                    .iter()
                    .find(|&(_, &node_id)| node_id == parent.id)
                    .map(|(&key, _)| key);

                if let Some(parent_key) = parent_key_opt {
                    if let Some(parent_node) = data.nodes.get(&parent_key) {
                        parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }

        // Rebuild parent-child relationships
        for (&key, node) in data.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                let parent_id = parent.id;
                // Find parent scene key
                let parent_key_opt = data
                    .key_to_node_id
                    .iter()
                    .find(|&(_, &node_id)| node_id == parent_id)
                    .map(|(&key, _)| key);

                if let Some(_parent_key) = parent_key_opt {
                    let node_id = *data.key_to_node_id.get(&key).expect("key");
                    parent_children.entry(parent_id).or_default().push(node_id);
                }
            }
        }

        // Apply relationships
        for (parent_id, children) in parent_children {
            // Find parent node by NodeID
            let parent_key_opt = data
                .key_to_node_id
                .iter()
                .find(|&(_, &node_id)| node_id == parent_id)
                .map(|(&key, _)| key);

            if let Some(parent_key) = parent_key_opt {
                if let Some(parent) = data.nodes.get_mut(&parent_key) {
                    for child_id in children {
                        parent.add_child(child_id);
                    }
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
    pub(crate) nodes: NodeArena,
    pub(crate) root_id: NodeID,
    pub signals: SignalBus,
    queued_signals: Vec<(SignalID, SmallVec<[Value; 3]>)>,
    queued_calls: Vec<(NodeID, u64, SmallVec<[Value; 3]>)>,
    /// Scripts stored as Rc<UnsafeCell<Box<dyn ScriptObject>>>
    ///
    /// SAFETY: Using UnsafeCell is safe because:
    /// - All script access is controlled by the ScriptApi
    /// - Scripts are never accessed directly by user code
    /// - All execution is synchronous and single-threaded
    /// - The transpiler ensures all script code goes through the API
    /// - Nested calls are safe because they're part of the same synchronous call chain
    /// - Variable access (get/set) is safe because it's controlled by the API
    pub scripts: FxHashMap<NodeID, Rc<UnsafeCell<Box<dyn ScriptObject>>>>,
    pub provider: P,
    pub project: Rc<RefCell<Project>>,
    pub app_command_tx: Option<Sender<AppCommand>>, // NEW field
    // OPTIMIZED: Lazy controller manager - only create when explicitly enabled
    // Using OnceCell for thread-safe lazy initialization
    pub controller_manager: OnceCell<Mutex<ControllerManager>>, // Controller input manager
    pub controller_enabled: std::sync::atomic::AtomicBool, // Flag to track if controllers are enabled
    pub input_manager: Mutex<InputManager>,                // Keyboard/mouse input manager

    pub last_scene_update: Option<Instant>,
    pub delta_accum: f32,
    pub true_updates: i32,

    // Fixed update timing
    pub fixed_update_accumulator: f32,
    pub last_fixed_update: Option<Instant>,

    // Render timing (for draw() methods - tracks actual frame time, not update time)
    // Optimize: Use HashSet for O(1) contains() checks (order doesn't matter for fixed updates)
    pub nodes_with_internal_fixed_update: HashSet<NodeID>,
    // Optimize: Use HashSet for O(1) contains() checks (order doesn't matter for render updates)
    pub nodes_with_internal_render_update: HashSet<NodeID>,

    // Physics (wrapped in RefCell for interior mutability through trait objects)
    // OPTIMIZED: Lazy initialization - only create when first physics object is added
    pub physics_2d: Option<std::cell::RefCell<PhysicsWorld2D>>,

    // OPTIMIZED: Cache script IDs to avoid Vec allocation every frame
    cached_script_ids: Vec<NodeID>,
    scripts_dirty: bool,

    // OPTIMIZED: Separate vectors for scripts with update/fixed_update to avoid checking all scripts
    scripts_with_update: Vec<NodeID>,
    scripts_with_fixed_update: Vec<NodeID>,

    // Track if texture_path → texture_id conversion has been done
    textures_converted: bool,

    // OPTIMIZED: Pre-accumulated set of node IDs that need rerendering
    needs_rerender: HashSet<NodeID>,
}

#[derive(Default)]
pub struct SignalBus {
    // signal_id → { script_uuid → SmallVec<[u64; 4]> (function_ids) }
    pub connections: HashMap<SignalID, HashMap<NodeID, SmallVec<[u64; 4]>>>,
}

impl<P: ScriptProvider + 'static> Scene<P> {
    /// Check if any UINode has a focused text input element
    pub fn has_focused_text_input(&self) -> bool {
        for node in self.nodes.values() {
            if let SceneNode::UINode(ui_node) = node {
                if ui_node.focused_element.is_some() {
                    return true;
                }
            }
        }
        false
    }

    /// Create a runtime scene from a root node.
    /// Arena assigns the root's ID from the next available slot.
    pub fn new(root: SceneNode, provider: P, project: Rc<RefCell<Project>>) -> Self {
        let mut nodes = NodeArena::new();
        let root_id = nodes.insert(root);

        Self {
            textures_converted: false,
            nodes,
            root_id,
            signals: SignalBus::default(),
            queued_signals: Vec::new(),
            queued_calls: Vec::new(),
            scripts: FxHashMap::default(),
            provider,
            project,
            app_command_tx: None,
            // OPTIMIZED: Lazy controller manager initialization
            controller_manager: OnceCell::new(),
            controller_enabled: std::sync::atomic::AtomicBool::new(false),
            input_manager: Mutex::new(InputManager::new()),

            last_scene_update: Some(Instant::now()),
            delta_accum: 0.0,
            true_updates: 0,
            fixed_update_accumulator: 0.0,
            last_fixed_update: Some(Instant::now()),
            nodes_with_internal_fixed_update: HashSet::new(),
            nodes_with_internal_render_update: HashSet::new(),
            // OPTIMIZED: Lazy physics initialization - only create when needed
            physics_2d: None,

            // OPTIMIZED: Initialize script ID cache
            cached_script_ids: Vec::new(),
            scripts_dirty: true,

            // OPTIMIZED: Initialize separate vectors for update/fixed_update/draw scripts
            scripts_with_update: Vec::new(),
            scripts_with_fixed_update: Vec::new(),

            // OPTIMIZED: Initialize nodes needing rerender tracking
            needs_rerender: HashSet::new(),
        }
    }

    /// Create a runtime scene from serialized data
    pub fn from_data(data: SceneData, provider: P, project: Rc<RefCell<Project>>) -> Self {
        // Convert SceneData to runtime format
        let (mut nodes, root_id) = data.to_runtime_nodes();

        // Mark all nodes as transform_dirty when loading from data
        for node in nodes.values_mut() {
            node.mark_transform_dirty_if_node2d();
        }

        // Note: texture_path → texture_id conversion happens lazily during first render
        // when Graphics is available (see convert_texture_paths_to_ids)

        Self {
            textures_converted: false,
            nodes,
            root_id,
            signals: SignalBus::default(),
            queued_signals: Vec::new(),
            queued_calls: Vec::new(),
            scripts: FxHashMap::default(),
            // OPTIMIZED: Lazy physics initialization - only create when needed
            physics_2d: None,
            provider,
            project,
            app_command_tx: None,
            // OPTIMIZED: Lazy controller manager initialization
            controller_manager: OnceCell::new(),
            controller_enabled: std::sync::atomic::AtomicBool::new(false),
            input_manager: Mutex::new(InputManager::new()),

            last_scene_update: Some(Instant::now()),
            delta_accum: 0.0,
            true_updates: 0,
            fixed_update_accumulator: 0.0,
            last_fixed_update: Some(Instant::now()),
            nodes_with_internal_fixed_update: HashSet::new(),
            nodes_with_internal_render_update: HashSet::new(),

            // OPTIMIZED: Initialize script ID cache
            cached_script_ids: Vec::new(),
            scripts_dirty: true,

            // OPTIMIZED: Initialize separate vectors for update/fixed_update/draw scripts
            scripts_with_update: Vec::new(),
            scripts_with_fixed_update: Vec::new(),

            // OPTIMIZED: Initialize nodes needing rerender tracking
            needs_rerender: HashSet::new(),
        }
    }

    /// Load a runtime scene from disk or pak
    pub fn load(res_path: &str, provider: P, project: Rc<RefCell<Project>>) -> io::Result<Self> {
        let data = SceneData::load(res_path)?;
        Ok(Scene::from_data(data, provider, project))
    }

    /// Get or initialize the physics world (lazy initialization)
    /// OPTIMIZED: Only creates physics world when first needed
    fn get_or_init_physics_2d(&mut self) -> &mut std::cell::RefCell<PhysicsWorld2D> {
        if self.physics_2d.is_none() {
            self.physics_2d = Some(std::cell::RefCell::new(PhysicsWorld2D::new()));
        }
        self.physics_2d.as_mut().unwrap()
    }

    /// Debug method to check if physics is initialized
    pub fn is_physics_initialized(&self) -> bool {
        self.physics_2d.is_some()
    }

    /// Debug: Print physics initialization status
    pub fn debug_physics_status(&self) {
        if self.physics_2d.is_some() {
            println!(
                "⚠️ PhysicsWorld2D is INITIALIZED (should be None for projects without physics)"
            );
        } else {
            println!("✅ PhysicsWorld2D is NOT initialized (correct for projects without physics)");
        }
    }

    /// Convert runtime Scene to SceneData for serialization
    /// Assigns u32 scene keys to nodes based on traversal order
    pub fn to_scene_data(&self) -> SceneData {
        // Assign scene keys based on traversal order (root first, then children)
        let mut key = 0u32;
        // OPTIMIZED: Use with_capacity(0) for known-empty maps initially
        let mut node_id_to_key: HashMap<NodeID, u32> = HashMap::with_capacity(0);
        let mut nodes = HashMap::new();
        let mut key_to_node_id: HashMap<u32, NodeID> = HashMap::with_capacity(0);

        // Traverse tree starting from root
        let mut to_process = vec![self.root_id];
        while let Some(node_id) = to_process.pop() {
            if node_id_to_key.contains_key(&node_id) {
                continue; // Already processed
            }

            if let Some(node) = self.nodes.get(node_id) {
                node_id_to_key.insert(node_id, key);
                key_to_node_id.insert(key, node_id);
                nodes.insert(key, node.clone());

                // Add children to processing queue
                for child_id in node.get_children() {
                    to_process.push(*child_id);
                }

                key += 1;
            }
        }

        // Find root scene key
        let root_key = node_id_to_key.get(&self.root_id).copied().unwrap_or(0);

        // Convert parent NodeIDs to match key_to_node_id (so serialization can find them)
        // The parent.id should be the NodeID from key_to_node_id for that parent's scene key
        for (_key, node) in nodes.iter_mut() {
            if let Some(parent) = node.get_parent() {
                // Find which scene key this parent NodeID corresponds to
                if let Some(&parent_key) = node_id_to_key.get(&parent.id) {
                    // Get the NodeID from key_to_node_id for this parent scene key
                    if let Some(&parent_node_id_from_key) = key_to_node_id.get(&parent_key) {
                        // Update parent.id to match the NodeID in key_to_node_id
                        // This ensures serialization can find it in the reverse mapping
                        let parent_type = crate::nodes::node::ParentType::new(
                            parent_node_id_from_key,
                            parent.node_type,
                        );
                        node.set_parent(Some(parent_type));
                    }
                }
            }
        }

        SceneData {
            root_key,
            nodes: CowMap::from(nodes),
            key_to_node_id: CowMap::from(key_to_node_id),
        }
    }

    /// Save scene to disk
    pub fn save(&self, res_path: &str) -> io::Result<()> {
        let data = self.to_scene_data();
        data.save(res_path)
    }

    /// Convert texture_path → texture_id for all nodes that have texture_path
    /// This is called during the first render when Graphics is available
    /// Loads textures into TextureManager and sets texture_id on nodes
    /// Uses EngineRegistry to find all node types with texture_path field
    fn convert_texture_paths_to_ids(&mut self, gfx: &mut Graphics) {
        use crate::structs::engine_registry::ENGINE_REGISTRY;

        // Find all node types that have texture_path field
        let nodes_with_texture_path = ENGINE_REGISTRY.find_nodes_with_field("texture_path");

        for (_node_id, node) in self.nodes.iter_mut() {
            let node_type = node.get_type();

            // Only process nodes that have texture_path field
            if !nodes_with_texture_path.contains(&node_type) {
                continue;
            }

            // Handle each node type that has texture_path
            match node {
                crate::nodes::node_registry::SceneNode::Sprite2D(sprite) => {
                    // Only convert if texture_path exists and texture_id is not already set
                    if sprite.texture_path.is_some() && sprite.texture_id.is_none() {
                        if let Some(path) = &sprite.texture_path {
                            // Load texture and get its UUID
                            match gfx.texture_manager.get_or_load_texture_id(
                                path,
                                &gfx.device,
                                &gfx.queue,
                            ) {
                                Ok(texture_id) => {
                                    sprite.texture_id = Some(texture_id);
                                }
                                Err(e) => {
                                    eprintln!("Failed to load texture '{}': {}", path, e);
                                    // Continue without setting texture_id - sprite will render without texture
                                }
                            }
                        }
                    }
                }
                // Add other node types here as they get texture_path support
                // For example:
                // crate::nodes::node_registry::SceneNode::SomeOtherNodeType(node) => {
                //     if node.texture_path.is_some() && node.texture_id.is_none() {
                //         if let Some(path) = &node.texture_path {
                //             let texture_id = gfx.texture_manager.get_or_load_texture_id(
                //                 path, &gfx.device, &gfx.queue);
                //             node.texture_id = Some(texture_id);
                //         }
                //     }
                // }
                _ => {
                    // Node type has texture_path in registry but we don't handle it yet
                    // This is fine - it will be handled when that node type is implemented
                }
            }
        }
    }

    /// Build a runtime scene from a project with a given provider
    /// Used for StaticScriptProvider (export builds) and also DLL provider (via delegation)
    pub fn from_project_with_provider(
        project: Rc<RefCell<Project>>,
        provider: P,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<Self> {
        // Create game root (id 1) — Root, owns main scene, has root script.
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

        // Global order: index 0 = @root script (attaches to Root node NodeID(1)); indices 1.. = @global scripts (siblings of Root).
        let global_order: Vec<String> = game_scene
            .provider
            .get_global_registry_order()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let global_names: Vec<String> = game_scene
            .provider
            .get_global_registry_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let root_id = game_scene.get_root().get_id();
        let mut global_node_ids: Vec<NodeID> = Vec::with_capacity(global_order.len());
        let first_is_root = global_names.first().map(|s| s.as_str()).unwrap_or("") == "Root";
        for (i, identifier) in global_order.iter().enumerate() {
            let name = global_names
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or(identifier.as_str());
            if i == 0 && first_is_root {
                // Index 0 = Root script; attach to root node, don't create a new node.
                global_node_ids.push(root_id);
            } else {
                let mut node = Node::new();
                node.name = Cow::Owned(name.to_string());
                let global_node = SceneNode::Node(node);
                let global_id = game_scene.nodes.insert(global_node);
                global_node_ids.push(global_id);
            }
        }

        // ✅ attach global scripts in order (index 0 = root script on root node when present) and call init
        for (identifier, &global_id) in global_order.iter().zip(global_node_ids.iter()) {
            if let Ok(ctor) = game_scene.provider.load_ctor(identifier.as_str()) {
                let boxed = game_scene.instantiate_script(ctor, global_id);
                let handle = Rc::new(UnsafeCell::new(boxed));
                game_scene.scripts.insert(global_id, handle);

                let project_ref = game_scene.project.clone();
                let mut project_borrow = project_ref.borrow_mut();
                let now = Instant::now();
                let true_delta = match game_scene.last_scene_update {
                    Some(prev) => now.duration_since(prev).as_secs_f32(),
                    None => 0.0,
                };
                let mut api =
                    ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow, gfx);
                api.apply_exposed_vars_from_node(global_id);
                api.call_init(global_id);
                if let Some(node) = game_scene.nodes.get(global_id) {
                    if node.is_renderable() {
                        game_scene.needs_rerender.insert(global_id);
                    }
                }
            }
        }

        // ✅ main scene second — timer: retrieve (phf lookup + clone) + remap only
        let main_scene_path: String = {
            let proj_ref = game_scene.project.borrow();
            let path = proj_ref.main_scene().to_string();
            path
        };

        let scene_load_start = Instant::now();
        let loaded_data = game_scene.provider.load_scene_data(&main_scene_path)?;
        let load_ms = scene_load_start.elapsed().as_secs_f64() * 1000.0;

        let merge_start = Instant::now();
        let game_root = game_scene.get_root().get_id();
        game_scene.merge_scene_data(loaded_data, game_root, gfx)?;
        let merge_ms = merge_start.elapsed().as_secs_f64() * 1000.0;

        let total_ms = scene_load_start.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "⏱️ Scene (retrieve+remap): load {:.2}ms, merge {:.2}ms, total {:.2}ms",
            load_ms, merge_ms, total_ms
        );
        Ok(game_scene)
    }

    pub fn merge_scene_data(
        &mut self,
        other: SceneData,
        parent_id: NodeID,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<()> {
        use std::time::Instant;

        use crate::ids::NodeID;
        let id_map_start = Instant::now();
        let root_key = other.root_key;
        let key_to_node_id_copy: HashMap<u32, NodeID> = other
            .key_to_node_id()
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();
        // Reverse map for O(1) parent key lookup (avoids O(n) find per node in other_parent_types)
        let node_id_to_key: HashMap<NodeID, u32> = key_to_node_id_copy
            .iter()
            .map(|(&k, &n)| (n, k))
            .collect();

        // Check if root has is_root_of (skip inserting root when true)
        let skip_root = other
            .nodes
            .get(&other.root_key)
            .and_then(|n| Self::get_is_root_of(n))
            .is_some();

        // Collect parent types from other before we consume nodes (O(n) with reverse map)
        let mut other_parent_types: HashMap<NodeID, crate::node_registry::NodeType> =
            HashMap::with_capacity(other.nodes.len());
        for (&_key, node) in other.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                if let Some(&parent_key) = node_id_to_key.get(&parent.id) {
                    if let Some(parent_node) = other.nodes.get(&parent_key) {
                        other_parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }

        // Key order: root first, then rest sorted (deterministic)
        let key_order = scene_key_order(root_key, other.nodes.keys().copied());

        // Owned copy so we can remove (CowMap may be borrowed). Clone nodes in parallel.
        let refs: Vec<(u32, &SceneNode)> = other.nodes.iter().map(|(k, v)| (*k, v)).collect();
        let mut other_nodes_owned: HashMap<u32, SceneNode> = refs
            .par_iter()
            .map(|(k, r)| (*k, (*r).clone()))
            .collect::<Vec<_>>()
            .into_iter()
            .collect();

        let mut old_node_id_to_new_node_id: HashMap<NodeID, NodeID> =
            HashMap::with_capacity(other.nodes.len() + 1);
        let mut key_to_new_id: HashMap<u32, NodeID> = HashMap::with_capacity(other.nodes.len() + 1);
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);
        // (key, new_id, parent_old_id) for fixing parent/children after insert
        let mut insert_info: Vec<(u32, NodeID, Option<NodeID>)> =
            Vec::with_capacity(other.nodes.len());

        // 1️⃣ INSERT: arena assigns next available slot for each node (root first, then rest; skip root if is_root_of)
        for &key in &key_order {
            if skip_root && key == root_key {
                continue;
            }
            let mut node = other_nodes_owned.remove(&key).expect("key in key_order");
            let old_node_id = key_to_node_id_copy[&key];
            let parent_old_id = node.get_parent().map(|p| p.id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            let new_id = self.nodes.insert(node);
            old_node_id_to_new_node_id.insert(old_node_id, new_id);
            key_to_new_id.insert(key, new_id);
            insert_info.push((key, new_id, parent_old_id));
        }
        let _ = id_map_start.elapsed();
        let remap_start = Instant::now();

        // 2️⃣ FIX PARENT/CHILDREN using ids from arena
        for (key, new_id, parent_old_id) in &insert_info {
            if let Some(po) = parent_old_id {
                if let Some(&pnew) = old_node_id_to_new_node_id.get(po) {
                    let parent_type = other_parent_types.get(po).copied().unwrap_or_else(|| {
                        self.nodes
                            .get(pnew)
                            .map(|n| n.get_type())
                            .unwrap_or(crate::node_registry::NodeType::Node)
                    });
                    if let Some(node) = self.nodes.get_mut(*new_id) {
                        node.set_parent(Some(crate::nodes::node::ParentType::new(
                            pnew,
                            parent_type,
                        )));
                    }
                    if let Some(parent_node) = self.nodes.get_mut(pnew) {
                        parent_node.add_child(*new_id);
                    }
                }
            } else if *key == root_key && !skip_root {
                let parent_type = self
                    .nodes
                    .get(parent_id)
                    .map(|n| n.get_type())
                    .unwrap_or(crate::node_registry::NodeType::Node);
                if let Some(node) = self.nodes.get_mut(*new_id) {
                    node.set_parent(Some(crate::nodes::node::ParentType::new(
                        parent_id,
                        parent_type,
                    )));
                }
                parent_children.entry(parent_id).or_default().push(*new_id);
            }
        }

        // Remap script_exp_vars NodeRef(scene_key) → NodeRef(new arena ID)
        use crate::nodes::node::ScriptExpVarValue;
        for (_, new_id, _) in &insert_info {
            if let Some(node) = self.nodes.get_mut(*new_id) {
                if let Some(script_vars) = node.get_script_exp_vars_raw_mut() {
                    let keys_to_remap: Vec<_> = script_vars
                        .iter()
                        .filter_map(|(k, v)| {
                            if let ScriptExpVarValue::NodeRef(scene_key_id) = v {
                                let key = scene_key_id.index();
                                if let Some(&new_node_id) = key_to_new_id.get(&key) {
                                    #[cfg(debug_assertions)]
                                    eprintln!(
                                        "[perro scene] merge remap script_exp_vars NodeRef(key {}) -> {}",
                                        key, new_node_id
                                    );
                                    return Some((*k, new_node_id));
                                }
                            }
                            None
                        })
                        .collect();
                    for (k, new_node_id) in keys_to_remap {
                        script_vars.insert(k, ScriptExpVarValue::NodeRef(new_node_id));
                    }
                }
            }
        }

        let _ = remap_start.elapsed();
        let insert_start = Instant::now();

        let inserted_ids: Vec<NodeID> = insert_info.iter().map(|(_, id, _)| *id).collect();

        // Resolve name conflicts (need &self for conflict checks, so collect then apply)
        let mut renames: Vec<(NodeID, String)> = Vec::new();
        for (_key, new_id, parent_old_id) in &insert_info {
            let parent_id_opt =
                parent_old_id.and_then(|po| old_node_id_to_new_node_id.get(&po).copied());
            if let Some(node) = self.nodes.get(*new_id) {
                let node_name = node.get_name();
                let has_sibling_conflict = parent_id_opt
                    .map(|pid| self.has_sibling_name_conflict(pid, node_name, Some(*new_id)))
                    .unwrap_or(false);
                let has_parent_conflict =
                    self.has_parent_or_ancestor_name_conflict(parent_id_opt, node_name);
                if has_sibling_conflict || has_parent_conflict {
                    let resolved_name = parent_id_opt
                        .map(|pid| self.resolve_name_conflict(pid, node_name))
                        .unwrap_or_else(|| node_name.to_string());
                    renames.push((*new_id, resolved_name));
                }
            }
        }
        for (id, resolved_name) in renames {
            if let Some(node) = self.nodes.get_mut(id) {
                Self::set_node_name(node, resolved_name);
            }
        }
        // Single pass: mark inserted nodes for rerender / internal update sets
        for (_, new_id, _) in &insert_info {
            if let Some(node_ref) = self.nodes.get(*new_id) {
                if node_ref.is_renderable() {
                    self.needs_rerender.insert(*new_id);
                }
                if node_ref.needs_internal_fixed_update() {
                    self.nodes_with_internal_fixed_update.insert(*new_id);
                }
                if node_ref.needs_internal_render_update() {
                    self.nodes_with_internal_render_update.insert(*new_id);
                }
            }
        }

        if let Some(children_of_game_parent) = parent_children.get(&parent_id) {
            if let Some(game_parent) = self.nodes.get_mut(parent_id) {
                for child_id in children_of_game_parent {
                    if !game_parent.get_children().contains(child_id) {
                        game_parent.add_child(*child_id);
                    }
                }
            }
        }

        for id in &inserted_ids {
            self.mark_transform_dirty_recursive(*id);
        }
        let _ = insert_start.elapsed();

        // ───────────────────────────────────────────────
        // 4️⃣ HANDLE is_root_of SCENE REFERENCES (RECURSIVE)
        // ───────────────────────────────────────────────
        let nested_scene_start = Instant::now();
        let mut nodes_with_nested_scenes: Vec<(NodeID, String)> = Vec::new();

        // Collect nodes with is_root_of from newly inserted nodes
        for id in &inserted_ids {
            if let Some(node) = self.nodes.get(*id) {
                if let Some(scene_path) = Self::get_is_root_of(node) {
                    nodes_with_nested_scenes.push((*id, scene_path));
                }
            }
        }

        // Load all nested scene data in parallel (I/O or static lookup); then merge sequentially
        let provider = &self.provider;
        let nested_loads: Vec<(NodeID, String, io::Result<SceneData>)> = nodes_with_nested_scenes
            .par_iter()
            .map(|(parent_node_id, scene_path)| {
                let result = provider.load_scene_data(scene_path);
                (*parent_node_id, scene_path.clone(), result)
            })
            .collect();

        for (parent_node_id, scene_path, result) in nested_loads {
            match result {
                Ok(nested_scene_data) => {
                    if let Err(e) = self.merge_scene_data_with_root_replacement(
                        nested_scene_data,
                        parent_node_id,
                        gfx,
                    ) {
                        eprintln!("⚠️ Error merging nested scene '{}': {}", scene_path, e);
                    }
                }
                Err(_) => eprintln!("⚠️ Failed to load nested scene: {}", scene_path),
            }
        }

        let _nested_scene_time = nested_scene_start.elapsed();
        let _nested_scene_count = nodes_with_nested_scenes.len();

        // ───────────────────────────────────────────────
        // 5️⃣ REGISTER COLLISION SHAPES WITH PHYSICS
        // ───────────────────────────────────────────────
        self.register_collision_shapes(&inserted_ids);

        // ───────────────────────────────────────────────
        // 6️⃣ FUR LOADING (UI FILES)
        // ───────────────────────────────────────────────
        // Collect FUR paths
        let fur_paths: Vec<(NodeID, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.nodes.get(*id).and_then(|node| {
                    if let SceneNode::UINode(u) = node {
                        u.fur_path.as_ref().map(|path| (*id, path.to_string()))
                    } else {
                        None
                    }
                })
            })
            .collect();

        // Load FUR data - always parallelize (I/O operations benefit even more)
        let fur_loads: Vec<(NodeID, Result<Vec<FurElement>, std::io::Error>)> =
            if fur_paths.len() > 1 {
                fur_paths
                    .par_iter()
                    .map(|(id, fur_path)| {
                        let result = self.provider.load_fur_data(fur_path);
                        (*id, result)
                    })
                    .collect()
            } else if let Some((id, fur_path)) = fur_paths.first() {
                vec![(*id, self.provider.load_fur_data(fur_path))]
            } else {
                Vec::new()
            };

        // Apply FUR results
        for (id, result) in fur_loads {
            if let Some(node) = self.nodes.get_mut(id) {
                if let SceneNode::UINode(u) = node {
                    match result {
                        Ok(fur_elements) => {
                            build_ui_elements_from_fur(u, &fur_elements);
                            // Mark UINode as needing rerender after elements are created
                            if u.is_renderable() {
                                self.needs_rerender.insert(id);
                            }
                        }
                        Err(err) => eprintln!("⚠️ Error loading FUR for {}: {}", id, err),
                    }
                }
            }
        }

        // ───────────────────────────────────────────────
        // 7️⃣ SCRIPT INITIALIZATION
        // ───────────────────────────────────────────────
        // Collect script paths
        let script_targets: Vec<(NodeID, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.nodes
                    .get(*id)
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
                let boxed = self.instantiate_script(ctor, id);
                let handle = Rc::new(UnsafeCell::new(boxed));

                // Check flags and add to appropriate vectors
                let flags = unsafe { (*handle.get()).script_flags() };

                if flags.has_update() && !self.scripts_with_update.contains(&id) {
                    self.scripts_with_update.push(id);
                }
                if flags.has_fixed_update() && !self.scripts_with_fixed_update.contains(&id) {
                    self.scripts_with_fixed_update.push(id);
                }

                self.scripts.insert(id, handle);
                self.scripts_dirty = true;

                let mut api = ScriptApi::new(dt, self, &mut *project_borrow, gfx);
                api.apply_exposed_vars_from_node(id);
                api.call_init(id);

                // After script initialization, ensure renderable nodes are marked for rerender
                // (old system would have called mark_dirty() here)
                if let Some(node) = self.nodes.get(id) {
                    if node.is_renderable() {
                        self.needs_rerender.insert(id);
                    }
                }
            }
        }

        // ───────────────────────────────────────────────
        // 8️⃣ PERFORMANCE SUMMARY
        // ───────────────────────────────────────────────

        // Print scene tree after merge
        self.print_scene_tree();

        Ok(())
    }

    /// Merge a nested scene where the nested scene's root REPLACES an existing node
    /// (used for is_root_of scenarios). Non-root nodes get IDs from arena (next available slot).
    fn merge_scene_data_with_root_replacement(
        &mut self,
        other: SceneData,
        replacement_root_id: NodeID,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<()> {
        use crate::ids::NodeID;
        let subscene_root_key = other.root_key;
        let key_to_node_id_copy: HashMap<u32, NodeID> = other
            .key_to_node_id()
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();

        // Owned copy so we can remove (CowMap may be borrowed)
        let mut other_nodes_owned: HashMap<u32, SceneNode> =
            other.nodes.iter().map(|(k, v)| (*k, v.clone())).collect();

        let mut other_parent_types: HashMap<NodeID, crate::node_registry::NodeType> =
            HashMap::with_capacity(0);
        for (&_key, node) in other.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                let parent_key_opt = other
                    .key_to_node_id()
                    .iter()
                    .find(|&(_, &node_id)| node_id == parent.id)
                    .map(|(&key, _)| key);
                if let Some(parent_key) = parent_key_opt {
                    if let Some(parent_node) = other.nodes.get(&parent_key) {
                        other_parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }

        let mut old_node_id_to_new_id: HashMap<NodeID, NodeID> =
            HashMap::with_capacity(other.nodes.len());
        let mut key_to_new_id: HashMap<u32, NodeID> = HashMap::with_capacity(other.nodes.len());
        old_node_id_to_new_id.insert(key_to_node_id_copy[&subscene_root_key], replacement_root_id);
        key_to_new_id.insert(subscene_root_key, replacement_root_id);

        let key_order = scene_key_order(subscene_root_key, other.nodes.keys().copied());
        let mut insert_info: Vec<(u32, NodeID, Option<NodeID>)> = Vec::new();

        // Insert non-root nodes; arena assigns next available slot. Root is not inserted (replaced by replacement_root_id).
        for &key in &key_order {
            if key == subscene_root_key {
                continue;
            }
            let mut node = other_nodes_owned.remove(&key).expect("key in key_order");
            let old_node_id = key_to_node_id_copy[&key];
            let parent_old_id = node.get_parent().map(|p| p.id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            let new_id = self.nodes.insert(node);
            old_node_id_to_new_id.insert(old_node_id, new_id);
            key_to_new_id.insert(key, new_id);
            insert_info.push((key, new_id, parent_old_id));
        }

        // Fix parent/children using arena-assigned ids
        for (_key, new_id, parent_old_id) in &insert_info {
            if let Some(po) = parent_old_id {
                if let Some(&pnew) = old_node_id_to_new_id.get(po) {
                    let parent_type = other_parent_types.get(po).copied().unwrap_or_else(|| {
                        self.nodes
                            .get(pnew)
                            .map(|n| n.get_type())
                            .unwrap_or(crate::node_registry::NodeType::Node)
                    });
                    if let Some(node) = self.nodes.get_mut(*new_id) {
                        node.set_parent(Some(crate::nodes::node::ParentType::new(
                            pnew,
                            parent_type,
                        )));
                    }
                    if pnew == replacement_root_id {
                        if let Some(existing_node) = self.nodes.get_mut(replacement_root_id) {
                            existing_node.add_child(*new_id);
                        }
                    } else if let Some(parent_node) = self.nodes.get_mut(pnew) {
                        parent_node.add_child(*new_id);
                    }
                } else {
                    let (parent_runtime_id, parent_type) = self
                        .nodes
                        .get(*po)
                        .map(|n| (n.get_id(), n.get_type()))
                        .unwrap_or((*po, crate::node_registry::NodeType::Node));
                    if let Some(node) = self.nodes.get_mut(*new_id) {
                        node.set_parent(Some(crate::nodes::node::ParentType::new(
                            parent_runtime_id,
                            parent_type,
                        )));
                    }
                    if let Some(p) = self.nodes.get_mut(parent_runtime_id) {
                        p.add_child(*new_id);
                    }
                }
            }
        }

        use crate::nodes::node::ScriptExpVarValue;
        for (_, new_id, _) in &insert_info {
            if let Some(node) = self.nodes.get_mut(*new_id) {
                if let Some(script_vars) = node.get_script_exp_vars_raw_mut() {
                    let keys_to_remap: Vec<_> = script_vars
                        .iter()
                        .filter_map(|(k, v)| {
                            if let ScriptExpVarValue::NodeRef(scene_key_id) = v {
                                let key = scene_key_id.index();
                                key_to_new_id
                                    .get(&key)
                                    .map(|&new_node_id| (*k, new_node_id))
                            } else {
                                None
                            }
                        })
                        .collect();
                    for (k, new_node_id) in keys_to_remap {
                        script_vars.insert(k, ScriptExpVarValue::NodeRef(new_node_id));
                    }
                }
            }
        }

        let inserted_ids: Vec<NodeID> = insert_info.iter().map(|(_, id, _)| *id).collect();

        let mut renames: Vec<(NodeID, String)> = Vec::new();
        for (_key, new_id, parent_old_id) in &insert_info {
            let parent_id_opt =
                parent_old_id.and_then(|po| old_node_id_to_new_id.get(&po).copied());
            if let Some(node) = self.nodes.get(*new_id) {
                let node_name = node.get_name();
                if let Some(pid) = parent_id_opt {
                    if self.has_sibling_name_conflict(pid, node_name, Some(*new_id)) {
                        let resolved_name = self.resolve_name_conflict(pid, node_name);
                        renames.push((*new_id, resolved_name));
                    }
                }
            }
        }
        for (id, resolved_name) in renames {
            if let Some(node) = self.nodes.get_mut(id) {
                Self::set_node_name(node, resolved_name);
            }
        }
        for (_, new_id, _) in &insert_info {
            if let Some(node_ref) = self.nodes.get(*new_id) {
                if node_ref.is_renderable() {
                    self.needs_rerender.insert(*new_id);
                }
                if node_ref.needs_internal_fixed_update() {
                    self.nodes_with_internal_fixed_update.insert(*new_id);
                }
            }
        }

        for id in &inserted_ids {
            self.mark_transform_dirty_recursive(*id);
        }

        // Register collision shapes
        self.register_collision_shapes(&inserted_ids);

        // Load FUR files for UI nodes
        // Optimize: use as_ref() instead of clone() for Option<String>
        for id in &inserted_ids {
            if let Some(node) = self.nodes.get_mut(*id) {
                if let SceneNode::UINode(ui_node) = node {
                    if let Some(fur_path) = ui_node.fur_path.as_ref() {
                        if let Ok(fur_elements) = self.provider.load_fur_data(fur_path) {
                            build_ui_elements_from_fur(ui_node, &fur_elements);
                            // Mark UINode as needing rerender after elements are created
                            if ui_node.is_renderable() {
                                self.needs_rerender.insert(*id);
                            }
                        }
                    }
                }
            }
        }

        // Initialize scripts
        let script_targets: Vec<(NodeID, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.nodes
                    .get(*id)
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
                        let boxed = self.instantiate_script(ctor, id);
                        let handle = Rc::new(UnsafeCell::new(boxed));

                        // Check flags and add to appropriate vectors
                        let flags = unsafe { (*handle.get()).script_flags() };

                        if flags.has_update() && !self.scripts_with_update.contains(&id) {
                            self.scripts_with_update.push(id);
                        }
                        if flags.has_fixed_update() && !self.scripts_with_fixed_update.contains(&id)
                        {
                            self.scripts_with_fixed_update.push(id);
                        }

                        self.scripts.insert(id, handle);
                        self.scripts_dirty = true;

                        let mut api = ScriptApi::new(dt, self, &mut *project_borrow, gfx);
                        api.apply_exposed_vars_from_node(id);
                        api.call_init(id);

                        // After script initialization, ensure renderable nodes are marked for rerender
                        // (old system would have called mark_dirty() here)
                        if let Some(node) = self.nodes.get(id) {
                            if node.is_renderable() {
                                self.needs_rerender.insert(id);
                            }
                        }
                    }
                }
            }
        }

        // ───────────────────────────────────────────────
        // HANDLE NESTED is_root_of SCENE REFERENCES (RECURSIVE)
        // ───────────────────────────────────────────────
        let mut nodes_with_nested_scenes: Vec<(NodeID, String)> = Vec::new();

        for id in &inserted_ids {
            if let Some(node) = self.nodes.get(*id) {
                if let Some(scene_path) = Self::get_is_root_of(node) {
                    nodes_with_nested_scenes.push((*id, scene_path));
                }
            }
        }

        for (parent_node_id, scene_path) in nodes_with_nested_scenes {
            if let Ok(nested_scene_data) = self.provider.load_scene_data(&scene_path) {
                if let Err(e) = self.merge_scene_data_with_root_replacement(
                    nested_scene_data,
                    parent_node_id,
                    gfx,
                ) {
                    eprintln!("⚠️ Error merging nested scene '{}': {}", scene_path, e);
                }
            } else {
                eprintln!("⚠️ Failed to load nested scene: {}", scene_path);
            }
        }

        Ok(())
    }

    pub fn print_scene_tree(&self) {
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📊 SCENE TREE DEBUG:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Find root nodes (nodes with no parent)
        let root_nodes: Vec<NodeID> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.get_parent().is_none())
            .map(|(id, _)| id)
            .collect();

        if root_nodes.is_empty() {
            println!("⚠️  No root nodes found!");
            return;
        }

        for (i, root_id) in root_nodes.iter().enumerate() {
            let is_last = i == root_nodes.len() - 1;
            self.print_node_recursive(*root_id, 0, is_last);
        }

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }

    #[allow(dead_code)]
    fn print_node_recursive(&self, node_id: NodeID, depth: usize, is_last: bool) {
        // Build the tree characters (needed in both branches)
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

        if let Some(node) = self.nodes.get(node_id) {
            // Get node info
            let name = node.get_name();
            let node_type = node.get_type();
            let script_path = node.get_script_path();
            let parent_info = node
                .get_parent()
                .map(|p| format!("parent={} ({:?})", p.id, p.node_type))
                .unwrap_or_else(|| "ROOT".to_string());
            let has_script = self.scripts.contains_key(&node_id);
            let script_status = if has_script {
                "✓SCRIPT"
            } else {
                "✗NO_SCRIPT"
            };

            // Print this node with detailed debug info
            println!(
                "{}{} [id={}] [type={:?}] [{}] [{}] {}",
                prefix,
                name,
                node_id,
                node_type,
                parent_info,
                script_status,
                script_path
                    .map(|p| format!("script={}", p))
                    .unwrap_or_default(),
            );

            // Print children recursively
            let children = node.get_children();
            let child_count = children.len();

            for (i, child_id) in children.iter().enumerate() {
                let is_last_child = i == child_count - 1;
                self.print_node_recursive(*child_id, depth + 1, is_last_child);
            }
        } else {
            println!("{}⚠️  Node {} not found in scene!", prefix, node_id);
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

    /// Check if a node name conflicts with any *other* sibling (same parent).
    /// Pass exclude_node_id when checking the node we might rename so we don't count it as a conflict.
    fn has_sibling_name_conflict(
        &self,
        parent_id: NodeID,
        name: &str,
        exclude_node_id: Option<NodeID>,
    ) -> bool {
        self.nodes.iter().any(|(id, n)| {
            exclude_node_id != Some(id)
                && n.get_parent().map(|p| p.id) == Some(parent_id)
                && n.get_name() == name
        })
    }

    /// Check if a node name conflicts with its parent or any ancestor
    fn has_parent_or_ancestor_name_conflict(&self, parent_id: Option<NodeID>, name: &str) -> bool {
        let mut current_id = parent_id;

        // Walk up the tree checking each ancestor
        while let Some(id) = current_id {
            if let Some(ancestor) = self.nodes.get(id) {
                if ancestor.get_name() == name {
                    return true;
                }

                // Move to parent
                current_id = ancestor.get_parent().map(|p| p.id);
                if current_id == Some(id) {
                    // Reached root (parent points to itself)
                    break;
                }
            } else {
                // Parent doesn't exist in scene yet
                break;
            }
        }

        false
    }

    /// Resolve name conflicts by appending a digit suffix
    /// Checks for conflicts among siblings AND with parent/ancestors
    fn resolve_name_conflict(&self, parent_id: NodeID, base_name: &str) -> String {
        let mut counter = 1;
        let mut candidate = format!("{}{}", base_name, counter);

        // Check if candidate conflicts with siblings OR parent/ancestors
        while self.has_sibling_name_conflict(parent_id, &candidate, None)
            || self.has_parent_or_ancestor_name_conflict(Some(parent_id), &candidate)
        {
            counter += 1;
            candidate = format!("{}{}", base_name, counter);
        }

        candidate
    }

    fn ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.provider.load_ctor(short)
    }

    pub fn update(&mut self, gfx: &mut crate::rendering::Graphics, now: Instant) {
        #[cfg(feature = "profiling")]
        let _span = tracing::span!(tracing::Level::INFO, "Scene::update").entered();

        // OPTIMIZED: Accept now as parameter to avoid duplicate Instant::now() call
        let true_delta = match self.last_scene_update {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0, // first update
        };
        self.last_scene_update = Some(now);

        // store this dt somewhere for global stats
        self.delta_accum += true_delta;
        self.true_updates += 1;

        if self.delta_accum >= 3.0 {
            self.delta_accum = 0.0;
            self.true_updates = 0;
        }

        // Automatically poll Joy-Con 1 devices if polling is enabled
        // (Joy-Con 2 is polled automatically via async task)
        // OPTIMIZED: Use try_lock() to avoid blocking (very rare case)
        if let Some(mgr) = self.get_controller_manager() {
            if let Ok(mgr) = mgr.try_lock() {
                if mgr.is_polling_enabled() {
                    mgr.poll_joycon1_sync();
                }
            }
        }

        // Fixed update logic - runs at XPS rate from project manifest
        // OPTIMIZED: Cache xps to avoid RefCell borrow every frame (xps rarely changes)
        // For now, we still borrow but could cache if xps becomes a field
        let xps = {
            let project_ref = self.project.borrow();
            project_ref.xps()
        };
        // OPTIMIZED: Pre-calculate fixed_delta (division is expensive)
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
                    let _span = tracing::span!(tracing::Level::INFO, "update_collider_transforms")
                        .entered();
                    self.update_collider_transforms();
                }

                // Step physics simulation
                // OPTIMIZED: Skip physics step if world doesn't exist or is empty
                {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "physics_step").entered();
                    if let Some(physics) = &mut self.physics_2d {
                        let mut physics = physics.borrow_mut();
                        // OPTIMIZED: Skip step if no colliders exist (saves CPU cycles)
                        if !physics.colliders.is_empty() {
                            physics.step(fixed_delta);
                        }
                    }
                }

                // Run fixed update for all scripts
                {
                    #[cfg(feature = "profiling")]
                    let _span =
                        tracing::span!(tracing::Level::INFO, "script_fixed_updates").entered();

                    // OPTIMIZED: Rebuild cached_script_ids when dirty (update/fixed_update vectors are maintained incrementally)
                    if self.scripts_dirty {
                        self.cached_script_ids.clear();
                        self.cached_script_ids.extend(self.scripts.keys().copied());
                        self.scripts_dirty = false;
                    }

                    // OPTIMIZED: Use pre-filtered vector of scripts with fixed_update (preallocate)
                    let mut script_ids = Vec::with_capacity(self.scripts_with_fixed_update.len());
                    script_ids.extend(self.scripts_with_fixed_update.iter().copied());

                    // Clone project reference before loop to avoid borrow conflicts
                    let project_ref = self.project.clone();
                    for id in script_ids {
                        #[cfg(feature = "profiling")]
                        let _span =
                            tracing::span!(tracing::Level::INFO, "script_fixed_update", id = %id)
                                .entered();

                        // Rc::clone() is cheap (just increments ref count), but we need it per call
                        // because ScriptApi::new takes &mut self
                        let mut project_borrow = project_ref.borrow_mut();
                        let mut api = ScriptApi::new(fixed_delta, self, &mut *project_borrow, gfx);
                        api.call_fixed_update(id);
                    }
                }

                // Run internal fixed update for nodes that need it
                {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "node_internal_fixed_updates")
                        .entered();

                    // Optimize: collect first to avoid borrow checker issues (preallocate)
                    let mut node_ids =
                        Vec::with_capacity(self.nodes_with_internal_fixed_update.len());
                    node_ids.extend(self.nodes_with_internal_fixed_update.iter().copied());
                    // OPTIMIZED: Clone project once before loop instead of per node
                    let project_ref = self.project.clone();
                    for node_id in node_ids {
                        let mut project_borrow = project_ref.borrow_mut();
                        let mut api = ScriptApi::new(fixed_delta, self, &mut *project_borrow, gfx);
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
            // OPTIMIZED: Only run script updates when there are scripts (do not return - we still need the render pass below)
            if !self.scripts_with_update.is_empty() {
                // OPTIMIZED: Use pre-filtered vector of scripts with update (preallocate)
                let mut script_ids = Vec::with_capacity(self.scripts_with_update.len());
                script_ids.extend(self.scripts_with_update.iter().copied());

                // Clone project reference before loop to avoid borrow conflicts
                let project_ref = self.project.clone();

                #[cfg(feature = "profiling")]
                let _span = tracing::span!(
                    tracing::Level::INFO,
                    "script_updates",
                    count = script_ids.len()
                )
                .entered();

                for id in script_ids {
                    #[cfg(feature = "profiling")]
                    let _span =
                        tracing::span!(tracing::Level::INFO, "script_update", id = %id).entered();

                    // OPTIMIZED: Borrow project once per script (RefCell borrow_mut is fast but still has overhead)
                    // Note: We can't borrow project once for all scripts because ScriptApi needs &mut self (scene)
                    let mut project_borrow = project_ref.borrow_mut();
                    let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow, gfx);
                    api.call_update(id);
                }
            }
        }

        // Global transforms are now calculated lazily when needed (in traverse_and_render)

        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "process_queued_calls").entered();
            self.process_queued_calls(gfx, true_delta);
        }
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "process_queued_signals").entered();
            self.process_queued_signals(gfx, true_delta);
        }

        // RENDERING - unified with update (no separate draw() calls)
        // Convert texture_path → texture_id on first render (when Graphics is available)
        if !self.textures_converted {
            self.convert_texture_paths_to_ids(gfx);
            self.textures_converted = true;
        }

        // Run internal render update for nodes that need it (e.g., UI interactions)
        {
            #[cfg(feature = "profiling")]
            let _span =
                tracing::span!(tracing::Level::INFO, "node_internal_render_updates").entered();

            // Optimize: collect first to avoid borrow checker issues (HashSet iteration order is non-deterministic but that's fine)
            let node_ids: Vec<NodeID> = self
                .nodes_with_internal_render_update
                .iter()
                .copied()
                .collect();

            if !node_ids.is_empty() {
                // Clone project reference before loop to avoid borrow conflicts
                let project_ref = self.project.clone();

                for node_id in node_ids {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "node_internal_render_update", id = %node_id).entered();

                    // OPTIMIZED: Borrow project once per node
                    let mut project_borrow = project_ref.borrow_mut();
                    let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow, gfx);
                    api.call_node_internal_render_update(node_id);
                }
            }
        }

        let nodes_needing_rerender = {
            #[cfg(feature = "profiling")]
            let _span =
                tracing::span!(tracing::Level::INFO, "get_nodes_needing_rerender").entered();
            self.get_nodes_needing_rerender()
        };
        if !nodes_needing_rerender.is_empty() {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(
                tracing::Level::INFO,
                "traverse_and_render",
                count = nodes_needing_rerender.len()
            )
            .entered();
            self.traverse_and_render(nodes_needing_rerender, gfx);
        }
    }

    // ---------- Signals ----------
    // Scene stores: connections (signal_id → target → function_ids) and deferred queue.
    // Instant emit: api.emit_signal_id gets connections via get_signal_connections and calls
    // call_function_id for each handler (no scene.emit_signal_id). Deferred: api calls
    // scene.emit_signal_id_deferred to queue; at end of frame process_queued_signals runs
    // handlers via ScriptApi::emit_signal_id (same path as instant).

    fn connect_signal(&mut self, signal: SignalID, target_id: NodeID, function_id: u64) {
        // Top-level map: signal_id → inner map (script → list of fn ids)
        let script_map = self.signals.connections.entry(signal).or_default();

        // Inner: target script → function list
        let funcs = script_map.entry(target_id).or_default();

        // Avoid duplicate function connections
        if !funcs.iter().any(|&id| id == function_id) {
            funcs.push(function_id);
        }
    }

    /// Emit signal deferred - queue for processing at end of frame
    fn emit_signal_id_deferred(&mut self, signal: SignalID, params: &[Value]) {
        // Convert slice to SmallVec for stack-allocated storage (≤3 params = no heap allocation)
        let mut smallvec = SmallVec::new();
        smallvec.extend(params.iter().cloned());
        self.queued_signals.push((signal, smallvec));
    }

    /// Queue a script function call for processing at end of frame.
    fn call_function_id_deferred(&mut self, node_id: NodeID, function_id: u64, params: &[Value]) {
        let mut smallvec = SmallVec::new();
        smallvec.extend(params.iter().cloned());
        self.queued_calls.push((node_id, function_id, smallvec));
    }

    /// Process deferred calls at end of frame (before deferred signals).
    fn process_queued_calls(&mut self, gfx: &mut crate::rendering::Graphics, delta: f32) {
        if self.queued_calls.is_empty() {
            return;
        }
        let project_ref = self.project.clone();
        let mut queued = Vec::with_capacity(self.queued_calls.len());
        queued.extend(self.queued_calls.drain(..));
        for (node_id, func_id, params) in queued {
            let mut project_borrow = project_ref.borrow_mut();
            let mut api = ScriptApi::new(delta, self, &mut *project_borrow, gfx);
            let _ = api.call_function_id(node_id, func_id, &params);
        }
    }

    /// Process deferred signals at end of frame. Scene only stores the queue; handler
    /// invocation goes through ScriptApi (same path as instant emit).
    fn process_queued_signals(&mut self, gfx: &mut crate::rendering::Graphics, delta: f32) {
        if self.queued_signals.is_empty() {
            return;
        }
        let project_ref = self.project.clone();
        let mut queued = Vec::with_capacity(self.queued_signals.len());
        queued.extend(self.queued_signals.drain(..));
        for (signal, params) in queued {
            let mut project_borrow = project_ref.borrow_mut();
            let mut api = ScriptApi::new(delta, self, &mut *project_borrow, gfx);
            api.emit_signal_id(signal, &params);
        }
    }

    pub fn add_node_to_scene(
        &mut self,
        node: SceneNode,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<NodeID> {
        // Always use arena insert: allocates lowest open slot+generation and returns that id.
        let id = self.nodes.insert(node);
        let node = self.nodes.get_mut(id).expect("node just inserted");

        // Handle UI nodes with .fur files (re-borrow node after insert)
        if let SceneNode::UINode(ui_node) = node {
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
        // Mark transform as dirty for Node2D nodes (after insertion)
        self.mark_transform_dirty_recursive(id);
        // Add to needs_rerender set since this is a newly created node
        // (mark_transform_dirty_recursive will add it if not already in set, but we know it's new)
        // Also ensure UINode is marked if FUR file was just loaded (elements are now ready to render)
        if let Some(node) = self.nodes.get(id) {
            if node.is_renderable() {
                self.needs_rerender.insert(id);
            }
        }

        // Register node for internal fixed updates if needed
        if let Some(node_ref) = self.nodes.get(id) {
            if node_ref.needs_internal_fixed_update() {
                // Optimize: HashSet insert is O(1) and handles duplicates automatically
                self.nodes_with_internal_fixed_update.insert(id);
            }
        }

        // node is moved already, so get it back immutably from scene
        let script_path_opt = self
            .nodes
            .get(id)
            .and_then(|node_ref| node_ref.get_script_path().map(|s| s.to_string()));

        if let Some(script_path) = script_path_opt {
            println!("   ✅ Found script_path: {}", script_path);

            let identifier = script_path_to_identifier(&script_path)
                .map_err(|e| anyhow::anyhow!("Invalid script path {}: {}", script_path, e))?;
            let ctor = self.ctor(&identifier)?;

            // Create the script
            let boxed = self.instantiate_script(ctor, id);
            let handle = Rc::new(UnsafeCell::new(boxed));

            // Check flags and add to appropriate vectors
            let flags = unsafe { (*handle.get()).script_flags() };

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

            let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow, gfx);
            api.apply_exposed_vars_from_node(id);
            api.call_init(id);

            // After script initialization, ensure renderable nodes are marked for rerender
            // (old system would have called mark_dirty() here)
            if let Some(node) = self.nodes.get(id) {
                if node.is_renderable() {
                    self.needs_rerender.insert(id);
                }
            }
        }

        Ok(id)
    }

    pub fn get_root(&self) -> &SceneNode {
        self.nodes.get(self.root_id).expect("Root node not found")
    }

    /// Get reference to scripts that have draw() implemented
    /// Used by the rendering loop to call draw on frame-synchronized scripts

    // Remove node and stop rendering
    pub fn remove_node(&mut self, node_id: NodeID, gfx: &mut Graphics) {
        // Check if node exists before trying to delete (might have been deleted already)
        if !self.nodes.contains_key(node_id) {
            return; // Node already deleted, nothing to do
        }

        // If this is a CollisionShape2D, unregister it from physics BEFORE deleting
        // This prevents the physics system from trying to access the deleted node
        if let Some(node) = self.nodes.get(node_id) {
            if let SceneNode::CollisionShape2D(collision_shape) = node {
                // Get parent info before we delete the node
                let parent_id_opt = collision_shape.get_parent().map(|p| p.id);
                let is_area2d_parent = parent_id_opt
                    .and_then(|pid| self.nodes.get(pid))
                    .map(|p| matches!(p, SceneNode::Area2D(_)))
                    .unwrap_or(false);

                // Unregister from physics
                if let Some(physics) = &mut self.physics_2d {
                    let mut physics = physics.borrow_mut();

                    // Remove from area's collider list if it's a child of an Area2D
                    if is_area2d_parent {
                        if let Some(parent_id) = parent_id_opt {
                            if let Some(colliders) = physics.area_to_colliders.get_mut(&parent_id) {
                                if let Some(collider_handle) = collision_shape.collider_handle {
                                    colliders.retain(|&h| h != collider_handle);
                                }
                            }
                        }
                    }

                    // Remove the collider from physics world
                    physics.remove_collider(node_id);
                }
            }
        }

        // Stop rendering this node and all its children
        self.stop_rendering_recursive(node_id, gfx);

        // Remove from scene
        self.nodes.remove(node_id);

        // Remove scripts - actually delete them from scene
        // Always clean up script vectors even if script doesn't exist (defensive cleanup)
        let had_script = self.scripts.remove(&node_id).is_some();

        // Remove from update/fixed_update/draw vectors (always clean up, even if script wasn't in HashMap)
        // retain() is idempotent - safe to call even if node_id isn't in the vector
        // Check if any cleanup is needed to avoid unnecessary work
        let needs_cleanup = had_script
            || self.scripts_with_update.contains(&node_id)
            || self.scripts_with_fixed_update.contains(&node_id);

        if needs_cleanup {
            self.scripts_with_update
                .retain(|&script_id| script_id != node_id);
            self.scripts_with_fixed_update
                .retain(|&script_id| script_id != node_id);
            self.scripts_dirty = true;
        }

        // Clean up signal connections - remove this node from all signal connection maps
        // This prevents deferred signals or later emissions from trying to call handlers on a deleted node
        for (_signal_id, script_map) in self.signals.connections.iter_mut() {
            script_map.remove(&node_id);
        }

        // Also clean up empty signal entries to avoid memory leaks
        self.signals
            .connections
            .retain(|_, script_map| !script_map.is_empty());

        // Remove from needs_rerender set (if it was there)
        self.needs_rerender.remove(&node_id);

        // IMPORTANT: Clean up Area2D's previous_collisions tracking when a node is removed
        // This prevents Area2D from trying to emit signals for nodes that no longer exist
        // Iterate through all Area2D nodes and remove the deleted node from their previous_collisions
        for (_area_id, area_node) in self.nodes.iter_mut() {
            if let SceneNode::Area2D(area) = area_node {
                if let Some(ref mut set) = area.previous_collisions {
                    set.remove(&node_id);
                }
            }
        }
    }

    /// Get the global transform for a node, calculating it lazily if dirty
    /// This recursively traverses up the parent chain until it finds a clean transform
    pub fn get_global_transform(
        &mut self,
        node_id: NodeID,
    ) -> Option<crate::structs2d::Transform2D> {
        // OPTIMIZED: Reduced hashmap lookups by collecting all needed data in single pass
        // Build chain from node to root, then calculate transforms top-down

        // First check if already cached (single lookup)
        if let Some(node) = self.nodes.get(node_id) {
            if let Some(node2d) = node.as_node2d() {
                if !node2d.transform_dirty {
                    return Some(node2d.global_transform);
                }
            } else {
                return None; // Not a Node2D node
            }
        } else {
            return None;
        }

        // Step 1: Build the chain from this node up to root (or first cached ancestor)
        // OPTIMIZED: Collect local transforms and parent info in single pass to reduce lookups
        let mut chain: Vec<(NodeID, crate::structs2d::Transform2D)> = Vec::new();
        let mut current_id = Some(node_id);
        let mut cached_ancestor_id = None;
        let mut cached_ancestor_transform = crate::structs2d::Transform2D::default();

        while let Some(id) = current_id {
            if let Some(node) = self.nodes.get(id) {
                if let Some(node2d) = node.as_node2d() {
                    // Check if already cached (not dirty)
                    if !node2d.transform_dirty {
                        // Found a cached ancestor - we can use it and stop
                        cached_ancestor_id = Some(id);
                        cached_ancestor_transform = node2d.global_transform;
                        break;
                    }
                    // Collect local transform now to avoid second lookup later
                    if let Some(local) = node.get_node2d_transform() {
                        chain.push((id, local));
                    } else {
                        break;
                    }
                    current_id = node.get_parent().map(|p| p.id);
                } else {
                    // Not a Node2D node - stop here, use identity
                    break;
                }
            } else {
                break;
            }
        }

        if chain.is_empty() {
            return None;
        }

        // Step 2: Start with cached ancestor transform or identity
        let mut parent_global = if cached_ancestor_id.is_some() {
            cached_ancestor_transform
        } else {
            crate::structs2d::Transform2D::default()
        };

        // Step 3: Process chain from root to target node (chain is built root->target, so reverse it)
        // OPTIMIZED: Single mutable lookup per node instead of get + get_mut
        for &(id, local) in chain.iter().rev() {
            // Calculate global transform
            let global = crate::structs2d::Transform2D::calculate_global(&parent_global, &local);

            // Cache the result (single mutable lookup)
            if let Some(node) = self.nodes.get_mut(id) {
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.global_transform = global;
                    node2d.transform_dirty = false;
                }
            }

            parent_global = global;
        }

        Some(parent_global)
    }

    /// OPTIONAL: Batch-optimized version for precalculate_transforms_in_dependency_order
    /// Use this when calculating many siblings with the same parent
    /// ~20% faster than calling get_global_transform() in a loop
    /// OPTIMIZED: Reduced hashmap lookups by collecting all data in single pass
    fn precalculate_transforms_batch(&mut self, parent_id: NodeID, child_ids: &[NodeID]) {
        // OPTIMIZED: Fast path for empty batches
        if child_ids.is_empty() {
            return;
        }

        // Get parent's global transform once
        let parent_global = self
            .get_global_transform(parent_id)
            .unwrap_or_else(|| crate::structs2d::Transform2D::default());

        // OPTIMIZED: Fast path for identity parent (common case - static nodes)
        if parent_global.is_default() {
            // Just copy local transforms directly, no calculation needed
            // OPTIMIZED: Get local transform first (immutable borrow), then update (mutable borrow)
            for &child_id in child_ids {
                // Get local transform first (immutable borrow)
                let local = self
                    .nodes
                    .get(child_id)
                    .and_then(|node| node.get_node2d_transform());

                // Then update (mutable borrow)
                if let Some(local) = local {
                    if let Some(node) = self.nodes.get_mut(child_id) {
                        if let Some(node2d) = node.as_node2d_mut() {
                            node2d.global_transform = local;
                            node2d.transform_dirty = false;
                        }
                    }
                }
            }
            return;
        }

        // OPTIMIZED: Collect local transforms and child IDs in single pass
        // Pre-allocate with exact capacity to avoid reallocations
        let mut local_transforms = Vec::with_capacity(child_ids.len());
        let mut valid_child_ids = Vec::with_capacity(child_ids.len());

        // Single pass: collect all needed data
        for &child_id in child_ids {
            if let Some(node) = self.nodes.get(child_id) {
                // OPTIMIZED: Skip if not dirty (already calculated) - check before collecting
                if let Some(node2d) = node.as_node2d() {
                    if !node2d.transform_dirty {
                        continue; // Skip already-clean nodes
                    }
                }

                if let Some(local) = node.get_node2d_transform() {
                    local_transforms.push(local);
                    valid_child_ids.push(child_id);
                }
            }
        }

        // OPTIMIZED: Early return if no dirty nodes
        if local_transforms.is_empty() {
            return;
        }

        // Batch calculate (reuses parent matrix conversion)
        let globals = crate::structs2d::Transform2D::batch_calculate_global(
            &parent_global,
            &local_transforms,
        );

        // Cache results - single mutable lookup per node
        // OPTIMIZED: Use iterators for better performance
        for (child_id, global) in valid_child_ids.iter().zip(globals.iter()) {
            if let Some(node) = self.nodes.get_mut(*child_id) {
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.global_transform = *global;
                    node2d.transform_dirty = false;
                }
            }
        }
    }

    /// Set the global transform for a Node2D.
    /// Converts the desired world-space transform into local space (relative to parent) and sets
    /// `transform` so that the normal propagation (parent_global * local → global) yields this value.
    pub fn set_global_transform(
        &mut self,
        node_id: NodeID,
        desired_global: crate::structs2d::Transform2D,
    ) -> Option<()> {
        let parent_global = self
            .nodes
            .get(node_id)
            .and_then(|n| n.get_parent())
            .map(|p| self.get_global_transform(p.id))
            .flatten()
            .unwrap_or_default();

        let local = parent_global.inverse().multiply(&desired_global);

        let ok = if let Some(node) = self.nodes.get_mut(node_id) {
            if let Some(node2d) = node.as_node2d_mut() {
                node2d.transform = local;
                node2d.global_transform = desired_global;
                node2d.transform_dirty = false;
                true
            } else {
                false
            }
        } else {
            false
        };
        if ok {
            self.mark_children_transform_dirty_recursive(node_id);
        }
        ok.then_some(())
    }

    /// Get the global transform for a Node3D, calculating it lazily if dirty
    pub fn get_global_transform_3d(
        &mut self,
        node_id: NodeID,
    ) -> Option<crate::structs3d::Transform3D> {
        if let Some(node) = self.nodes.get(node_id) {
            if let Some(node3d) = node.as_node3d() {
                if !node3d.transform_dirty {
                    return Some(node3d.global_transform);
                }
            } else {
                return None;
            }
        } else {
            return None;
        }

        let mut chain: Vec<(NodeID, crate::structs3d::Transform3D)> = Vec::new();
        let mut current_id = Some(node_id);
        let mut cached_ancestor_id = None;
        let mut cached_ancestor_transform = crate::structs3d::Transform3D::default();

        while let Some(id) = current_id {
            if let Some(node) = self.nodes.get(id) {
                if let Some(node3d) = node.as_node3d() {
                    if !node3d.transform_dirty {
                        cached_ancestor_id = Some(id);
                        cached_ancestor_transform = node3d.global_transform;
                        break;
                    }
                    if let Some(local) = node.get_node3d_transform() {
                        chain.push((id, local));
                    } else {
                        break;
                    }
                    current_id = node.get_parent().map(|p| p.id);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if chain.is_empty() {
            return None;
        }

        let mut parent_global = if cached_ancestor_id.is_some() {
            cached_ancestor_transform
        } else {
            crate::structs3d::Transform3D::default()
        };

        for &(id, local) in chain.iter().rev() {
            let global = crate::structs3d::Transform3D::calculate_global(&parent_global, &local);

            if let Some(node) = self.nodes.get_mut(id) {
                if let Some(node3d) = node.as_node3d_mut() {
                    node3d.global_transform = global;
                    node3d.transform_dirty = false;
                }
            }

            parent_global = global;
        }

        Some(parent_global)
    }

    /// Set the global transform for a Node3D.
    /// Converts the desired world-space transform into local space (relative to parent) and sets
    /// `transform` so that the normal propagation (parent_global * local → global) yields this value.
    pub fn set_global_transform_3d(
        &mut self,
        node_id: NodeID,
        desired_global: crate::structs3d::Transform3D,
    ) -> Option<()> {
        let parent_global = self
            .nodes
            .get(node_id)
            .and_then(|n| n.get_parent())
            .map(|p| self.get_global_transform_3d(p.id))
            .flatten()
            .unwrap_or_default();

        let local = parent_global.inverse().multiply(&desired_global);

        let ok = if let Some(node) = self.nodes.get_mut(node_id) {
            if let Some(node3d) = node.as_node3d_mut() {
                node3d.transform = local;
                node3d.global_transform = desired_global;
                node3d.transform_dirty = false;
                true
            } else {
                false
            }
        } else {
            false
        };
        if ok {
            self.mark_children_transform_dirty_recursive(node_id);
        }
        ok.then_some(())
    }

    /// Mark a node's transform as dirty (and all its children)
    /// Also marks nodes as needing rerender so they get picked up by get_nodes_needing_rerender()
    /// OPTIMIZED: Uses iterative work queue instead of recursion for better performance and cache locality
    pub fn mark_transform_dirty_recursive(&mut self, node_id: NodeID) {
        // Check if node exists before processing (might have been deleted)
        if !self.nodes.contains_key(node_id) {
            eprintln!(
                "⚠️ mark_transform_dirty_recursive: Node {} does not exist, skipping",
                node_id
            );
            return;
        }

        // OPTIMIZED: Use iterative work queue instead of recursion
        // This provides better cache locality and avoids stack overhead
        let mut work_queue = Vec::new();
        work_queue.push(node_id);

        while let Some(current_id) = work_queue.pop() {
            // Skip if node was deleted during processing
            if !self.nodes.contains_key(current_id) {
                continue;
            }

            // Step 1: Get Node2D children list (immutable borrow)
            let (node2d_child_ids, needs_2d_cache_update): (Vec<NodeID>, bool) = {
                // Check if we have a cached list of Node2D children
                let cached = self
                    .nodes
                    .get(current_id)
                    .and_then(|node| node.as_node2d())
                    .and_then(|n2d| n2d.node2d_children_cache.as_ref());

                if let Some(cached_ids) = cached {
                    // Use cache - but filter out any stale references to deleted nodes
                    let filtered: Vec<NodeID> = cached_ids
                        .iter()
                        .copied()
                        .filter(|&child_id| {
                            // Verify child exists and is still a child of this parent
                            if let Some(child_node) = self.nodes.get(child_id) {
                                // Verify it's still a child (parent might have changed)
                                if let Some(parent) = child_node.get_parent() {
                                    parent.id == current_id && child_node.as_node2d().is_some()
                                } else {
                                    false // Child has no parent, so it's not a child of this node
                                }
                            } else {
                                false // Child doesn't exist
                            }
                        })
                        .collect();

                    // If we filtered out any entries, we need to update the cache
                    let needs_update = filtered.len() != cached_ids.len();
                    (filtered, needs_update)
                } else {
                    // Cache miss - build cache and use it
                    let child_ids: Vec<NodeID> = self
                        .nodes
                        .get(current_id)
                        .map(|node| node.get_children().iter().copied().collect())
                        .unwrap_or_default();

                    // Filter to only Node2D children and cache the result
                    let node2d_ids: Vec<NodeID> = child_ids
                        .iter()
                        .copied()
                        .filter_map(|child_id| {
                            if let Some(child_node) = self.nodes.get(child_id) {
                                if child_node.as_node2d().is_some() {
                                    Some(child_id)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect();

                    // Update cache for future use
                    if let Some(node) = self.nodes.get_mut(current_id) {
                        if let Some(node2d) = node.as_node2d_mut() {
                            node2d.node2d_children_cache = Some(node2d_ids.clone());
                        }
                    }

                    (node2d_ids, false) // Cache was just built, no update needed
                }
            };

            // Step 1b: Get Node3D children list (immutable borrow)
            let (node3d_child_ids, needs_3d_cache_update): (Vec<NodeID>, bool) = {
                let cached = self
                    .nodes
                    .get(current_id)
                    .and_then(|node| node.as_node3d())
                    .and_then(|n3d| n3d.node3d_children_cache.as_ref());

                if let Some(cached_ids) = cached {
                    let filtered: Vec<NodeID> = cached_ids
                        .iter()
                        .copied()
                        .filter(|&child_id| {
                            if let Some(child_node) = self.nodes.get(child_id) {
                                if let Some(parent) = child_node.get_parent() {
                                    parent.id == current_id && child_node.as_node3d().is_some()
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        })
                        .collect();

                    let needs_update = filtered.len() != cached_ids.len();
                    (filtered, needs_update)
                } else {
                    let child_ids: Vec<NodeID> = self
                        .nodes
                        .get(current_id)
                        .map(|node| node.get_children().iter().copied().collect())
                        .unwrap_or_default();

                    let node3d_ids: Vec<NodeID> = child_ids
                        .iter()
                        .copied()
                        .filter_map(|child_id| {
                            if let Some(child_node) = self.nodes.get(child_id) {
                                if child_node.as_node3d().is_some() {
                                    Some(child_id)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .collect();

                    if let Some(node) = self.nodes.get_mut(current_id) {
                        if let Some(node3d) = node.as_node3d_mut() {
                            node3d.node3d_children_cache = Some(node3d_ids.clone());
                        }
                    }

                    (node3d_ids, false)
                }
            };

            // Step 2: Mark this node as dirty and update caches if needed (mutable borrow)
            let is_renderable = {
                let node = self.nodes.get_mut(current_id).unwrap();

                // Mark transform as dirty if it's a Node2D-based node
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.transform_dirty = true;
                    if needs_2d_cache_update {
                        node2d.node2d_children_cache = Some(node2d_child_ids.clone());
                    }
                }

                // Mark transform as dirty if it's a Node3D-based node
                if let Some(node3d) = node.as_node3d_mut() {
                    node3d.transform_dirty = true;
                    if needs_3d_cache_update {
                        node3d.node3d_children_cache = Some(node3d_child_ids.clone());
                    }
                }

                // Check if renderable (before dropping mutable borrow)
                node.is_renderable()
            };

            // Step 3: Add to needs_rerender if renderable
            if is_renderable && !self.needs_rerender.contains(&current_id) {
                self.needs_rerender.insert(current_id);
            }

            // Step 4: Add Node2D and Node3D children to work queue (depth-first)
            for child_id in node2d_child_ids.into_iter().rev() {
                if self.nodes.contains_key(child_id) {
                    work_queue.push(child_id);
                }
            }
            for child_id in node3d_child_ids.into_iter().rev() {
                if self.nodes.contains_key(child_id) {
                    work_queue.push(child_id);
                }
            }
        }
    }

    /// Mark only the children (and their descendants) as transform dirty, not the node itself.
    /// Used after set_global_transform so the node we set stays clean and children get recalculated.
    fn mark_children_transform_dirty_recursive(&mut self, node_id: NodeID) {
        let child_ids: Vec<NodeID> = self
            .nodes
            .get(node_id)
            .map(|node| node.get_children().iter().copied().collect())
            .unwrap_or_default();
        for child_id in child_ids {
            if self.nodes.contains_key(child_id) {
                self.mark_transform_dirty_recursive(child_id);
            }
        }
    }

    /// Clear the Node2D children cache for a parent node (when all children are removed)
    /// This keeps the cache in sync so we don't need hashmap lookups every frame
    pub fn update_node2d_children_cache_on_clear(&mut self, parent_id: NodeID) {
        if let Some(node) = self.nodes.get_mut(parent_id) {
            if let Some(node2d) = node.as_node2d_mut() {
                // Clear the cache - set to empty vec so it's ready for new children
                node2d.node2d_children_cache = Some(Vec::new());
            }
        }
        // If parent doesn't exist, that's fine - node was probably deleted
    }

    /// Update the Node2D children cache for a parent node when a child is added
    /// This keeps the cache in sync so we don't need hashmap lookups every frame
    pub fn update_node2d_children_cache_on_add(&mut self, parent_id: NodeID, child_id: NodeID) {
        // Check if child is Node2D first (immutable borrow)
        let child_exists = self.nodes.contains_key(child_id);
        let is_node2d = if child_exists {
            self.nodes
                .get(child_id)
                .map(|child_node| child_node.as_node2d().is_some())
                .unwrap_or(false)
        } else {
            // Child doesn't exist yet - can't update cache
            return;
        };

        // Only update cache if child is Node2D
        if !is_node2d {
            return;
        }

        // Now update parent's cache (mutable borrow)
        if let Some(parent_node) = self.nodes.get_mut(parent_id) {
            if let Some(node2d) = parent_node.as_node2d_mut() {
                // Add to cache if it exists, otherwise invalidate (will rebuild on next use)
                if let Some(ref mut cache) = node2d.node2d_children_cache {
                    if !cache.contains(&child_id) {
                        cache.push(child_id);
                    }
                } else {
                    // Cache doesn't exist yet - invalidate so it rebuilds on next use
                    node2d.node2d_children_cache = None;
                }
            }
        }
    }

    /// Update the Node2D children cache for a parent node when a child is removed
    fn update_node2d_children_cache_on_remove(&mut self, parent_id: NodeID, child_id: NodeID) {
        if let Some(parent_node) = self.nodes.get_mut(parent_id) {
            if let Some(node2d) = parent_node.as_node2d_mut() {
                if let Some(ref mut cache) = node2d.node2d_children_cache {
                    cache.retain(|&id| id != child_id);
                }
            }
        }
    }

    /// Clear the Node3D children cache for a parent node (when all children are removed)
    pub fn update_node3d_children_cache_on_clear(&mut self, parent_id: NodeID) {
        if let Some(node) = self.nodes.get_mut(parent_id) {
            if let Some(node3d) = node.as_node3d_mut() {
                node3d.node3d_children_cache = Some(Vec::new());
            }
        }
    }

    /// Update the Node3D children cache for a parent node when a child is added
    pub fn update_node3d_children_cache_on_add(&mut self, parent_id: NodeID, child_id: NodeID) {
        let is_node3d = self
            .nodes
            .get(child_id)
            .map(|n| n.as_node3d().is_some())
            .unwrap_or(false);
        if !is_node3d {
            return;
        }
        if let Some(parent_node) = self.nodes.get_mut(parent_id) {
            if let Some(node3d) = parent_node.as_node3d_mut() {
                if let Some(ref mut cache) = node3d.node3d_children_cache {
                    if !cache.contains(&child_id) {
                        cache.push(child_id);
                    }
                } else {
                    node3d.node3d_children_cache = None;
                }
            }
        }
    }

    /// Update the Node3D children cache for a parent node when a child is removed
    fn update_node3d_children_cache_on_remove(&mut self, parent_id: NodeID, child_id: NodeID) {
        if let Some(parent_node) = self.nodes.get_mut(parent_id) {
            if let Some(node3d) = parent_node.as_node3d_mut() {
                if let Some(ref mut cache) = node3d.node3d_children_cache {
                    cache.retain(|&id| id != child_id);
                }
            }
        }
    }

    /// Update collider transforms to match node transforms
    fn update_collider_transforms(&mut self) {
        // OPTIMIZED: Parallelize node filtering (read-only operation)
        let node_ids: Vec<NodeID> = if self.nodes.len() >= 10 {
            // Use parallel iteration for larger node counts
            // Convert to Vec first since NodeArena doesn't implement IntoParallelIterator
            let nodes_vec: Vec<(NodeID, &SceneNode)> = self.nodes.iter().collect();
            nodes_vec
                .par_iter()
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
                .collect()
        } else {
            // Sequential for small counts (overhead not worth it)
            self.nodes
                .iter()
                .filter_map(|(node_id, node)| {
                    if let SceneNode::CollisionShape2D(cs) = node {
                        if cs.collider_handle.is_some() {
                            Some(node_id)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Mark all collision shapes as dirty to force recalculation
        // This ensures their global transforms are recalculated even if the dirty flag
        // wasn't set (e.g., if parent moved but child wasn't marked dirty)
        // Note: Can't parallelize this easily due to mutable borrow requirements
        for node_id in &node_ids {
            if let Some(node) = self.nodes.get_mut(*node_id) {
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.transform_dirty = true;
                }
            }
        }

        // Get global transforms (requires mutable access)
        // Now that we've marked them dirty, get_global_transform will recalculate
        // OPTIMIZED: Pre-allocate with known capacity
        let mut to_update: Vec<(NodeID, [f32; 2], f32)> = Vec::with_capacity(node_ids.len());
        for node_id in node_ids {
            if let Some(global) = self.get_global_transform(node_id) {
                let position = [global.position.x, global.position.y];
                let rotation = global.rotation;
                to_update.push((node_id, position, rotation));
            }
        }

        // Update physics colliders (after releasing all borrows)
        // OPTIMIZED: Only update if physics world exists
        // Filter out any nodes that were deleted during the update
        if let Some(physics) = &mut self.physics_2d {
            let mut physics = physics.borrow_mut();
            for (node_id, position, rotation) in to_update {
                // Only update if node still exists (might have been deleted)
                if self.nodes.contains_key(node_id) {
                    physics.update_collider_transform(node_id, position, rotation);
                }
            }
        }
    }

    /// Register CollisionShape2D nodes with the physics world
    fn register_collision_shapes(&mut self, node_ids: &[NodeID]) {
        // First, collect all the data we need (shape info, transforms, parent info)
        let mut to_register: Vec<(
            NodeID,
            crate::structs2d::Shape2D,
            Option<crate::nodes::node::ParentType>,
        )> = Vec::new();

        for &node_id in node_ids {
            if let Some(node) = self.nodes.get(node_id) {
                if let SceneNode::CollisionShape2D(collision_shape) = node {
                    // Only register if it has a shape defined
                    if let Some(shape) = collision_shape.shape {
                        let parent_opt = collision_shape.get_parent();
                        to_register.push((node_id, shape, parent_opt));
                    }
                }
            }
        }

        // Get global transforms for all nodes (requires mutable access)
        // OPTIMIZED: Use with_capacity(0) for known-empty map initially
        let mut global_transforms: HashMap<NodeID, ([f32; 2], f32)> = HashMap::with_capacity(0);
        for (node_id, _, _) in &to_register {
            if let Some(global) = self.get_global_transform(*node_id) {
                global_transforms.insert(
                    *node_id,
                    ([global.position.x, global.position.y], global.rotation),
                );
            }
        }

        // Now register with physics (after releasing all node borrows)
        // OPTIMIZED: Lazy initialization - create physics world when first collision shape is registered
        if to_register.is_empty() {
            return;
        }

        // First, check which parents are Area2D nodes (before borrowing physics)
        // Store tuples of (node_id, shape, parent_opt, is_area2d_parent)
        let mut registration_data: Vec<(NodeID, crate::structs2d::Shape2D, Option<NodeID>, bool)> =
            Vec::new();
        for (node_id, shape, parent_opt) in to_register {
            let is_area2d_parent = if let Some(parent) = &parent_opt {
                let pid = parent.id;
                if let Some(parent_node) = self.nodes.get(pid) {
                    matches!(parent_node, SceneNode::Area2D(_))
                } else {
                    false
                }
            } else {
                false
            };
            registration_data.push((node_id, shape, parent_opt.map(|p| p.id), is_area2d_parent));
        }

        // Now borrow physics and create all colliders
        let mut physics = self.get_or_init_physics_2d().borrow_mut();
        let mut handles_to_store: Vec<(NodeID, rapier2d::prelude::ColliderHandle, Option<NodeID>)> =
            Vec::new();

        for (node_id, shape, parent_id_opt, is_area2d_parent) in registration_data {
            // Use global transform if available, otherwise use default (for first frame)
            let (world_position, world_rotation) = global_transforms
                .get(&node_id)
                .copied()
                .unwrap_or(([0.0, 0.0], 0.0));

            // Create the sensor collider in physics world with world transform
            let collider_handle =
                physics.create_sensor_collider(node_id, shape, world_position, world_rotation);

            // If this collision shape is a child of an Area2D, register it
            if is_area2d_parent {
                if let Some(pid) = parent_id_opt {
                    physics.register_area_collider(pid, collider_handle);
                }
            }

            handles_to_store.push((node_id, collider_handle, parent_id_opt));
        }

        // Drop physics borrow before mutating nodes
        drop(physics);

        // Store handles in collision shapes
        for (node_id, collider_handle, _) in handles_to_store {
            if let Some(node) = self.nodes.get_mut(node_id) {
                if let SceneNode::CollisionShape2D(cs) = node {
                    cs.collider_handle = Some(collider_handle);
                }
            }
        }
    }

    fn stop_rendering_recursive(&self, node_id: NodeID, gfx: &mut Graphics) {
        // DEBUG: Track the problematic node
        let target_node = NodeID::parse_str("d36d3c7f-7c49-497e-b5b2-8770e4e6d633").ok();
        let is_target = target_node.map(|t| node_id == t).unwrap_or(false);

        if is_target {
            println!(
                "🔍 [STOP_RENDER] stop_rendering_recursive called for {}",
                node_id
            );
        }

        // Check if node exists before accessing (might have been deleted)
        if let Some(node) = self.nodes.get(node_id) {
            // Stop rendering this node itself
            gfx.stop_rendering(node_id.as_u64());

            // If it's a UI node, stop rendering all of its UI elements
            if let SceneNode::UINode(ui_node) = node {
                if let Some(elements) = &ui_node.elements {
                    for (element_id, _) in elements {
                        // UIElementID needs to be converted to NodeID for stop_rendering
                        // Note: stop_rendering takes u64; we pass element_id.as_u64()
                        gfx.stop_rendering(element_id.as_u64());
                    }
                }
            }

            // Recursively stop rendering children (only if they still exist)
            // Collect children first to avoid borrowing issues
            let child_ids: Vec<NodeID> = node.get_children().iter().copied().collect();
            if is_target {
                println!(
                    "🔍 [STOP_RENDER] Node {} has children: {:?}",
                    node_id, child_ids
                );
            }
            for child_id in child_ids {
                // Only recurse if child still exists (might have been deleted)
                if self.nodes.contains_key(child_id) {
                    self.stop_rendering_recursive(child_id, gfx);
                } else {
                    let is_target_child = target_node.map(|t| child_id == t).unwrap_or(false);
                    if is_target_child {
                        eprintln!(
                            "⚠️ [STOP_RENDER] Child {} does NOT exist but was in children list of {}",
                            child_id, node_id
                        );
                    }
                }
            }
        } else {
            if is_target {
                eprintln!("⚠️ [STOP_RENDER] Node {} does NOT exist in scene!", node_id);
            }
        }
    }

    // Get nodes needing rerender
    // OPTIMIZED: Returns pre-accumulated set instead of iterating over all nodes (preallocate from drain)
    fn get_nodes_needing_rerender(&mut self) -> Vec<NodeID> {
        let cap = self.needs_rerender.len();
        let mut out = Vec::with_capacity(cap);
        out.extend(self.needs_rerender.drain());
        out
    }

    /// Pre-calculate transforms for nodes in dependency order (parents before children)
    /// This ensures that when calculating a child's transform, the parent's transform is already cached.
    /// This is a major performance optimization when many children share the same moving parent.
    /// OPTIMIZED: Skips non-Node2D parents when calculating depth (they don't affect transform inheritance).
    fn precalculate_transforms_in_dependency_order(&mut self, node_ids: &[NodeID]) {
        // OPTIMIZED: Simple topological sort approach - process nodes whose parents are already processed
        // This avoids the expensive depth calculation that was walking parent chains for every node

        // Step 1: Group Node2D nodes by parent (single pass)
        let mut nodes_by_parent: std::collections::HashMap<Option<NodeID>, Vec<NodeID>> =
            std::collections::HashMap::new();
        let mut node_set = std::collections::HashSet::new();

        for &node_id in node_ids {
            if let Some(node) = self.nodes.get(node_id) {
                if node.as_node2d().is_some() {
                    let parent_id = node.get_parent().map(|p| p.id);
                    nodes_by_parent.entry(parent_id).or_default().push(node_id);
                    node_set.insert(node_id);
                }
            }
        }

        // Step 2: Process nodes iteratively - only process when parent is already processed
        let mut processed = std::collections::HashSet::new();
        let mut changed = true;

        while changed {
            changed = false;

            for (parent_id, siblings) in &nodes_by_parent {
                // Skip if already processed or if parent is in node_set but not yet processed
                let can_process = if let Some(parent) = parent_id {
                    !node_set.contains(parent) || processed.contains(parent)
                } else {
                    true // Root nodes can always be processed
                };

                if can_process {
                    let siblings_to_process: Vec<NodeID> = siblings
                        .iter()
                        .copied()
                        .filter(|&id| !processed.contains(&id))
                        .collect();

                    if !siblings_to_process.is_empty() {
                        changed = true;

                        // Collect node IDs for processing (need to clone since we iterate twice)
                        let node_ids: Vec<NodeID> = siblings_to_process.iter().copied().collect();

                        if let Some(parent) = *parent_id {
                            // Batch process siblings with same parent
                            self.precalculate_transforms_batch(parent, &node_ids);
                        } else {
                            // Root nodes - process individually
                            for node_id in &node_ids {
                                let _ = self.get_global_transform(*node_id);
                            }
                        }

                        for node_id in &node_ids {
                            processed.insert(*node_id);
                        }
                    }
                }
            }
        }
    }

    fn traverse_and_render(&mut self, nodes_needing_rerender: Vec<NodeID>, gfx: &mut Graphics) {
        // Internal enum for parallel render command collection
        enum RenderCommand {
            Texture {
                texture_id: Option<TextureID>,
                texture_path: Option<String>,
                global_transform: crate::structs2d::Transform2D,
                pivot: Vector2,
                z_index: i32,
            },
            Rect {
                transform: crate::structs2d::Transform2D,
                size: Vector2,
                pivot: Vector2,
                color: crate::Color,
                corner_radius: Option<crate::ui_elements::ui_container::CornerRadius>,
                border_thickness: f32,
                is_border: bool,
                z_index: i32,
            },
            UINode,
            Camera2D,
            Camera3D,
            Mesh {
                mesh_id: Option<MeshID>,
                path: Option<String>,
                transform: Transform3D,
                material_path: Option<String>,
            },
            Light(crate::renderer_3d::LightUniform),
        }

        // OPTIMIZED: Pre-calculate transforms in dependency order to avoid redundant parent recalculations
        // When many children share the same moving parent, this ensures the parent's transform
        // is calculated once and cached before all children use it.
        self.precalculate_transforms_in_dependency_order(&nodes_needing_rerender);

        // Pre-calculate 3D global transforms for Node3D nodes (so render commands use world-space transform)
        for &node_id in &nodes_needing_rerender {
            if self
                .nodes
                .get(node_id)
                .and_then(|n| n.as_node3d())
                .is_some()
            {
                let _ = self.get_global_transform_3d(node_id);
            }
        }

        // OPTIMIZED: Parallelize data collection and batch queue operations
        // Collect render commands first, then batch queue them
        const PARALLEL_THRESHOLD: usize = 10;

        if nodes_needing_rerender.len() >= PARALLEL_THRESHOLD {
            // OPTIMIZED: Parallelize data collection and render command building
            // Step 1: Collect all node data in parallel (read-only access)
            let node_data: Vec<_> = nodes_needing_rerender
                .par_iter()
                .filter_map(|&node_id| {
                    // Read-only access to collect initial data
                    if let Some(node) = self.nodes.get(node_id) {
                        let timestamp = node.get_created_timestamp();
                        let needs_transform = node.as_node2d().is_some();
                        let node_type = node.get_type();
                        // OPTIMIZED: After precalculate_transforms_in_dependency_order, transforms are cached
                        let global_transform_opt = if needs_transform {
                            node.as_node2d()
                                .filter(|n2d| !n2d.transform_dirty)
                                .map(|n2d| n2d.global_transform)
                        } else {
                            None
                        };
                        // 3D global transform (pre-calculated above)
                        let global_transform_3d_opt = node
                            .as_node3d()
                            .filter(|n3d| !n3d.transform_dirty)
                            .map(|n3d| n3d.global_transform);
                        Some((
                            node_id,
                            timestamp,
                            needs_transform,
                            node_type,
                            global_transform_opt,
                            global_transform_3d_opt,
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            // Step 2: Process nodes in parallel to build render commands
            // We'll defer texture path resolution until after parallel processing
            let render_data: Vec<_> = node_data
                .par_iter()
                .filter_map(
                    |(node_id, timestamp, _needs_transform, _node_type, global_transform_opt, global_transform_3d_opt)| {
                    // Read-only access to process the node
                    if let Some(node) = self.nodes.get(*node_id) {
                        match node {
                            SceneNode::Sprite2D(sprite) => {
                                if sprite.visible {
                                    // Collect texture info (we'll resolve path later)
                                    let texture_id = sprite.texture_id;
                                    let texture_path = sprite.texture_path.as_ref().map(|p| p.to_string());
                                    if texture_id.is_some() || texture_path.is_some() {
                                        if let Some(global_transform) = global_transform_opt {
                                            return Some((
                                                *node_id,
                                                *timestamp,
                                                RenderCommand::Texture {
                                                    texture_id,
                                                    texture_path,
                                                    global_transform: *global_transform,
                                                    pivot: sprite.pivot,
                                                    z_index: sprite.z_index,
                                                },
                                            ));
                                        }
                                    }
                                }
                            }
                            SceneNode::Camera2D(camera) => {
                                if camera.active {
                                    return Some((*node_id, *timestamp, RenderCommand::Camera2D));
                                }
                            }
                            SceneNode::ShapeInstance2D(shape) => {
                                if shape.visible {
                                    if let Some(shape_type) = shape.shape {
                                        if let Some(transform) = global_transform_opt {
                                            let pivot = shape.pivot;
                                            let z_index = shape.z_index;
                                            let color = shape.color.unwrap_or(crate::Color::new(255, 255, 255, 200));
                                            let border_thickness = if shape.filled { 0.0 } else { 2.0 };
                                            let is_border = !shape.filled;

                                            let rect_cmd = match shape_type {
                                                Shape2D::Rectangle { width, height } => {
                                                    RenderCommand::Rect {
                                                        transform: *transform,
                                                        size: crate::Vector2::new(width, height),
                                                        pivot,
                                                        color,
                                                        corner_radius: None,
                                                        border_thickness,
                                                        is_border,
                                                        z_index,
                                                    }
                                                }
                                                Shape2D::Circle { radius } => {
                                                    let size = radius * 2.0;
                                                    RenderCommand::Rect {
                                                        transform: *transform,
                                                        size: crate::Vector2::new(size, size),
                                                        pivot,
                                                        color,
                                                        corner_radius: Some(crate::ui_elements::ui_container::CornerRadius {
                                                            top_left: radius,
                                                            top_right: radius,
                                                            bottom_left: radius,
                                                            bottom_right: radius,
                                                        }),
                                                        border_thickness,
                                                        is_border,
                                                        z_index,
                                                    }
                                                }
                                                Shape2D::Square { size } => {
                                                    RenderCommand::Rect {
                                                        transform: *transform,
                                                        size: crate::Vector2::new(size, size),
                                                        pivot,
                                                        color,
                                                        corner_radius: None,
                                                        border_thickness,
                                                        is_border,
                                                        z_index,
                                                    }
                                                }
                                                Shape2D::Triangle { base, height } => {
                                                    RenderCommand::Rect {
                                                        transform: *transform,
                                                        size: crate::Vector2::new(base, height),
                                                        pivot,
                                                        color,
                                                        corner_radius: None,
                                                        border_thickness,
                                                        is_border,
                                                        z_index,
                                                    }
                                                }
                                            };
                                            return Some((*node_id, *timestamp, rect_cmd));
                                        }
                                    }
                                }
                            }
                            SceneNode::UINode(_) => {
                                return Some((*node_id, *timestamp, RenderCommand::UINode));
                            }
                            SceneNode::Camera3D(camera) => {
                                if camera.active {
                                    return Some((*node_id, *timestamp, RenderCommand::Camera3D));
                                }
                            }
                            SceneNode::MeshInstance3D(mesh) => {
                                if mesh.visible {
                                    let transform = global_transform_3d_opt.unwrap_or(mesh.base.transform);
                                    let mesh_id = mesh.mesh_id;
                                    let path = mesh.mesh_path.as_ref().map(|p| p.to_string());
                                    if mesh_id.is_some() || path.is_some() {
                                        return Some((
                                            *node_id,
                                            *timestamp,
                                            RenderCommand::Mesh {
                                                mesh_id,
                                                path,
                                                transform,
                                                material_path: mesh.material_path.as_ref().map(|p| p.to_string()),
                                            },
                                        ));
                                    }
                                }
                            }
                            SceneNode::OmniLight3D(light) => {
                                let pos = global_transform_3d_opt
                                    .map(|t| t.position.to_array())
                                    .unwrap_or_else(|| light.base.transform.position.to_array());
                                return Some((
                                    *node_id,
                                    *timestamp,
                                    RenderCommand::Light(crate::renderer_3d::LightUniform {
                                        position: pos,
                                        color: light.color.to_array(),
                                        intensity: light.intensity,
                                        ambient: [0.05, 0.05, 0.05],
                                        ..Default::default()
                                    }),
                                ));
                            }
                            SceneNode::DirectionalLight3D(light) => {
                                let dir = global_transform_3d_opt
                                    .map(|t| t.forward())
                                    .unwrap_or_else(|| light.base.transform.forward());
                                return Some((
                                    *node_id,
                                    *timestamp,
                                    RenderCommand::Light(crate::renderer_3d::LightUniform {
                                        position: [dir.x, dir.y, dir.z],
                                        color: light.color.to_array(),
                                        intensity: light.intensity,
                                        ambient: [0.05, 0.05, 0.05],
                                        ..Default::default()
                                    }),
                                ));
                            }
                            SceneNode::SpotLight3D(light) => {
                                let dir = global_transform_3d_opt
                                    .map(|t| t.forward())
                                    .unwrap_or_else(|| light.base.transform.forward());
                                return Some((
                                    *node_id,
                                    *timestamp,
                                    RenderCommand::Light(crate::renderer_3d::LightUniform {
                                        position: [dir.x, dir.y, dir.z],
                                        color: light.color.to_array(),
                                        intensity: light.intensity,
                                        ambient: [0.05, 0.05, 0.05],
                                        ..Default::default()
                                    }),
                                ));
                            }
                            _ => {}
                        }
                    }
                    None
                })
                .collect();

            // Step 3: Separate render commands by type and resolve texture paths (preallocate to avoid realloc in hot path)
            let n = render_data.len();
            let mut rect_commands = Vec::with_capacity(n);
            let mut texture_commands = Vec::with_capacity(n);
            let mut texture_id_updates = Vec::new(); // (node_id, new_texture_id) when evicted id was reloaded
            let mut ui_nodes = Vec::with_capacity(n);
            let mut camera_2d_updates = Vec::with_capacity(n);
            let mut camera_3d_updates = Vec::with_capacity(n);
            let mut mesh_commands = Vec::with_capacity(n);
            let mut light_commands = Vec::with_capacity(n);

            for (node_id, timestamp, cmd) in render_data {
                match cmd {
                    RenderCommand::Texture {
                        texture_id,
                        texture_path,
                        global_transform,
                        pivot,
                        z_index,
                    } => {
                        // Resolve texture_id (use existing or load from path)
                        let resolved_id = match texture_id {
                            Some(id) => Some(id),
                            None => texture_path.as_deref().and_then(|path| {
                                gfx.texture_manager
                                    .get_or_load_texture_id(path, &gfx.device, &gfx.queue)
                                    .ok()
                            }),
                        };
                        if let Some(tex_id) = resolved_id {
                            // If id was evicted, reload from disk (we keep id→path when evicting)
                            let effective_id = gfx
                                .texture_manager
                                .ensure_texture_loaded(tex_id, &gfx.device, &gfx.queue)
                                .unwrap_or(tex_id);
                            if effective_id != tex_id {
                                texture_id_updates.push((node_id, effective_id));
                            }
                            texture_commands.push((
                                node_id,
                                effective_id,
                                global_transform,
                                pivot,
                                z_index,
                                timestamp,
                            ));
                        }
                    }
                    RenderCommand::Rect {
                        transform,
                        size,
                        pivot,
                        color,
                        corner_radius,
                        border_thickness,
                        is_border,
                        z_index,
                    } => {
                        rect_commands.push((
                            node_id,
                            transform,
                            size,
                            pivot,
                            color,
                            corner_radius,
                            border_thickness,
                            is_border,
                            z_index,
                            timestamp,
                        ));
                    }
                    RenderCommand::UINode => {
                        ui_nodes.push(node_id);
                    }
                    RenderCommand::Camera2D => {
                        camera_2d_updates.push(node_id);
                    }
                    RenderCommand::Camera3D => {
                        camera_3d_updates.push(node_id);
                    }
                    RenderCommand::Mesh {
                        mesh_id,
                        path,
                        transform,
                        material_path,
                    } => {
                        mesh_commands.push((node_id, mesh_id, path, transform, material_path));
                    }
                    RenderCommand::Light(light_uniform) => {
                        light_commands.push((node_id, light_uniform));
                    }
                }
            }

            // Step 4: Clear transform_dirty flags sequentially (needs mutable access)
            for &node_id in &nodes_needing_rerender {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    if let Some(node2d) = node.as_node2d_mut() {
                        node2d.transform_dirty = false;
                    }
                    if let Some(node3d) = node.as_node3d_mut() {
                        node3d.transform_dirty = false;
                    }
                }
            }

            // OPTIMIZED: Batch queue operations
            // Queue all rects
            for (
                node_id,
                transform,
                size,
                pivot,
                color,
                corner_radius,
                border_thickness,
                is_border,
                z_index,
                timestamp,
            ) in rect_commands
            {
                gfx.renderer_2d.queue_rect(
                    &mut gfx.renderer_prim,
                    node_id,
                    transform,
                    size,
                    pivot,
                    color,
                    corner_radius,
                    border_thickness,
                    is_border,
                    z_index,
                    timestamp,
                );
            }

            // Queue all textures (by id; path→id already resolved above)
            for (node_id, tex_id, global_transform, pivot, z_index, timestamp) in texture_commands {
                gfx.renderer_2d.queue_texture(
                    &mut gfx.renderer_prim,
                    &mut gfx.texture_manager,
                    &gfx.device,
                    &gfx.queue,
                    node_id,
                    tex_id,
                    global_transform,
                    pivot,
                    z_index,
                    timestamp,
                );
            }
            // Update node texture_ids when we reloaded an evicted texture (so next frame uses the new id)
            for (node_id, new_id) in texture_id_updates {
                if let Some(SceneNode::Sprite2D(sprite)) = self.nodes.get_mut(node_id) {
                    sprite.texture_id = Some(new_id);
                }
            }

            // Process UI nodes (they need mutable access to gfx)
            for node_id in ui_nodes {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    if let SceneNode::UINode(ui_node) = node {
                        // Pass provider so FUR can be loaded using the correct method (dev vs release)
                        render_ui(ui_node, gfx, Some(&self.provider));
                    }
                }
            }

            // Update cameras
            for node_id in camera_2d_updates {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    if let SceneNode::Camera2D(camera) = node {
                        if camera.active {
                            gfx.update_camera_2d(camera);
                        }
                    }
                }
            }

            for node_id in camera_3d_updates {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    if let SceneNode::Camera3D(camera) = node {
                        if camera.active {
                            gfx.update_camera_3d(camera);
                        }
                    }
                }
            }

            // Queue meshes (preallocate mesh_id_updates for eviction reloads)
            let mut mesh_id_updates: Vec<(NodeID, MeshID)> =
                Vec::with_capacity(mesh_commands.len());
            for (node_id, mesh_id, path, transform, material_path) in mesh_commands {
                // Resolve mesh id (prefer existing id; reload from path if evicted/absent)
                let resolved_id: Option<MeshID> = match mesh_id {
                    Some(id) if gfx.mesh_manager.get_mesh_by_id(id).is_some() => Some(id),
                    Some(id) => {
                        // Try reload via remembered id->path mapping, or provided path
                        let reload_path = gfx
                            .mesh_manager
                            .get_mesh_path_from_id(&id)
                            .map(|s| s.to_string())
                            .or(path.clone());
                        reload_path.and_then(|p| {
                            gfx.mesh_manager
                                .get_or_load_mesh(&p, &gfx.device, &gfx.queue)
                        })
                    }
                    None => path.as_deref().and_then(|p| {
                        gfx.mesh_manager
                            .get_or_load_mesh(p, &gfx.device, &gfx.queue)
                    }),
                };

                if let Some(mid) = resolved_id {
                    gfx.renderer_3d.queue_mesh_id(
                        node_id,
                        mid,
                        transform,
                        material_path.as_deref(),
                        &mut gfx.mesh_manager,
                        &mut gfx.material_manager,
                    );
                    mesh_id_updates.push((node_id, mid));
                }
            }
            // Update node mesh_ids (so script-side `mesh` reflects the runtime handle)
            for (node_id, new_id) in mesh_id_updates {
                if let Some(SceneNode::MeshInstance3D(mesh)) = self.nodes.get_mut(node_id) {
                    mesh.mesh_id = Some(new_id);
                }
            }

            // Queue lights (allocate LightID from LightManager if needed)
            for (node_id, light_uniform) in light_commands {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    let light_id = match node {
                        SceneNode::OmniLight3D(light) => {
                            if light.light_id.is_none() {
                                light.light_id = Some(gfx.light_manager.allocate());
                            }
                            light.light_id.unwrap()
                        }
                        SceneNode::DirectionalLight3D(light) => {
                            if light.light_id.is_none() {
                                light.light_id = Some(gfx.light_manager.allocate());
                            }
                            light.light_id.unwrap()
                        }
                        SceneNode::SpotLight3D(light) => {
                            if light.light_id.is_none() {
                                light.light_id = Some(gfx.light_manager.allocate());
                            }
                            light.light_id.unwrap()
                        }
                        _ => continue,
                    };
                    gfx.renderer_3d.queue_light(light_id, light_uniform);
                }
            }
        } else {
            // Small batch - use original sequential approach (less overhead)
            for node_id in nodes_needing_rerender {
                // Get global transform first (before borrowing node mutably)
                let global_transform_opt = if let Some(node) = self.nodes.get(node_id) {
                    if node.as_node2d().is_some() {
                        self.get_global_transform(node_id)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let global_transform_3d_opt = if self
                    .nodes
                    .get(node_id)
                    .and_then(|n| n.as_node3d())
                    .is_some()
                {
                    self.get_global_transform_3d(node_id)
                } else {
                    None
                };

                if let Some(node) = self.nodes.get_mut(node_id) {
                    let timestamp = node.get_created_timestamp();
                    match node {
                        SceneNode::Sprite2D(sprite) => {
                            if sprite.visible {
                                let resolved_id = match &sprite.texture_id {
                                    Some(id) => Some(*id),
                                    None => sprite.texture_path.as_deref().and_then(|path| {
                                        gfx.texture_manager
                                            .get_or_load_texture_id(path, &gfx.device, &gfx.queue)
                                            .ok()
                                    }),
                                };
                                if let Some(tex_id) = resolved_id {
                                    // If id was evicted, reload from disk and update node so next frame uses new id
                                    let effective_id = gfx
                                        .texture_manager
                                        .ensure_texture_loaded(tex_id, &gfx.device, &gfx.queue)
                                        .unwrap_or(tex_id);
                                    if effective_id != tex_id {
                                        sprite.texture_id = Some(effective_id);
                                    }
                                    if let Some(global_transform) = global_transform_opt {
                                        gfx.renderer_2d.queue_texture(
                                            &mut gfx.renderer_prim,
                                            &mut gfx.texture_manager,
                                            &gfx.device,
                                            &gfx.queue,
                                            node_id,
                                            effective_id,
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
                        SceneNode::ShapeInstance2D(shape) => {
                            if shape.visible {
                                if let Some(shape_type) = shape.shape {
                                    if let Some(transform) = global_transform_opt {
                                        let pivot = shape.pivot;
                                        let z_index = shape.z_index;
                                        let color = shape
                                            .color
                                            .unwrap_or(crate::Color::new(255, 255, 255, 200));
                                        let border_thickness = if shape.filled { 0.0 } else { 2.0 };
                                        let is_border = !shape.filled;

                                        match shape_type {
                                            Shape2D::Rectangle { width, height } => {
                                                gfx.renderer_2d.queue_rect(
                                                    &mut gfx.renderer_prim,
                                                    node_id,
                                                    transform,
                                                    crate::Vector2::new(width, height),
                                                    pivot,
                                                    color,
                                                    None,
                                                    border_thickness,
                                                    is_border,
                                                    z_index,
                                                    timestamp,
                                                );
                                            }
                                            Shape2D::Circle { radius } => {
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
                                            Shape2D::Square { size } => {
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
                                            Shape2D::Triangle { base, height } => {
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
                            // Pass provider so FUR can be loaded using the correct method (dev vs release)
                            render_ui(ui_node, gfx, Some(&self.provider));
                        }
                        SceneNode::Camera3D(camera) => {
                            if camera.active {
                                gfx.update_camera_3d(camera);
                            }
                        }
                        SceneNode::MeshInstance3D(mesh) => {
                            if mesh.visible {
                                let transform =
                                    global_transform_3d_opt.unwrap_or(mesh.base.transform);

                                // Resolve mesh id: prefer `mesh.mesh_id`, else load from `mesh_path`.
                                // If the id was evicted, reload from remembered path or mesh_path.
                                let resolved_id: Option<MeshID> = match mesh.mesh_id {
                                    Some(id) if gfx.mesh_manager.get_mesh_by_id(id).is_some() => {
                                        Some(id)
                                    }
                                    Some(id) => {
                                        // IMPORTANT: materialize the reload path as an owned String first,
                                        // so we don't hold an immutable borrow of gfx.mesh_manager while
                                        // trying to mutably borrow it to reload.
                                        let reload_path: Option<String> = gfx
                                            .mesh_manager
                                            .get_mesh_path_from_id(&id)
                                            .map(|s| s.to_string())
                                            .or_else(|| {
                                                mesh.mesh_path.as_deref().map(|s| s.to_string())
                                            });

                                        if let Some(p) = reload_path.as_deref() {
                                            gfx.mesh_manager.get_or_load_mesh(
                                                p,
                                                &gfx.device,
                                                &gfx.queue,
                                            )
                                        } else {
                                            None
                                        }
                                    }
                                    None => mesh.mesh_path.as_deref().and_then(|p| {
                                        gfx.mesh_manager.get_or_load_mesh(
                                            p,
                                            &gfx.device,
                                            &gfx.queue,
                                        )
                                    }),
                                };

                                if let Some(mid) = resolved_id {
                                    gfx.renderer_3d.queue_mesh_id(
                                        node_id,
                                        mid,
                                        transform,
                                        mesh.material_path.as_deref(),
                                        &mut gfx.mesh_manager,
                                        &mut gfx.material_manager,
                                    );
                                    mesh.mesh_id = Some(mid);
                                }
                            }
                        }
                        SceneNode::OmniLight3D(light) => {
                            if light.light_id.is_none() {
                                light.light_id = Some(gfx.light_manager.allocate());
                            }
                            let light_id = light.light_id.unwrap();
                            let pos = global_transform_3d_opt
                                .map(|t| t.position.to_array())
                                .unwrap_or_else(|| light.base.transform.position.to_array());
                            gfx.renderer_3d.queue_light(
                                light_id,
                                crate::renderer_3d::LightUniform {
                                    position: pos,
                                    color: light.color.to_array(),
                                    intensity: light.intensity,
                                    ambient: [0.05, 0.05, 0.05],
                                    ..Default::default()
                                },
                            );
                        }
                        SceneNode::DirectionalLight3D(light) => {
                            if light.light_id.is_none() {
                                light.light_id = Some(gfx.light_manager.allocate());
                            }
                            let light_id = light.light_id.unwrap();
                            let dir = global_transform_3d_opt
                                .map(|t| t.forward())
                                .unwrap_or_else(|| light.base.transform.forward());
                            gfx.renderer_3d.queue_light(
                                light_id,
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
                            if light.light_id.is_none() {
                                light.light_id = Some(gfx.light_manager.allocate());
                            }
                            let light_id = light.light_id.unwrap();
                            let dir = global_transform_3d_opt
                                .map(|t| t.forward())
                                .unwrap_or_else(|| light.base.transform.forward());
                            gfx.renderer_3d.queue_light(
                                light_id,
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
                    if let Some(node2d) = node.as_node2d_mut() {
                        node2d.transform_dirty = false;
                    }
                    if let Some(node3d) = node.as_node3d_mut() {
                        node3d.transform_dirty = false;
                    }
                }
            }
        }
    }
}

//
// ---------------- SceneAccess impl ----------------
//

impl<P: ScriptProvider + 'static> SceneAccess for Scene<P> {
    fn get_scene_node_ref(&self, id: NodeID) -> Option<&SceneNode> {
        self.nodes.get(id)
    }

    fn get_scene_node_mut(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        self.nodes.get_mut(id)
    }

    fn mark_needs_rerender(&mut self, node_id: NodeID) {
        // Only add renderable nodes to needs_rerender
        // Non-renderable nodes (like Node, Node2D, Area2D) don't need to be rendered
        if let Some(node) = self.nodes.get(node_id) {
            // Check if node is renderable before adding
            if !node.is_renderable() {
                return; // Skip non-renderable nodes
            }

            // Check if node is already in the HashSet (O(1) check)
            // If not in set, add it
            if !self.needs_rerender.contains(&node_id) {
                self.needs_rerender.insert(node_id);
            }
        }
    }

    fn get_scene_node(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        self.get_scene_node_mut(id)
    }

    fn next_node_id(&mut self) -> NodeID {
        self.nodes.next_id()
    }

    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.provider.load_ctor(short)
    }

    fn instantiate_script(&mut self, ctor: CreateFn, node_id: NodeID) -> Box<dyn ScriptObject> {
        // Trait requires Box, but we wrap it in Rc<RefCell<>> when inserting into scripts HashMap
        let raw = ctor();
        let mut boxed: Box<dyn ScriptObject> = unsafe { Box::from_raw(raw) };
        boxed.set_id(node_id);
        boxed
    }

    fn add_node_to_scene(
        &mut self,
        node: SceneNode,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<NodeID> {
        self.add_node_to_scene(node, gfx)
    }

    fn connect_signal_id(&mut self, signal: SignalID, target_id: NodeID, function: u64) {
        self.connect_signal(signal, target_id, function);
    }

    fn get_signal_connections(
        &self,
        signal: SignalID,
    ) -> Option<&HashMap<NodeID, SmallVec<[u64; 4]>>> {
        self.signals.connections.get(&signal)
    }

    fn emit_signal_id_deferred(&mut self, signal: SignalID, params: &[Value]) {
        self.emit_signal_id_deferred(signal, params);
    }

    fn call_function_id_deferred(&mut self, node_id: NodeID, function_id: u64, params: &[Value]) {
        self.call_function_id_deferred(node_id, function_id, params);
    }

    fn get_script(&mut self, id: NodeID) -> Option<Rc<UnsafeCell<Box<dyn ScriptObject>>>> {
        // Clone the Rc so the script stays in the HashMap
        self.scripts.get(&id).map(|rc| Rc::clone(rc))
    }

    fn get_script_mut(&mut self, id: NodeID) -> Option<Rc<UnsafeCell<Box<dyn ScriptObject>>>> {
        self.get_script(id)
    }

    fn take_script(&mut self, id: NodeID) -> Option<Rc<UnsafeCell<Box<dyn ScriptObject>>>> {
        // Scripts are now always in memory, just clone the Rc
        self.get_script(id)
    }

    fn insert_script(&mut self, _id: NodeID, _script: Box<dyn ScriptObject>) {
        // Scripts are now stored as Rc<RefCell<Box<>>>, so we don't need to insert them back
        // This method is kept for compatibility but does nothing
    }

    // NEW method implementation
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>> {
        self.app_command_tx.as_ref()
    }

    fn get_controller_manager(&self) -> Option<&Mutex<ControllerManager>> {
        // Only create controller manager if explicitly enabled
        if !self
            .controller_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        // OPTIMIZED: Lazy initialization - create on first access using OnceCell
        self.controller_manager
            .get_or_init(|| Mutex::new(ControllerManager::new()));
        self.controller_manager.get()
    }

    fn enable_controller_manager(&self) -> bool {
        // Enable controllers and initialize the manager
        self.controller_enabled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        // Force initialization by calling get_or_init
        self.controller_manager
            .get_or_init(|| Mutex::new(ControllerManager::new()));
        true
    }

    fn get_input_manager(&self) -> Option<&Mutex<InputManager>> {
        Some(&self.input_manager)
    }

    fn get_physics_2d(&self) -> Option<&std::cell::RefCell<PhysicsWorld2D>> {
        self.physics_2d.as_ref()
    }

    fn get_parent_opt(&mut self, node_id: NodeID) -> Option<NodeID> {
        self.nodes
            .get(node_id)
            .and_then(|n| n.get_parent().map(|p| p.id))
    }

    fn get_global_transform(&mut self, node_id: NodeID) -> Option<crate::structs2d::Transform2D> {
        Self::get_global_transform(self, node_id)
    }

    fn set_global_transform(
        &mut self,
        node_id: NodeID,
        transform: crate::structs2d::Transform2D,
    ) -> Option<()> {
        Self::set_global_transform(self, node_id, transform)
    }

    fn mark_transform_dirty_recursive(&mut self, node_id: NodeID) {
        Self::mark_transform_dirty_recursive(self, node_id)
    }

    fn update_node2d_children_cache_on_add(&mut self, parent_id: NodeID, child_id: NodeID) {
        Self::update_node2d_children_cache_on_add(self, parent_id, child_id)
    }

    fn update_node2d_children_cache_on_remove(&mut self, parent_id: NodeID, child_id: NodeID) {
        Self::update_node2d_children_cache_on_remove(self, parent_id, child_id)
    }

    fn update_node2d_children_cache_on_clear(&mut self, parent_id: NodeID) {
        Self::update_node2d_children_cache_on_clear(self, parent_id)
    }

    fn update_node3d_children_cache_on_add(&mut self, parent_id: NodeID, child_id: NodeID) {
        Self::update_node3d_children_cache_on_add(self, parent_id, child_id)
    }

    fn update_node3d_children_cache_on_remove(&mut self, parent_id: NodeID, child_id: NodeID) {
        Self::update_node3d_children_cache_on_remove(self, parent_id, child_id)
    }

    fn update_node3d_children_cache_on_clear(&mut self, parent_id: NodeID) {
        Self::update_node3d_children_cache_on_clear(self, parent_id)
    }

    fn get_global_transform_3d(
        &mut self,
        node_id: NodeID,
    ) -> Option<crate::structs3d::Transform3D> {
        Self::get_global_transform_3d(self, node_id)
    }

    fn set_global_transform_3d(
        &mut self,
        node_id: NodeID,
        transform: crate::structs3d::Transform3D,
    ) -> Option<()> {
        Self::set_global_transform_3d(self, node_id, transform)
    }

    fn remove_node(&mut self, node_id: NodeID, gfx: &mut crate::rendering::Graphics) {
        Self::remove_node(self, node_id, gfx)
    }
}

//
// ---------------- Specialization for DllScriptProvider ----------------
//

use crate::registry::DllScriptProvider;
use libloading::Library;

pub fn default_perro_rust_path() -> io::Result<PathBuf> {
    // Try to get project root from global state first
    match get_project_root() {
        ProjectRoot::Disk { root, .. } => {
            let mut path = root;
            path.push(".perro");
            path.push("scripts");
            path.push("builds");

            // Use centralized naming from compiler so the loader and compiler agree
            let filename = crate::scripting::compiler::script_dylib_name();

            path.push(filename);
            Ok(path)
        }
        ProjectRoot::Brk { .. } => Err(io::Error::new(
            io::ErrorKind::Other,
            "default_perro_rust_path is not available in release/export mode",
        )),
    }
}

/// Get the default perro rust path using a project root path directly
/// This avoids requiring the global project root to be set
pub fn default_perro_rust_path_from_root(project_root: &Path) -> PathBuf {
    let mut path = project_root.to_path_buf();
    path.push(".perro");
    path.push("scripts");
    path.push("builds");

    let filename = crate::scripting::compiler::script_dylib_name();
    path.push(filename);
    path
}

impl Scene<DllScriptProvider> {
    pub fn from_project(
        project: Rc<RefCell<Project>>,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<Self> {
        // Load DLL - use project root from project parameter to avoid requiring global project root
        let lib_path = {
            let root_path = {
                let project_ref = project.borrow();
                project_ref
                    .root()
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Project root path not set"))?
                    .to_path_buf()
            };
            default_perro_rust_path_from_root(&root_path)
        };

        // Check if DLL exists before trying to load it
        if !lib_path.exists() {
            return Err(anyhow::anyhow!(
                "Script DLL not found at {:?}. Please compile scripts first using: cargo run -p perro_core -- --path <path> --scripts",
                lib_path
            ));
        }

        let lib = unsafe {
            Library::new(&lib_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to load DLL at {:?}: {}. The DLL might be corrupted or incompatible.",
                    lib_path,
                    e
                )
            })?
        };
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

        // CRITICAL: Set the global project root BEFORE initializing scripts
        // This ensures that res:// paths can be resolved when scripts call Texture.load() etc.
        set_project_root(root.clone());

        // CRITICAL: On Windows, DLLs have separate static variable instances from the main binary.
        // This means the DLL cannot see the project root set in the main binary. We MUST inject
        // the project root into the DLL so that when script code (running in the DLL) calls
        // resolve_path(), it can access the DLL's copy of PROJECT_ROOT.
        //
        // This can potentially cause STATUS_ACCESS_VIOLATION if the DLL was built against a
        // different version of perro_core, but if the DLL is built correctly, it should work.
        // We handle errors gracefully to avoid crashes.
        if let Err(e) = provider.inject_project_root(&root) {
            eprintln!("⚠ Warning: Failed to inject project root into DLL: {}", e);
            eprintln!("   Scripts may not be able to resolve res:// paths. This usually means:");
            eprintln!("   1. The DLL was built against a different version of perro_core");
            eprintln!("   2. The perro_set_project_root symbol is missing from the DLL");
            eprintln!(
                "   Try rebuilding scripts: cargo run -p perro_core -- --path <path> --scripts"
            );
        } else {
        }

        // Create game root (id 1) — Root, owns main scene, has root script.
        let mut root_node = Node::new();
        root_node.name = Cow::Borrowed("Root");
        let root_node = SceneNode::Node(root_node);
        let mut game_scene = Scene::new(root_node, provider, project);

        // Initialize input manager with action map from project.toml
        {
            let project_ref = game_scene.project.borrow();
            let input_map = project_ref.get_input_map();
            let mut input_mgr = game_scene.input_manager.lock().unwrap();
            input_mgr.load_action_map(input_map);
        }

        // Global order: index 0 = @root script (attaches to Root node NodeID(1)); indices 1.. = @global scripts (siblings of Root).
        let global_order: Vec<String> = game_scene
            .provider
            .get_global_registry_order()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let global_names: Vec<String> = game_scene
            .provider
            .get_global_registry_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let root_id = game_scene.get_root().get_id();
        let mut global_node_ids: Vec<NodeID> = Vec::with_capacity(global_order.len());
        let first_is_root = global_names.first().map(|s| s.as_str()).unwrap_or("") == "Root";
        for (i, identifier) in global_order.iter().enumerate() {
            let name = global_names
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or(identifier.as_str());
            if i == 0 && first_is_root {
                global_node_ids.push(root_id);
            } else {
                let mut node = Node::new();
                node.name = Cow::Owned(name.to_string());
                let global_node = SceneNode::Node(node);
                let global_id = game_scene.nodes.insert(global_node);
                global_node_ids.push(global_id);
            }
        }

        // ✅ attach global scripts in order (index 0 = root script on root node when present) and call init
        for (identifier, &global_id) in global_order.iter().zip(global_node_ids.iter()) {
            if let Ok(ctor) = game_scene.provider.load_ctor(identifier.as_str()) {
                let boxed = game_scene.instantiate_script(ctor, global_id);
                let handle = Rc::new(UnsafeCell::new(boxed));
                game_scene.scripts.insert(global_id, handle);

                let project_ref = game_scene.project.clone();
                let mut project_borrow = project_ref.borrow_mut();
                let now = Instant::now();
                let true_delta = match game_scene.last_scene_update {
                    Some(prev) => now.duration_since(prev).as_secs_f32(),
                    None => 0.0,
                };
                let mut api =
                    ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow, gfx);
                api.apply_exposed_vars_from_node(global_id);
                api.call_init(global_id);
                if let Some(node) = game_scene.nodes.get(global_id) {
                    if node.is_renderable() {
                        game_scene.needs_rerender.insert(global_id);
                    }
                }
            }
        }

        // Timer: retrieve (file I/O + parse in dev) + remap only
        let main_scene_path = game_scene.project.borrow().main_scene().to_string();
        let scene_load_start = Instant::now();
        let loaded_data = SceneData::load(&main_scene_path)?;
        let load_ms = scene_load_start.elapsed().as_secs_f64() * 1000.0;

        let merge_start = Instant::now();
        let game_root = game_scene.get_root().get_id();
        game_scene.merge_scene_data(loaded_data, game_root, gfx)?;
        let merge_ms = merge_start.elapsed().as_secs_f64() * 1000.0;

        let total_ms = scene_load_start.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "⏱️ Scene (retrieve+remap): load {:.2}ms, merge {:.2}ms, total {:.2}ms",
            load_ms, merge_ms, total_ms
        );
        Ok(game_scene)
    }
}
