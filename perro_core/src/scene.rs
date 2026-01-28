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
    node_registry::{BaseNode, NodeType, SceneNode},
    physics::physics_2d::PhysicsWorld2D,
    script::{CreateFn, SceneAccess, ScriptObject, ScriptProvider},
    transpiler::script_path_to_identifier,
    ui_renderer::render_ui, // NEW import
};
use std::sync::Mutex;
use once_cell::sync::OnceCell;

use indexmap::IndexMap;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::{SerializeStruct, Error as SerdeError}};
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
use crate::uid32::{Uid32, NodeID, TextureID};

//
// ---------------- SceneData ----------------
//

/// Pure serializable scene data (no runtime state)
/// Uses u32 keys as scene-level node identifiers to preserve order during serialization
#[derive(Debug)]
pub struct SceneData {
    pub root_key: u32,
    pub nodes: IndexMap<u32, SceneNode>,
    // Mapping from scene key to NodeID (used during deserialization)
    // This allows us to remap parent references when converting to runtime
    // Not serialized - handled manually in Serialize/Deserialize impls
    key_to_node_id: HashMap<u32, NodeID>,
}

impl Clone for SceneData {
    fn clone(&self) -> Self {
        Self {
            root_key: self.root_key,
            nodes: self.nodes.iter().map(|(key, node)| {
                (*key, node.clone())
            }).collect(),
            key_to_node_id: self.key_to_node_id.clone(),
        }
    }
}

impl Serialize for SceneData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SceneData", 2)?;
        // Support both "root_key" (new) and "root_index" (legacy) for backward compatibility
        state.serialize_field("root_index", &self.root_key)?;
        
        // Build reverse mapping: NodeID -> key (for converting parent NodeIDs to scene keys)
        let node_id_to_key: HashMap<NodeID, u32> = self.key_to_node_id.iter()
            .map(|(key, node_id)| (*node_id, *key))
            .collect();
        
        // Serialize nodes with u32 keys as identifiers
        struct NodesMap<'a> {
            nodes: &'a IndexMap<u32, SceneNode>,
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
                    // Serialize node, but convert parent Uid32 to scene key
                    let mut node_value: Value = serde_json::to_value(node)
                        .map_err(|e| S::Error::custom(format!("Failed to serialize node: {}", e)))?;
                    
                    // Convert parent NodeID to scene key if present
                    if let Some(obj) = node_value.as_object_mut() {
                        if let Some(parent_value) = obj.get_mut("parent") {
                            if let Some(parent_obj) = parent_value.as_object_mut() {
                                if let Some(id_value) = parent_obj.get("id") {
                                    if let Some(uid_str) = id_value.as_str() {
                                        if let Ok(uid) = Uid32::parse_str(uid_str) {
                                            let node_id = NodeID::from_uid32(uid);
                                            if let Some(&parent_key) = self.node_id_to_key.get(&node_id) {
                                                // Replace parent object with just the scene key
                                                *parent_value = serde_json::Value::Number(parent_key.into());
                                            }
                                        }
                                    }
                                }
                            } else if let Some(uid_str) = parent_value.as_str() {
                                // Parent is a Uid32 hex string, convert to scene key
                                if let Ok(uid) = Uid32::parse_str(uid_str) {
                                    let node_id = NodeID::from_uid32(uid);
                                    if let Some(&parent_key) = self.node_id_to_key.get(&node_id) {
                                        *parent_value = serde_json::Value::Number(parent_key.into());
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
        
        state.serialize_field("nodes", &NodesMap { 
            nodes: &self.nodes,
            node_id_to_key: &node_id_to_key,
        })?;
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
        
        // Accept both "root_key" (new), "root_index" (legacy), and "root_id" (legacy) for compatibility
        let root_key = raw_value.get("root_key")
            .or_else(|| raw_value.get("root_index"))
            .or_else(|| raw_value.get("root_id"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| D::Error::custom("root_key, root_index, or root_id must be a u32"))? as u32;
        
        let nodes_obj = raw_value.get("nodes")
            .and_then(|v| v.as_object())
            .ok_or_else(|| D::Error::custom("nodes must be an object"))?;
        
        let capacity = nodes_obj.len();
        
        // Create scene key -> NodeID mapping using deterministic NodeIDs based on keys
        // Use the key directly to generate a deterministic NodeID (with a small offset to avoid nil)
        let mut key_to_node_id: HashMap<u32, NodeID> = HashMap::with_capacity(capacity);
        for key_str in nodes_obj.keys() {
            if let Ok(key) = key_str.parse::<u32>() {
                // Generate deterministic NodeID from scene key (add 1 to avoid nil)
                let uid = Uid32::from_u32(key.wrapping_add(1));
                let node_id = NodeID::from_uid32(uid);
                key_to_node_id.insert(key, node_id);
            }
        }
        
        // Deserialize nodes, handling parent scene keys
        let mut nodes = IndexMap::with_capacity(capacity);
        let mut parent_children: HashMap<u32, Vec<u32>> = HashMap::with_capacity(capacity / 4);
        
        // Helper function to recursively find "parent" field in nested JSON
        fn find_parent_recursive(value: &Value) -> Option<u32> {
            if let Some(obj) = value.as_object() {
                // Check if "parent" exists at this level
                if let Some(parent_val) = obj.get("parent") {
                    if let Some(n) = parent_val.as_u64() {
                        return Some(n as u32);
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
            let key = key_str.parse::<u32>()
                .map_err(|_| D::Error::custom(format!("Node key must be a u32 scene identifier, got: {}", key_str)))?;
            
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
                            let parent_type = crate::nodes::node::ParentType::new(parent_node_id, parent_type_enum);
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
        
        // Store key_to_node_id mapping for later use when converting to runtime
        let key_to_node_id_map: HashMap<u32, NodeID> = key_to_node_id.into_iter().collect();
        
        Ok(SceneData {
            root_key,
            nodes,
            key_to_node_id: key_to_node_id_map,
        })
    }
}

impl SceneData {
    /// Get the scene key to NodeID mapping
    pub fn key_to_node_id(&self) -> &HashMap<u32, NodeID> {
        &self.key_to_node_id
    }
    
    /// Create a new data scene with a root node
    pub fn new(root: SceneNode) -> Self {
        let root_id = root.get_id();
        let mut nodes = IndexMap::new();
        // OPTIMIZED: Use with_capacity(0) for known-empty map to avoid pre-allocation
        let mut key_to_node_id = HashMap::with_capacity(0);
        // Use key 0 for root
        key_to_node_id.insert(0, root_id);
        nodes.insert(0, root);
        Self { 
            root_key: 0, 
            nodes,
            key_to_node_id,
        }
    }
    
    /// Create SceneData from nodes and root_key
    /// Builds the key_to_node_id mapping and sets node IDs to deterministic NodeIDs based on scene keys
    pub fn from_nodes(root_key: u32, mut nodes: IndexMap<u32, SceneNode>) -> Self {
        let mut key_to_node_id = HashMap::with_capacity(nodes.len());
        
        // Generate deterministic NodeIDs based on scene keys
        for (&key, node) in nodes.iter_mut() {
            // Create deterministic NodeID from scene key (add 1 to avoid nil)
            let uid = Uid32::from_u32(key.wrapping_add(1));
            let node_id = NodeID::from_uid32(uid);
            
            // Set the node's ID to match the deterministic NodeID
            node.set_id(node_id);
            
            // Store in mapping
            key_to_node_id.insert(key, node_id);
        }
        
        // Now update parent and child NodeIDs to match the deterministic NodeIDs
        for node in nodes.values_mut() {
            // Update parent NodeID if it exists
            if let Some(parent) = node.get_parent() {
                let parent_uid = parent.id.as_uid32().as_u32();
                
                // Try to find the parent key in two ways:
                // 1. Check if parent ID matches a key directly (static scene data format)
                // 2. Check if parent ID is key+1 (from_nodes format)
                let parent_key_opt = if parent_uid > 0 {
                    // First try: parent ID is the key directly (static scene data)
                    if key_to_node_id.contains_key(&parent_uid) {
                        Some(parent_uid)
                    } else {
                        // Second try: parent ID is key+1 (from_nodes format)
                        let key_candidate = parent_uid.wrapping_sub(1);
                        if key_to_node_id.contains_key(&key_candidate) {
                            Some(key_candidate)
                        } else {
                            None
                        }
                    }
                } else {
                    None
                };
                
                if let Some(parent_key) = parent_key_opt {
                    if let Some(&correct_node_id) = key_to_node_id.get(&parent_key) {
                        if parent.id != correct_node_id {
                            // Update parent ID to match
                            let parent_type = crate::nodes::node::ParentType::new(correct_node_id, parent.node_type);
                            node.set_parent(Some(parent_type));
                        }
                    }
                }
            }
            
            // Update children NodeIDs
            let children = node.get_children().clone();
            node.clear_children();
            for child_id in children {
                let child_uid = child_id.as_uid32().as_u32();
                
                // Try to find the child key in two ways:
                // 1. Check if child ID matches a key directly (static scene data format)
                // 2. Check if child ID is key+1 (from_nodes format)
                let child_key_opt = if child_uid > 0 {
                    // First try: child ID is the key directly (static scene data)
                    if key_to_node_id.contains_key(&child_uid) {
                        Some(child_uid)
                    } else {
                        // Second try: child ID is key+1 (from_nodes format)
                        let key_candidate = child_uid.wrapping_sub(1);
                        if key_to_node_id.contains_key(&key_candidate) {
                            Some(key_candidate)
                        } else {
                            None
                        }
                    }
                } else {
                    None
                };
                
                if let Some(child_key) = child_key_opt {
                    if let Some(&correct_node_id) = key_to_node_id.get(&child_key) {
                        node.add_child(correct_node_id);
                    }
                }
            }
        }
        
        Self {
            root_key,
            nodes,
            key_to_node_id,
        }
    }
    
    /// Convert SceneData to runtime Scene format
    /// Maps u32 scene keys to new NodeIDs and remaps parent references
    pub fn to_runtime_nodes(self) -> (NodeArena, NodeID) {
        use crate::uid32::NodeID;
        // Create new NodeIDs for runtime
        // Process root first to ensure it gets ID 1
        let mut old_to_new_id: HashMap<NodeID, NodeID> = HashMap::with_capacity(self.nodes.len());
        
        // Process root first to ensure it gets the first sequential ID
        let root_old_node_id = self.key_to_node_id[&self.root_key];
        let root_new_id = NodeID::new();
        old_to_new_id.insert(root_old_node_id, root_new_id);
        
        // Then process all other nodes
        for &key in self.nodes.keys() {
            if key == self.root_key {
                continue; // Already processed
            }
            let old_node_id = self.key_to_node_id[&key];
            let new_id = NodeID::new();
            old_to_new_id.insert(old_node_id, new_id);
            println!("  Scene key {}: old_node_id={} -> new_id={}", key, old_node_id, new_id);
        }
        
        let mut runtime_nodes = NodeArena::new();
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);
        
        // First pass: create nodes with new IDs and collect parent relationships
        for (key, mut node) in self.nodes {
            let old_node_id = self.key_to_node_id[&key];
            let new_id = old_to_new_id[&old_node_id];
            node.set_id(new_id);
            node.clear_children();
            
            // Remap parent ID
            if let Some(parent) = node.get_parent() {
                println!("  Node {} (key={}): has parent with id={}", node.get_name(), key, parent.id);
                if let Some(&new_parent_id) = old_to_new_id.get(&parent.id) {
                    // We'll set parent after we have all node types
                    parent_children.entry(new_parent_id).or_default().push(new_id);
                } else {
                    eprintln!("⚠️ WARNING: Parent ID {} not found in old_to_new_id for node {} (key={}, name={})", 
                        parent.id, new_id, key, node.get_name());
                    eprintln!("    Available old NodeIDs in mapping: {:?}", old_to_new_id.keys().collect::<Vec<_>>());
                }
            } else {
                println!("  Node {} (key={}): NO PARENT", node.get_name(), key);
            }
            
            runtime_nodes.insert(new_id, node);
        }
        
        // Second pass: set parent relationships with proper types
        for (parent_id, child_ids) in parent_children {
            if let Some(parent_node) = runtime_nodes.get(parent_id) {
                let parent_type_enum = parent_node.get_type();
                
                for child_id in child_ids {
                    if let Some(child) = runtime_nodes.get_mut(child_id) {
                    let parent_type = crate::nodes::node::ParentType::new(parent_id, parent_type_enum);
                    child.set_parent(Some(parent_type));
                    // Debug: verify parent was set
                    if child.get_parent().is_none() {
                        eprintln!("⚠️ WARNING: Failed to set parent on child {} -> parent {}", child_id, parent_id);
                    }
                }
                // Add to parent's children list
                if let Some(parent) = runtime_nodes.get_mut(parent_id) {
                    parent.add_child(child_id);
                }
            }
        }
        }
        
        // Get root ID
        let root_old_node_id = self.key_to_node_id[&self.root_key];
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
        // use UUIDs from key_to_uuid, and children are already set.
        // This function can be used to verify/rebuild relationships if needed.
        
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);

        // Collect parent node types
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_types: HashMap<NodeID, crate::node_registry::NodeType> = HashMap::with_capacity(0);
        
        for (&_key, node) in data.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a NodeID, find which scene key it corresponds to
                let parent_key_opt = data.key_to_node_id.iter()
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
                let parent_key_opt = data.key_to_node_id.iter()
                    .find(|&(_, &node_id)| node_id == parent_id)
                    .map(|(&key, _)| key);
                
                if let Some(_parent_key) = parent_key_opt {
                    let node_id = data.key_to_node_id[&key];
                    parent_children.entry(parent_id).or_default().push(node_id);
                }
            }
        }

        // Apply relationships
        for (parent_id, children) in parent_children {
            // Find parent node by NodeID
            let parent_key_opt = data.key_to_node_id.iter()
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
    queued_signals: Vec<(u64, SmallVec<[Value; 3]>)>,
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
    pub input_manager: Mutex<InputManager>,         // Keyboard/mouse input manager

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
    // Using HashSet for O(1) membership checks
    needs_rerender: HashSet<NodeID>,
}

#[derive(Default)]
pub struct SignalBus {
    // signal_id → { script_uuid → SmallVec<[u64; 4]> (function_ids) }
    pub connections: HashMap<u64, HashMap<NodeID, SmallVec<[u64; 4]>>>,
}

impl<P: ScriptProvider> Scene<P> {
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
    
    /// Create a runtime scene from a root node
    pub fn new(root: SceneNode, provider: P, project: Rc<RefCell<Project>>) -> Self {
        let root_id = root.get_id();
        let mut nodes = NodeArena::new();
        nodes.insert(root_id, root);
        
        Self {
            textures_converted: false,
            nodes,
            root_id,
            signals: SignalBus::default(),
            queued_signals: Vec::new(),
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
            println!("⚠️ PhysicsWorld2D is INITIALIZED (should be None for projects without physics)");
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
        let mut nodes = IndexMap::new();
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
        let root_key = node_id_to_key.get(&self.root_id)
            .copied()
            .unwrap_or(0);
        
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
                            parent.node_type
                        );
                        node.set_parent(Some(parent_type));
                    }
                }
            }
        }
        
        SceneData {
            root_key,
            nodes,
            key_to_node_id,
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
        // Create game root - it will get ID 1 from NodeID::new()
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


        // ✅ root script first
        let root_script_opt: Option<String> = {
            let proj_ref = game_scene.project.borrow();
            proj_ref.root_script().map(|s| s.to_string())
        };


        if let Some(root_script_path) = root_script_opt {
            if let Ok(identifier) = script_path_to_identifier(&root_script_path) {
                if let Ok(ctor) = game_scene.provider.load_ctor(&identifier) {
                    let root_id = game_scene.get_root().get_id();
                    let boxed = game_scene.instantiate_script(ctor, root_id);
                    let handle = Rc::new(UnsafeCell::new(boxed));
                    game_scene.scripts.insert(root_id, handle);

                    let project_ref = game_scene.project.clone();
                    let mut project_borrow = project_ref.borrow_mut();

                    let now = Instant::now();
                    let true_delta = match game_scene.last_scene_update {
                        Some(prev) => now.duration_since(prev).as_secs_f32(),
                        None => 0.0,
                    };

                    let mut api = ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow, gfx);
                    api.call_init(root_id);
                    
                    // After script initialization, ensure renderable nodes are marked for rerender
                    // (old system would have called mark_dirty() here)
                    if let Some(node) = game_scene.nodes.get(root_id) {
                        if node.is_renderable() {
                            game_scene.needs_rerender.insert(root_id);
                        }
                    }
                }
            }
        }


        // ✅ main scene second
        let main_scene_path: String = {
            let proj_ref = game_scene.project.borrow();
            let path = proj_ref.main_scene().to_string();
            path
        };

        // measure load
        let _t_load_start = Instant::now();
        let loaded_data = game_scene.provider.load_scene_data(&main_scene_path)?;
        let _load_time = _t_load_start.elapsed();

        // measure merge/graft
        let _t_graft_start = Instant::now();
        let game_root = game_scene.get_root().get_id();
        game_scene.merge_scene_data(loaded_data, game_root, gfx)?; // <- was graft_data()
        let _graft_time = _t_graft_start.elapsed();


        Ok(game_scene)
    }

    fn remap_node_ids_in_json_value(value: &mut serde_json::Value, id_map: &HashMap<NodeID, NodeID>) {
        match value {
            serde_json::Value::String(s) => {
                if let Ok(uid) = Uid32::parse_str(s) {
                    let old_node_id = NodeID::from_uid32(uid);
                    if let Some(&new_node_id) = id_map.get(&old_node_id) {
                        *s = new_node_id.as_uid32().to_string();
                    }
                }
            }
            serde_json::Value::Object(obj) => {
                if obj.len() > 1 {
                    let mut entries: Vec<_> = obj.iter_mut().collect();
                    entries.par_iter_mut().for_each(|(_, v)| {
                        Self::remap_node_ids_in_json_value(v, id_map);
                    });
                } else if let Some((_, v)) = obj.iter_mut().next() {
                    Self::remap_node_ids_in_json_value(v, id_map);
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.len() > 1 {
                    arr.par_iter_mut().for_each(|v| {
                        Self::remap_node_ids_in_json_value(v, id_map);
                    });
                } else if let Some(v) = arr.iter_mut().next() {
                    Self::remap_node_ids_in_json_value(v, id_map);
                }
            }
            _ => {}
        }
    }

    fn remap_script_exp_vars_node_ids(
        script_exp_vars: &mut HashMap<String, serde_json::Value>,
        id_map: &HashMap<NodeID, NodeID>,
    ) {
        if script_exp_vars.len() > 1 {
            let mut entries: Vec<_> = script_exp_vars.iter_mut().collect();
            entries.par_iter_mut().for_each(|(_, value)| {
                Self::remap_node_ids_in_json_value(value, id_map);
            });
        } else if let Some((_, value)) = script_exp_vars.iter_mut().next() {
            Self::remap_node_ids_in_json_value(value, id_map);
        }
    }
    pub fn merge_scene_data(
        &mut self,
        mut other: SceneData,
        parent_id: NodeID,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<()> {
        use std::time::Instant;
    
        let merge_start = Instant::now();
    
        // ───────────────────────────────────────────────
        // 1️⃣ BUILD SCENE KEY → NEW RUNTIME ID MAP
        // ───────────────────────────────────────────────
        let id_map_start = Instant::now();
        // Map from old NodeID (from key_to_node_id) to new runtime NodeID
        let mut old_node_id_to_new_node_id: HashMap<NodeID, NodeID> = HashMap::with_capacity(other.nodes.len() + 1);
        // Also build scene key -> new NodeID map for easier lookup
        use crate::uid32::NodeID;
        let mut key_to_new_id: HashMap<u32, NodeID> = HashMap::with_capacity(other.nodes.len() + 1);
    
        // Generate new NodeIDs for all nodes
        // Process root first to ensure it gets the next sequential ID
        let root_key = other.root_key;
        let root_old_node_id = other.key_to_node_id()[&root_key];
        let root_new_id = NodeID::new();
        old_node_id_to_new_node_id.insert(root_old_node_id, root_new_id);
        key_to_new_id.insert(root_key, root_new_id);
        
        // Then process all other nodes
        for &key in other.nodes.keys() {
            if key == root_key {
                continue; // Already processed
            }
            let old_node_id = other.key_to_node_id()[&key];
            let new_id = NodeID::new();
            old_node_id_to_new_node_id.insert(old_node_id, new_id);
            key_to_new_id.insert(key, new_id);
        }
    
        let id_map_time = id_map_start.elapsed();
    
        // ───────────────────────────────────────────────
        // 2️⃣ REMAP NODES AND BUILD RELATIONSHIPS
        // ───────────────────────────────────────────────
        let remap_start = Instant::now();
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);
    
        // Get the subscene root's NEW runtime ID
        let subscene_root_key = other.root_key;
        let subscene_root_new_id = key_to_new_id[&subscene_root_key];
    
        // Check if root has is_root_of (determines if we skip the root later)
        let root_is_root_of = other
            .nodes
            .get(&other.root_key)
            .and_then(|n| Self::get_is_root_of(n));
    
        let skip_root_id: Option<NodeID> = if root_is_root_of.is_some() {
            Some(subscene_root_new_id)
        } else {
            None
        };
    
        // First, collect parent node types from other.nodes before mutable iteration
        // Parent IDs in nodes are NodeIDs from key_to_node_id, so we need to map them
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut other_parent_types: HashMap<NodeID, crate::node_registry::NodeType> = HashMap::with_capacity(0);
        for (&_key, node) in other.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a NodeID from key_to_node_id, find which scene key it corresponds to
                // We need to reverse lookup: find scene key where key_to_node_id[key] == parent.id
                let parent_key_opt = other.key_to_node_id().iter()
                    .find(|&(_, &node_id)| node_id == parent.id)
                    .map(|(&key, _)| key);
                
                if let Some(parent_key) = parent_key_opt {
                    if let Some(parent_node) = other.nodes.get(&parent_key) {
                        other_parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }

        // Collect key_to_node_id mapping before mutable iteration
        let key_to_node_id_copy: HashMap<u32, NodeID> = other.key_to_node_id().iter().map(|(&k, &v)| (k, v)).collect();

        // Also create NodeID->NodeID mapping for script_exp_vars remapping
        let mut old_node_id_to_new_node_id_for_scripts: HashMap<NodeID, NodeID> = HashMap::with_capacity(other.nodes.len());
        for (&old_node_id, &new_node_id) in &old_node_id_to_new_node_id {
            old_node_id_to_new_node_id_for_scripts.insert(old_node_id, new_node_id);
        }

        // Remap all nodes
        for (key, node) in other.nodes.iter_mut() {
            let new_id = key_to_new_id[key];
            node.set_id(new_id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Determine parent relationship
            if let Some(parent) = node.get_parent() {
                let parent_old_node_id = parent.id;
                
                
                // Check if parent is in the subscene (remap old NodeID to new NodeID)
                // Find which scene key corresponds to parent_old_node_id, then get the new NodeID
                let mapped_parent_id_opt = key_to_node_id_copy.iter()
                    .find(|&(_, &node_id)| node_id == parent_old_node_id)
                    .and_then(|(&key, _)| key_to_new_id.get(&key).copied());
                
                if let Some(mapped_parent_id) = mapped_parent_id_opt {
                    // Parent is in subscene - use mapped runtime NodeID
                    // Get parent type from other_parent_types (collected earlier) or from already-inserted nodes
                    let parent_type_enum = if let Some(&parent_type) = other_parent_types.get(&parent.id) {
                        parent_type
                    } else if let Some(parent_node) = self.nodes.get(mapped_parent_id) {
                        parent_node.get_type()
                    } else {
                        // Fallback - shouldn't happen
                        crate::node_registry::NodeType::Node
                    };
                    let parent_type = crate::nodes::node::ParentType::new(mapped_parent_id, parent_type_enum);
                    node.set_parent(Some(parent_type));
                    parent_children
                        .entry(mapped_parent_id)
                        .or_default()
                        .push(new_id);
                } else {
                    // Parent not in subscene - check if it exists in main scene
                    // parent_old_node_id is from other's key_to_node_id, so we need to find which scene key it corresponds to
                    // and then check if that scene key's NodeID exists in self.nodes
                    let _parent_key_opt = key_to_node_id_copy.iter()
                        .find(|&(_, &node_id)| node_id == parent_old_node_id)
                        .map(|(&key, _)| key);
                    
                    // For now, don't set parent - this is an invalid reference
                }
            } else if new_id == subscene_root_new_id {
                // This is the subscene root with no parent - attach to game's parent_id
                // But only if we're NOT skipping it (is_root_of case)
                if skip_root_id.is_none() {
                    // Create ParentType with the parent's type
                    if let Some(parent_node) = self.nodes.get(parent_id) {
                        let parent_type = crate::nodes::node::ParentType::new(parent_id, parent_node.get_type());
                        node.set_parent(Some(parent_type));
                    }
                    parent_children.entry(parent_id).or_default().push(new_id);
                }
            }
            // else: node has no parent and isn't root - leave as orphan (shouldn't happen normally)
    
            // Handle script_exp_vars - remap NodeIDs using NodeID->NodeID mapping
            if let Some(mut script_vars) = node.get_script_exp_vars() {
                Self::remap_script_exp_vars_node_ids(&mut script_vars, &old_node_id_to_new_node_id_for_scripts);
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
            for node in other.nodes.values_mut() {
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
    
        // ───────────────────────────────────────────────
        // 3️⃣ INSERT NODES INTO MAIN SCENE
        // ───────────────────────────────────────────────
        let insert_start = Instant::now();
        self.nodes.reserve(other.nodes.len() + 1);
    
        let mut inserted_ids: Vec<NodeID> = Vec::with_capacity(other.nodes.len());
    
        for mut node in other.nodes.into_values() {
            let node_id = node.get_id();

            // Skip root if it has is_root_of (will be replaced by nested scene content)
            if let Some(skip_id) = skip_root_id {
                if node_id == skip_id {
                    continue;
                }
            }

            node.mark_transform_dirty_if_node2d();

            // Resolve name conflicts (check siblings AND parent/ancestor)
            let node_name = node.get_name();
            let parent_id_opt = node.get_parent().map(|p| p.id);
            
            // Check if name conflicts with siblings OR with parent/ancestors
            let has_sibling_conflict = parent_id_opt.map(|pid| self.has_sibling_name_conflict(pid, node_name)).unwrap_or(false);
            let has_parent_conflict = self.has_parent_or_ancestor_name_conflict(parent_id_opt, node_name);
            
            if has_sibling_conflict || has_parent_conflict {
                let resolved_name = parent_id_opt.map(|pid| self.resolve_name_conflict(pid, node_name)).unwrap_or_else(|| node_name.to_string());
                Self::set_node_name(&mut node, resolved_name);
            }

            self.nodes.insert(node_id, node);
            inserted_ids.push(node_id);
            
            // Add to needs_rerender set only if this is a renderable node
            if let Some(node_ref) = self.nodes.get(node_id) {
                if node_ref.is_renderable() {
                    self.needs_rerender.insert(node_id);
                }
            }
    
            // Register node for internal fixed updates if needed
            if let Some(node_ref) = self.nodes.get(node_id) {
                if node_ref.needs_internal_fixed_update() {
                    // Optimize: HashSet insert is O(1) and handles duplicates automatically
                    self.nodes_with_internal_fixed_update.insert(node_id);
                }
                // Register node for internal render updates if needed
                if node_ref.needs_internal_render_update() {
                    // Optimize: HashSet insert is O(1) and handles duplicates automatically
                    self.nodes_with_internal_render_update.insert(node_id);
                }
            }
        }
    
        // Update the GAME's parent node to include new children
        if let Some(children_of_game_parent) = parent_children.get(&parent_id) {
            if let Some(game_parent) = self.nodes.get_mut(parent_id) {
                for child_id in children_of_game_parent {
                    if !game_parent.get_children().contains(child_id) {
                        game_parent.add_child(*child_id);
                    }
                }
            }
        }
    
        // Mark transforms dirty for all inserted nodes
        for id in &inserted_ids {
            self.mark_transform_dirty_recursive(*id);
        }
    
        let insert_time = insert_start.elapsed();
    
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
    
        // Recursively load and merge nested scenes
        let nested_scene_count = nodes_with_nested_scenes.len();
        for (parent_node_id, scene_path) in nodes_with_nested_scenes {
    
            // Load the nested scene
            if let Ok(nested_scene_data) = self.provider.load_scene_data(&scene_path) {
                // Merge with the node as parent - nested scene's root becomes child of this node
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
    
        let _nested_scene_time = nested_scene_start.elapsed();
        let _nested_scene_count = nested_scene_count;
    
        // ───────────────────────────────────────────────
        // 5️⃣ REGISTER COLLISION SHAPES WITH PHYSICS
        // ───────────────────────────────────────────────
        let physics_start = Instant::now();
        self.register_collision_shapes(&inserted_ids);
        let physics_time = physics_start.elapsed();
    
        // ───────────────────────────────────────────────
        // 6️⃣ FUR LOADING (UI FILES)
        // ───────────────────────────────────────────────
        let fur_start = Instant::now();
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
                        },
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
    
        let script_time = script_start.elapsed();
        let total_time = merge_start.elapsed();
    
        // ───────────────────────────────────────────────
        // 8️⃣ PERFORMANCE SUMMARY
        // ───────────────────────────────────────────────
    
    
        // Print scene tree for debugging
    
        Ok(())
    }
    
    /// Merge a nested scene where the nested scene's root REPLACES an existing node
    /// (used for is_root_of scenarios)
    fn merge_scene_data_with_root_replacement(
        &mut self,
        mut other: SceneData,
        replacement_root_id: NodeID,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<()> {
        use crate::uid32::NodeID;
    
        // Build mapping from old NodeID (from key_to_node_id) to new runtime NodeID
        let mut old_node_id_to_new_id: HashMap<NodeID, NodeID> = HashMap::with_capacity(other.nodes.len());
        let mut key_to_new_id: HashMap<u32, NodeID> = HashMap::with_capacity(other.nodes.len());
    
        // Generate NodeIDs for all nodes EXCEPT the root
        let subscene_root_key = other.root_key;
    
        for &key in other.nodes.keys() {
            let old_node_id = other.key_to_node_id()[&key];
            if key == subscene_root_key {
                // Root maps to the replacement node (which already exists)
                old_node_id_to_new_id.insert(old_node_id, replacement_root_id);
                key_to_new_id.insert(key, replacement_root_id);
            } else {
                let new_id = NodeID::new();
                old_node_id_to_new_id.insert(old_node_id, new_id);
                key_to_new_id.insert(key, new_id);
            }
        }
    
    
        // Build parent-children relationships
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<NodeID, Vec<NodeID>> = HashMap::with_capacity(0);
        
        // First, collect parent node types from other.nodes before mutable iteration
        // Parent IDs in nodes are NodeIDs from key_to_node_id, so we need to map them
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut other_parent_types: HashMap<NodeID, crate::node_registry::NodeType> = HashMap::with_capacity(0);
        for (&_key, node) in other.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a NodeID, find which scene key it corresponds to
                let parent_key_opt = other.key_to_node_id().iter()
                    .find(|&(_, &node_id)| node_id == parent.id)
                    .map(|(&key, _)| key);
                
                if let Some(parent_key) = parent_key_opt {
                    if let Some(parent_node) = other.nodes.get(&parent_key) {
                        other_parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }
    
        // Also create NodeID->NodeID mapping for script_exp_vars remapping
        let mut old_node_id_to_new_node_id_for_scripts: HashMap<NodeID, NodeID> = HashMap::with_capacity(other.nodes.len());
        for (&old_node_id, &new_id) in &old_node_id_to_new_id {
            old_node_id_to_new_node_id_for_scripts.insert(old_node_id, new_id);
        }

        // Remap all nodes
        for (key, node) in other.nodes.iter_mut() {
            let new_id = key_to_new_id[key];
            node.set_id(new_id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Remap parent using old_node_id_to_new_id (like in merge_scene_data)
            if let Some(parent) = node.get_parent() {
                let parent_old_node_id = parent.id;
                if let Some(&mapped_parent_id) = old_node_id_to_new_id.get(&parent_old_node_id) {
                    // Parent is in subscene - use mapped runtime NodeID
                    // Get parent type from other_parent_types (collected earlier) or from already-inserted nodes
                    let parent_type_enum = if let Some(&parent_type) = other_parent_types.get(&parent.id) {
                        parent_type
                    } else if let Some(parent_node) = self.nodes.get(mapped_parent_id) {
                        parent_node.get_type()
                    } else {
                        // Fallback - shouldn't happen
                        crate::node_registry::NodeType::Node
                    };
                    let parent_type = crate::nodes::node::ParentType::new(mapped_parent_id, parent_type_enum);
                    node.set_parent(Some(parent_type));
                    parent_children
                        .entry(mapped_parent_id)
                        .or_default()
                        .push(new_id);
                } else {
                    // Parent not in subscene - check if it exists in main scene
                    if let Some(parent_node) = self.nodes.get(parent_old_node_id) {
                        // Parent exists in main scene - use its runtime ID
                        let parent_runtime_id = parent_node.get_id();
                        let parent_type = crate::nodes::node::ParentType::new(parent_runtime_id, parent_node.get_type());
                        node.set_parent(Some(parent_type));
                        parent_children.entry(parent_runtime_id).or_default().push(new_id);
                    }
                }
            }
            // If no parent (this is the subscene root), its parent is already set
            // in the main scene (the node with is_root_of)

            // Remap script_exp_vars using NodeID->NodeID mapping
            if let Some(mut script_vars) = node.get_script_exp_vars() {
                Self::remap_script_exp_vars_node_ids(&mut script_vars, &old_node_id_to_new_node_id_for_scripts);
                node.set_script_exp_vars(Some(script_vars));
            }
        }
    
        // Apply parent-child relationships within the subscene
        for (parent_new_id, children) in &parent_children {
            // If parent is the replacement root, update the existing node in main scene
            if *parent_new_id == replacement_root_id {
                if let Some(existing_node) = self.nodes.get_mut(replacement_root_id) {
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
        let mut inserted_ids: Vec<NodeID> = Vec::new();
    
        for mut node in other.nodes.into_values() {
            let node_id = node.get_id();
    
            // Skip the root - it already exists in the main scene
            if node_id == replacement_root_id {
                continue;
            }
    
            node.mark_transform_dirty_if_node2d();
    
            // Resolve name conflicts (only check siblings - nodes with the same parent)
            let node_name = node.get_name();
            let parent_id_opt = node.get_parent().map(|p| p.id);
            if let Some(pid) = parent_id_opt {
                if self.has_sibling_name_conflict(pid, node_name) {
                    let resolved_name = self.resolve_name_conflict(pid, node_name);
                    Self::set_node_name(&mut node, resolved_name);
                }
            }

            self.nodes.insert(node_id, node);
            inserted_ids.push(node_id);
            
            // Add to needs_rerender set only if this is a renderable node
            if let Some(node_ref) = self.nodes.get(node_id) {
                if node_ref.is_renderable() {
                    self.needs_rerender.insert(node_id);
                }
            }
    
            // Register for internal fixed updates if needed
            if let Some(node_ref) = self.nodes.get(node_id) {
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
                        if flags.has_fixed_update() && !self.scripts_with_fixed_update.contains(&id) {
                            self.scripts_with_fixed_update.push(id);
                        }
                        
                        self.scripts.insert(id, handle);
                        self.scripts_dirty = true;
    
                        let mut api = ScriptApi::new(dt, self, &mut *project_borrow, gfx);
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
                if let Err(e) =
                    self.merge_scene_data_with_root_replacement(nested_scene_data, parent_node_id, gfx)
                {
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
        let root_nodes: Vec<NodeID> = self.nodes.iter()
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
            let parent_info = node.get_parent()
                .map(|p| format!("parent={} ({:?})", p.id, p.node_type))
                .unwrap_or_else(|| "ROOT".to_string());
            let has_script = self.scripts.contains_key(&node_id);
            let script_status = if has_script { "✓SCRIPT" } else { "✗NO_SCRIPT" };
            
            // Print this node with detailed debug info
            println!("{}{} [id={}] [type={:?}] [{}] [{}] {}",
                prefix, 
                name,
                node_id,
                node_type,
                parent_info,
                script_status,
                script_path.map(|p| format!("script={}", p)).unwrap_or_default(),
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



    /// Check if a node name conflicts with any sibling (node with the same parent)
    fn has_sibling_name_conflict(&self, parent_id: NodeID, name: &str) -> bool {
        self.nodes.values().any(|n| {
            n.get_parent().map(|p| p.id) == Some(parent_id) && n.get_name() == name
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
        while self.has_sibling_name_conflict(parent_id, &candidate) 
            || self.has_parent_or_ancestor_name_conflict(Some(parent_id), &candidate) {
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
                    let _span = tracing::span!(tracing::Level::INFO, "update_collider_transforms").entered();
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
                    let _span = tracing::span!(tracing::Level::INFO, "script_fixed_updates").entered();
                    
                    // OPTIMIZED: Rebuild cached_script_ids when dirty (update/fixed_update vectors are maintained incrementally)
                    if self.scripts_dirty {
                        self.cached_script_ids.clear();
                        self.cached_script_ids.extend(self.scripts.keys().copied());
                        self.scripts_dirty = false;
                    }
                    
                    // OPTIMIZED: Use pre-filtered vector of scripts with fixed_update
                    // Collect script IDs to avoid borrow checker issues
                    let script_ids: Vec<NodeID> = self.scripts_with_fixed_update.iter().copied().collect();
                    
                    // Clone project reference before loop to avoid borrow conflicts
                    let project_ref = self.project.clone();
                    for id in script_ids {
                        #[cfg(feature = "profiling")]
                        let _span = tracing::span!(tracing::Level::INFO, "script_fixed_update", id = %id).entered();
                        
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
                    let _span = tracing::span!(tracing::Level::INFO, "node_internal_fixed_updates").entered();
                    
                    // Optimize: collect first to avoid borrow checker issues (HashSet iteration order is non-deterministic but that's fine)
                    let node_ids: Vec<NodeID> = self.nodes_with_internal_fixed_update.iter().copied().collect();
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
            // OPTIMIZED: Early exit check before allocating Vec (most common case in empty projects)
            if self.scripts_with_update.is_empty() {
                self.process_queued_signals();
                return;
            }
            
            // OPTIMIZED: Use pre-filtered vector of scripts with update
            // Collect script IDs to avoid borrow checker issues
            // OPTIMIZED: Pre-allocate with known capacity (iter().copied() already does this efficiently)
            let script_ids: Vec<NodeID> = self.scripts_with_update.iter().copied().collect();
            
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
                let mut api = ScriptApi::new(true_delta, self, &mut *project_borrow, gfx);
                api.call_update(id);
            }
        }

        // Global transforms are now calculated lazily when needed (in traverse_and_render)

        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "process_queued_signals").entered();
            self.process_queued_signals();
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
            let _span = tracing::span!(tracing::Level::INFO, "node_internal_render_updates").entered();
            
            // Optimize: collect first to avoid borrow checker issues (HashSet iteration order is non-deterministic but that's fine)
            let node_ids: Vec<NodeID> = self.nodes_with_internal_render_update.iter().copied().collect();
            
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
            let _span = tracing::span!(tracing::Level::INFO, "get_nodes_needing_rerender").entered();
            self.get_nodes_needing_rerender()
        };
        if !nodes_needing_rerender.is_empty() {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "traverse_and_render", count = nodes_needing_rerender.len()).entered();
            self.traverse_and_render(nodes_needing_rerender, gfx);
        }
    }

    fn connect_signal(&mut self, signal: u64, target_id: NodeID, function_id: u64) {

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
    fn emit_signal_id_deferred(&mut self, signal: u64, params: &[Value]) {
        // Convert slice to SmallVec for stack-allocated storage (≤3 params = no heap allocation)
        let mut smallvec = SmallVec::new();
        smallvec.extend(params.iter().cloned());
        self.queued_signals.push((signal, smallvec));
    }

    // ✅ OPTIMIZED: Use drain() to collect, then process (avoids borrow checker issues)
    fn process_queued_signals(&mut self) {
        // OPTIMIZED: Early exit before allocation (most common case)
        if self.queued_signals.is_empty() {
            return;
        }

        // OPTIMIZED: drain() already pre-allocates efficiently
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
    fn emit_signal_impl(&mut self, signal: u64, _params: &[Value]) {
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
        let mut call_list = SmallVec::<[(NodeID, u64); 4]>::new();
        for (node_id, fns) in script_map.iter() {
            for &fn_id in fns.iter() {
                call_list.push((*node_id, fn_id));
            }
        }

        let _setup_time = start_time.elapsed();
        
        // Now all borrows of self.signals are dropped ✅
        let now = Instant::now();
        let _true_delta = self
            .last_scene_update
            .map(|prev| now.duration_since(prev).as_secs_f32())
            .unwrap_or(0.0);

        let project_ref = self.project.clone();
        let _project_borrow = project_ref.borrow_mut();

        // Note: Signals are called from ScriptApi which has Graphics, but emit_signal_impl
        // doesn't have access to it. For now, we'll skip signal emission here since
        // signals should be emitted through ScriptApi which has Graphics.
        // This is a temporary workaround - signals should be refactored to pass Graphics through.
        // The actual signal emission happens in ScriptApi::emit_signal_id which has Graphics.
        return;
    }

    pub fn add_node_to_scene(&mut self, mut node: SceneNode, gfx: &mut crate::rendering::Graphics) -> anyhow::Result<()> {
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
        self.nodes.insert(id, node);
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
        let script_path_opt = self.nodes.get(id)
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
            api.call_init(id);
            
            // After script initialization, ensure renderable nodes are marked for rerender
            // (old system would have called mark_dirty() here)
            if let Some(node) = self.nodes.get(id) {
                if node.is_renderable() {
                    self.needs_rerender.insert(id);
                }
            }

        }

        Ok(())
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
            self.scripts_with_update.retain(|&script_id| script_id != node_id);
            self.scripts_with_fixed_update.retain(|&script_id| script_id != node_id);
            self.scripts_dirty = true;
        }
        
        // Clean up signal connections - remove this node from all signal connection maps
        // This prevents deferred signals or later emissions from trying to call handlers on a deleted node
        for (_signal_id, script_map) in self.signals.connections.iter_mut() {
            script_map.remove(&node_id);
        }
        
        // Also clean up empty signal entries to avoid memory leaks
        self.signals.connections.retain(|_, script_map| !script_map.is_empty());
        
        // Remove from needs_rerender set (if it was there)
        self.needs_rerender.remove(&node_id);
        
        // IMPORTANT: Clean up Area2D's previous_collisions tracking when a node is removed
        // This prevents Area2D from trying to emit signals for nodes that no longer exist
        // Iterate through all Area2D nodes and remove the deleted node from their previous_collisions
        for (area_id, area_node) in self.nodes.iter_mut() {
            if let SceneNode::Area2D(area) = area_node {
                area.previous_collisions.remove(&node_id);
            }
        }
    }

    /// Get the global transform for a node, calculating it lazily if dirty
    /// This recursively traverses up the parent chain until it finds a clean transform
    pub fn get_global_transform(&mut self, node_id: NodeID) -> Option<crate::structs2d::Transform2D> {
        // OPTIMIZED: Reduced hashmap lookups by collecting all needed data in single pass
        // Build chain from node to root, then calculate transforms top-down
        
        // First check if already cached (single lookup)
        let (is_node2d, is_cached, cached_transform) = {
            if let Some(node) = self.nodes.get(node_id) {
                if let Some(node2d) = node.as_node2d() {
                    if !node2d.transform_dirty {
                        return Some(node2d.global_transform);
                    }
                    (true, false, crate::structs2d::Transform2D::default())
                } else {
                    return None; // Not a Node2D node
                }
            } else {
                return None;
            }
        };
        
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
        let parent_global = self.get_global_transform(parent_id)
            .unwrap_or_else(|| crate::structs2d::Transform2D::default());
        
        // OPTIMIZED: Fast path for identity parent (common case - static nodes)
        if parent_global.is_default() {
            // Just copy local transforms directly, no calculation needed
            // OPTIMIZED: Get local transform first (immutable borrow), then update (mutable borrow)
            for &child_id in child_ids {
                // Get local transform first (immutable borrow)
                let local = self.nodes.get(child_id)
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
        let globals = crate::structs2d::Transform2D::batch_calculate_global(&parent_global, &local_transforms);
        
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


    /// Set the global transform for a node (marks it as dirty)
    pub fn set_global_transform(&mut self, node_id: NodeID, transform: crate::structs2d::Transform2D) -> Option<()> {
        if let Some(node) = self.nodes.get_mut(node_id) {
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
    /// Also marks nodes as needing rerender so they get picked up by get_nodes_needing_rerender()
    /// OPTIMIZED: Uses iterative work queue instead of recursion for better performance and cache locality
    pub fn mark_transform_dirty_recursive(&mut self, node_id: NodeID) {
        // Check if node exists before processing (might have been deleted)
        if !self.nodes.contains_key(node_id) {
            eprintln!("⚠️ mark_transform_dirty_recursive: Node {} does not exist, skipping", node_id);
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
            let (node2d_child_ids, needs_cache_update): (Vec<NodeID>, bool) = {
                // Check if we have a cached list of Node2D children
                let cached = self.nodes
                    .get(current_id)
                    .and_then(|node| node.as_node2d())
                    .and_then(|n2d| n2d.node2d_children_cache.as_ref());
                
                if let Some(cached_ids) = cached {
                    // Use cache - but filter out any stale references to deleted nodes
                    let filtered: Vec<NodeID> = cached_ids.iter()
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
                    let child_ids: Vec<NodeID> = self.nodes
                        .get(current_id)
                        .map(|node| node.get_children().iter().copied().collect())
                        .unwrap_or_default();
                    
                    // Filter to only Node2D children and cache the result
                    let node2d_ids: Vec<NodeID> = child_ids.iter()
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
            
            // Step 2: Mark this node as dirty and update cache if needed (mutable borrow)
            let is_renderable = {
                let node = self.nodes.get_mut(current_id).unwrap();
                
                // Mark transform as dirty if it's a Node2D-based node
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.transform_dirty = true;
                    
                    // Update cache if we filtered out stale entries
                    if needs_cache_update {
                        node2d.node2d_children_cache = Some(node2d_child_ids.clone());
                    }
                }
                
                // Check if renderable (before dropping mutable borrow)
                node.is_renderable()
            };
            
            // Step 3: Add to needs_rerender if renderable
            if is_renderable && !self.needs_rerender.contains(&current_id) {
                self.needs_rerender.insert(current_id);
            }
            
            // Step 4: Add Node2D children to work queue (process them iteratively)
            // OPTIMIZED: Add to front of queue so we process depth-first (better cache locality)
            // This ensures we process a subtree completely before moving to siblings
            for child_id in node2d_child_ids.into_iter().rev() {
                if self.nodes.contains_key(child_id) {
                    work_queue.push(child_id);
                }
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
                    // Only add if not already present (avoid duplicates)
                    if !cache.contains(&child_id) {
                        cache.push(child_id);
                    }
                } else {
                    // Cache doesn't exist yet - invalidate so it rebuilds on next use
                    // This ensures cache is rebuilt with all current children
                    node2d.node2d_children_cache = None;
                }
            }
        }
    }
    
    /// Update the Node2D children cache for a parent node when a child is removed
    fn update_node2d_children_cache_on_remove(&mut self, parent_id: NodeID, child_id: NodeID) {
        if let Some(parent_node) = self.nodes.get_mut(parent_id) {
            if let Some(node2d) = parent_node.as_node2d_mut() {
                // Remove from cache if it exists
                if let Some(ref mut cache) = node2d.node2d_children_cache {
                    cache.retain(|&id| id != child_id);
                }
                // If cache doesn't exist, that's fine - it will be rebuilt on next use
            }
        }
        // If parent doesn't exist, that's also fine - node was probably deleted
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
        let mut to_register: Vec<(NodeID, crate::structs2d::Shape2D, Option<crate::nodes::node::ParentType>)> = Vec::new();
        
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
                global_transforms.insert(*node_id, ([global.position.x, global.position.y], global.rotation));
            }
        }
        
        // Now register with physics (after releasing all node borrows)
        // OPTIMIZED: Lazy initialization - create physics world when first collision shape is registered
        if to_register.is_empty() {
            return;
        }
        
        // First, check which parents are Area2D nodes (before borrowing physics)
        // Store tuples of (node_id, shape, parent_opt, is_area2d_parent)
        let mut registration_data: Vec<(NodeID, crate::structs2d::Shape2D, Option<NodeID>, bool)> = Vec::new();
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
        let mut handles_to_store: Vec<(NodeID, rapier2d::prelude::ColliderHandle, Option<NodeID>)> = Vec::new();
        
        for (node_id, shape, parent_id_opt, is_area2d_parent) in registration_data {
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
        let target_node = Uid32::parse_str("d36d3c7f-7c49-497e-b5b2-8770e4e6d633").ok().map(NodeID::from_uid32);
        let is_target = target_node.map(|t| node_id == t).unwrap_or(false);
        
        if is_target {
            println!("🔍 [STOP_RENDER] stop_rendering_recursive called for {}", node_id);
        }
        
        // Check if node exists before accessing (might have been deleted)
        if let Some(node) = self.nodes.get(node_id) {
            // Stop rendering this node itself
            gfx.stop_rendering(node_id);

            // If it's a UI node, stop rendering all of its UI elements
            if let SceneNode::UINode(ui_node) = node {
                if let Some(elements) = &ui_node.elements {
                    for (element_id, _) in elements {
                        // UIElementID needs to be converted to NodeID for stop_rendering
                        // Note: stop_rendering expects NodeID, so we convert UIElementID -> Uid32 -> NodeID
                        gfx.stop_rendering(NodeID::from_uid32(element_id.as_uid32()));
                    }
                }
            }

            // Recursively stop rendering children (only if they still exist)
            // Collect children first to avoid borrowing issues
            let child_ids: Vec<NodeID> = node.get_children().iter().copied().collect();
            if is_target {
                println!("🔍 [STOP_RENDER] Node {} has children: {:?}", node_id, child_ids);
            }
            for child_id in child_ids {
                // Only recurse if child still exists (might have been deleted)
                if self.nodes.contains_key(child_id) {
                    self.stop_rendering_recursive(child_id, gfx);
                } else {
                    let is_target_child = target_node.map(|t| child_id == t).unwrap_or(false);
                    if is_target_child {
                        eprintln!("⚠️ [STOP_RENDER] Child {} does NOT exist but was in children list of {}", child_id, node_id);
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
    // OPTIMIZED: Returns pre-accumulated set instead of iterating over all nodes
    fn get_nodes_needing_rerender(&mut self) -> Vec<NodeID> {
        // Take ownership of the HashSet and convert to Vec
        self.needs_rerender.drain().collect()
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
                path: String,
                transform: Transform3D,
                material_path: Option<String>,
            },
            Light(crate::renderer_3d::LightUniform),
        }
        
        // OPTIMIZED: Pre-calculate transforms in dependency order to avoid redundant parent recalculations
        // When many children share the same moving parent, this ensures the parent's transform
        // is calculated once and cached before all children use it.
        self.precalculate_transforms_in_dependency_order(&nodes_needing_rerender);
        
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
                        // Read cached transform directly (read-only) instead of calling get_global_transform
                        let global_transform_opt = if needs_transform {
                            node.as_node2d()
                                .filter(|n2d| !n2d.transform_dirty) // Only use if not dirty (should be clean after precalc)
                                .map(|n2d| n2d.global_transform)
                        } else {
                            None
                        };
                        Some((node_id, timestamp, needs_transform, node_type, global_transform_opt))
                    } else {
                        None
                    }
                })
                .collect();
            
            // Step 2: Process nodes in parallel to build render commands
            // We'll defer texture path resolution until after parallel processing
            let render_data: Vec<_> = node_data
                .par_iter()
                .filter_map(|(node_id, timestamp, _needs_transform, _node_type, global_transform_opt)| {
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
                                    if let Some(path) = &mesh.mesh_path {
                                        return Some((
                                            *node_id,
                                            *timestamp,
                                            RenderCommand::Mesh {
                                                path: path.to_string(),
                                                transform: mesh.transform,
                                                material_path: mesh.material_path.as_ref().map(|p| p.to_string()),
                                            },
                                        ));
                                    }
                                }
                            }
                            SceneNode::OmniLight3D(light) => {
                                return Some((
                                    *node_id,
                                    *timestamp,
                                    RenderCommand::Light(crate::renderer_3d::LightUniform {
                                        position: light.transform.position.to_array(),
                                        color: light.color.to_array(),
                                        intensity: light.intensity,
                                        ambient: [0.05, 0.05, 0.05],
                                        ..Default::default()
                                    }),
                                ));
                            }
                            SceneNode::DirectionalLight3D(light) => {
                                let dir = light.transform.forward();
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
                                let dir = light.transform.forward();
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
            
            // Step 3: Separate render commands by type and resolve texture paths
            let mut rect_commands = Vec::new();
            let mut texture_commands = Vec::new();
            let mut ui_nodes = Vec::new();
            let mut camera_2d_updates = Vec::new();
            let mut camera_3d_updates = Vec::new();
            let mut mesh_commands = Vec::new();
            let mut light_commands = Vec::new();
            
            for (node_id, timestamp, cmd) in render_data {
                match cmd {
                    RenderCommand::Texture { texture_id, texture_path, global_transform, pivot, z_index } => {
                        // Resolve texture path (needs gfx access, so do it sequentially)
                        let texture_path_opt: Option<String> = if let Some(texture_id) = texture_id {
                            gfx.texture_manager.get_texture_path_from_id(&texture_id).map(|s| s.to_string())
                        } else {
                            texture_path.map(|p| p.to_string())
                        };
                        
                        if let Some(tex_path) = texture_path_opt {
                            texture_commands.push((
                                node_id,
                                tex_path,
                                global_transform,
                                pivot,
                                z_index,
                                timestamp,
                            ));
                        }
                    }
                    RenderCommand::Rect { transform, size, pivot, color, corner_radius, border_thickness, is_border, z_index } => {
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
                    RenderCommand::Mesh { path, transform, material_path } => {
                        mesh_commands.push((
                            node_id,
                            path,
                            transform,
                            material_path,
                        ));
                    }
                    RenderCommand::Light(light_uniform) => {
                        light_commands.push((
                            node_id,
                            light_uniform,
                        ));
                    }
                }
            }
            
            // Step 4: Clear transform_dirty flags sequentially (needs mutable access)
            for &node_id in &nodes_needing_rerender {
                if let Some(node) = self.nodes.get_mut(node_id) {
                    if let Some(node2d) = node.as_node2d_mut() {
                        node2d.transform_dirty = false;
                    }
                }
            }
            
            // OPTIMIZED: Batch queue operations
            // Queue all rects
            for (node_id, transform, size, pivot, color, corner_radius, border_thickness, is_border, z_index, timestamp) in rect_commands {
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
            
            // Queue all textures
            for (node_id, tex_path, global_transform, pivot, z_index, timestamp) in texture_commands {
                gfx.renderer_2d.queue_texture(
                    &mut gfx.renderer_prim,
                    &mut gfx.texture_manager,
                    &gfx.device,
                    &gfx.queue,
                    node_id,
                    &tex_path,
                    global_transform,
                    pivot,
                    z_index,
                    timestamp,
                );
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
            
            // Queue meshes
            for (node_id, path, transform, material_path) in mesh_commands {
                gfx.renderer_3d.queue_mesh(
                    node_id,
                    &path,
                    transform,
                    material_path.as_deref(),
                    &mut gfx.mesh_manager,
                    &mut gfx.material_manager,
                    &mut gfx.device,
                    &mut gfx.queue,
                );
            }
            
            // Queue lights
            for (node_id, light_uniform) in light_commands {
                gfx.renderer_3d.queue_light(node_id, light_uniform);
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
                
                if let Some(node) = self.nodes.get_mut(node_id) {
                    let timestamp = node.get_created_timestamp();
                    match node {
                        SceneNode::Sprite2D(sprite) => {
                            if sprite.visible {
                                let texture_path_opt: Option<String> = if let Some(texture_id) = &sprite.texture_id {
                                    gfx.texture_manager.get_texture_path_from_id(texture_id).map(|s| s.to_string())
                                } else {
                                    sprite.texture_path.as_ref().map(|p| p.to_string())
                                };
                                
                                if let Some(tex_path) = texture_path_opt {
                                    if let Some(global_transform) = global_transform_opt {
                                        gfx.renderer_2d.queue_texture(
                                            &mut gfx.renderer_prim,
                                            &mut gfx.texture_manager,
                                            &gfx.device,
                                            &gfx.queue,
                                            node_id,
                                            &tex_path,
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
                                        let color = shape.color.unwrap_or(crate::Color::new(255, 255, 255, 200));
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
                    if let Some(node2d) = node.as_node2d_mut() {
                        node2d.transform_dirty = false;
                    }
                }
            }
        }
    }
}

//
// ---------------- SceneAccess impl ----------------
//

impl<P: ScriptProvider> SceneAccess for Scene<P> {
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

    fn instantiate_script(
        &mut self,
        ctor: CreateFn,
        node_id: NodeID,
    ) -> Box<dyn ScriptObject> {
        // Trait requires Box, but we wrap it in Rc<RefCell<>> when inserting into scripts HashMap
        let raw = ctor();
        let mut boxed: Box<dyn ScriptObject> = unsafe { Box::from_raw(raw) };
        boxed.set_id(node_id);
        boxed
    }

    fn add_node_to_scene(&mut self, node: SceneNode, gfx: &mut crate::rendering::Graphics) -> anyhow::Result<()> {
        self.add_node_to_scene(node, gfx)
    }


    fn connect_signal_id(&mut self, signal: u64, target_id: NodeID, function: u64) {
        self.connect_signal(signal, target_id, function);
    }

    fn get_signal_connections(&self, signal: u64) -> Option<&HashMap<NodeID, SmallVec<[u64; 4]>>> {
        self.signals.connections.get(&signal)
    }

    fn emit_signal_id(&mut self, signal: u64, params: &[Value]) {
        self.emit_signal_id(signal, params);
    }

    fn emit_signal_id_deferred(&mut self, signal: u64, params: &[Value]) {
        self.emit_signal_id_deferred(signal, params);
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
        if !self.controller_enabled.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        // OPTIMIZED: Lazy initialization - create on first access using OnceCell
        self.controller_manager.get_or_init(|| Mutex::new(ControllerManager::new()));
        self.controller_manager.get()
    }
    
    fn enable_controller_manager(&self) -> bool {
        // Enable controllers and initialize the manager
        self.controller_enabled.store(true, std::sync::atomic::Ordering::Relaxed);
        // Force initialization by calling get_or_init
        self.controller_manager.get_or_init(|| Mutex::new(ControllerManager::new()));
        true
    }

    fn get_input_manager(&self) -> Option<&Mutex<InputManager>> {
        Some(&self.input_manager)
    }

    fn get_physics_2d(&self) -> Option<&std::cell::RefCell<PhysicsWorld2D>> {
        self.physics_2d.as_ref()
    }

    fn get_global_transform(&mut self, node_id: NodeID) -> Option<crate::structs2d::Transform2D> {
        Self::get_global_transform(self, node_id)
    }

    fn set_global_transform(&mut self, node_id: NodeID, transform: crate::structs2d::Transform2D) -> Option<()> {
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

            let filename = if cfg!(target_os = "windows") {
                "scripts.dll"
            } else if cfg!(target_os = "macos") {
                "libscripts.dylib"
            } else {
                "scripts.so"
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

/// Get the default perro rust path using a project root path directly
/// This avoids requiring the global project root to be set
pub fn default_perro_rust_path_from_root(project_root: &Path) -> PathBuf {
    let mut path = project_root.to_path_buf();
    path.push(".perro");
    path.push("scripts");
    path.push("builds");

    let filename = if cfg!(target_os = "windows") {
        "scripts.dll"
    } else if cfg!(target_os = "macos") {
        "libscripts.dylib"
    } else {
        "scripts.so"
    };

    path.push(filename);
    path
}

impl Scene<DllScriptProvider> {
    pub fn from_project(project: Rc<RefCell<Project>>, gfx: &mut crate::rendering::Graphics) -> anyhow::Result<Self> {
        let mut root_node = Node::new();
        root_node.name = Cow::Borrowed("Root");
        let root_node = SceneNode::Node(root_node);

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
        
        let lib = unsafe { Library::new(&lib_path).map_err(|e| {
            anyhow::anyhow!("Failed to load DLL at {:?}: {}. The DLL might be corrupted or incompatible.", lib_path, e)
        })? };
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
            eprintln!("   Try rebuilding scripts: cargo run -p perro_core -- --path <path> --scripts");
        } else {
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

        
        // ✅ root script first - load before merging main scene
        let root_script_path_opt = {
            let project_ref = game_scene.project.borrow();
            project_ref.root_script().map(|s| s.to_string())
        };

        if let Some(root_script_path) = root_script_path_opt {
            if let Ok(identifier) = script_path_to_identifier(&root_script_path) {
                if let Ok(ctor) = game_scene.provider.load_ctor(&identifier) {
                    let root_id = game_scene.get_root().get_id();
                    let boxed = game_scene.instantiate_script(ctor, root_id);
                    let handle = Rc::new(UnsafeCell::new(boxed));
                    game_scene.scripts.insert(root_id, handle);

                    let project_ref = game_scene.project.clone();
                    let mut project_borrow = project_ref.borrow_mut();

                    let now = Instant::now();
                    let true_delta = match game_scene.last_scene_update {
                        Some(prev) => now.duration_since(prev).as_secs_f32(),
                        None => 0.0,
                    };

                    let mut api = ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow, gfx);
                    api.call_init(root_id);
                    
                    // After script initialization, ensure renderable nodes are marked for rerender
                    // (old system would have called mark_dirty() here)
                    if let Some(node) = game_scene.nodes.get(root_id) {
                        if node.is_renderable() {
                            game_scene.needs_rerender.insert(root_id);
                        }
                    }
                } else {
                    println!("❌ Could not find symbol for {}", identifier);
                }
            }
        }

        let main_scene_path = game_scene.project.borrow().main_scene().to_string();
        let _t_load_begin = Instant::now();
        let loaded_data = SceneData::load(&main_scene_path)?;
        let _load_time = _t_load_begin.elapsed();

        // ────────────────────────────────────────────────
        // ⏱  Benchmark: Scene graft
        // ────────────────────────────────────────────────
        let _t_graft_begin = Instant::now();
        let game_root = game_scene.get_root().get_id();
        game_scene.merge_scene_data(loaded_data, game_root, gfx)?;
        let _graft_time = _t_graft_begin.elapsed();


        Ok(game_scene)
    }
}
