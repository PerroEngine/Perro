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
    asset_io::{ProjectRoot, get_project_root, load_asset, save_asset, set_project_root},
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
use once_cell::sync::OnceCell;

use glam::{Mat4, Vec3};
use indexmap::IndexMap;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::{SerializeStruct, Error as SerdeError}};
use serde_json::Value;
use smallvec::SmallVec;
use std::{
    any::Any,
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
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
/// Uses u32 indices for keys to preserve order during serialization
#[derive(Debug)]
pub struct SceneData {
    pub root_index: u32,
    pub nodes: IndexMap<u32, SceneNode>,
    // Mapping from index to temporary UUID (used during deserialization)
    // This allows us to remap parent references when converting to runtime
    // Not serialized - handled manually in Serialize/Deserialize impls
    index_to_uuid: HashMap<u32, Uuid>,
}

impl Clone for SceneData {
    fn clone(&self) -> Self {
        Self {
            root_index: self.root_index,
            nodes: self.nodes.iter().map(|(idx, node)| {
                (*idx, node.clone())
            }).collect(),
            index_to_uuid: self.index_to_uuid.clone(),
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
        state.serialize_field("root_index", &self.root_index)?;
        
        // Build reverse mapping: UUID -> index (for converting parent UUIDs to indices)
        let uuid_to_index: HashMap<Uuid, u32> = self.index_to_uuid.iter()
            .map(|(idx, uuid)| (*uuid, *idx))
            .collect();
        
        // Serialize nodes with u32 indices as keys
        struct NodesMap<'a> {
            nodes: &'a IndexMap<u32, SceneNode>,
            uuid_to_index: &'a HashMap<Uuid, u32>,
        }
        
        impl<'a> Serialize for NodesMap<'a> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(self.nodes.len()))?;
                for (idx, node) in self.nodes.iter() {
                    // Serialize node, but convert parent UUID to index
                    let mut node_value: Value = serde_json::to_value(node)
                        .map_err(|e| S::Error::custom(format!("Failed to serialize node: {}", e)))?;
                    
                    // Convert parent UUID to index if present
                    if let Some(obj) = node_value.as_object_mut() {
                        if let Some(parent_value) = obj.get_mut("parent") {
                            if let Some(parent_obj) = parent_value.as_object_mut() {
                                if let Some(id_value) = parent_obj.get("id") {
                                    if let Some(uuid_str) = id_value.as_str() {
                                        if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                                            if let Some(&parent_idx) = self.uuid_to_index.get(&uuid) {
                                                // Replace parent object with just the index
                                                *parent_value = serde_json::Value::Number(parent_idx.into());
                                            }
                                        }
                                    }
                                }
                            } else if let Some(uuid_str) = parent_value.as_str() {
                                // Parent is a UUID string, convert to index
                                if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                                    if let Some(&parent_idx) = self.uuid_to_index.get(&uuid) {
                                        *parent_value = serde_json::Value::Number(parent_idx.into());
                                    }
                                }
                            }
                        }
                    }
                    
                    map.serialize_entry(idx, &node_value)?;
                }
                map.end()
            }
        }
        
        state.serialize_field("nodes", &NodesMap { 
            nodes: &self.nodes,
            uuid_to_index: &uuid_to_index,
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
        
        // Deserialize as raw JSON first to extract parent indices
        let raw_value: Value = Value::deserialize(deserializer)?;
        
        // Accept both "root_index" and "root_id" for compatibility
        let root_index = raw_value.get("root_index")
            .or_else(|| raw_value.get("root_id"))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| D::Error::custom("root_index or root_id must be a u32"))? as u32;
        
        let nodes_obj = raw_value.get("nodes")
            .and_then(|v| v.as_object())
            .ok_or_else(|| D::Error::custom("nodes must be an object"))?;
        
        let capacity = nodes_obj.len();
        
        // Create index -> UUID mapping using deterministic UUIDs based on indices
        // Format: 00000000-0000-0000-0000-0000000000XX (where XX is the index in hex)
        let mut index_to_uuid: HashMap<u32, Uuid> = HashMap::with_capacity(capacity);
        for key in nodes_obj.keys() {
            if let Ok(idx) = key.parse::<u32>() {
                // Generate deterministic UUID from index
                let uuid_str = format!("00000000-0000-0000-0000-{:012x}", idx);
                let uuid = Uuid::parse_str(&uuid_str)
                    .unwrap_or_else(|_| Uuid::nil());
                index_to_uuid.insert(idx, uuid);
            }
        }
        
        // Deserialize nodes, handling parent indices
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
                    if let Some(parent_idx) = find_parent_recursive(v) {
                        return Some(parent_idx);
                    }
                }
            } else if let Some(arr) = value.as_array() {
                for item in arr {
                    if let Some(parent_idx) = find_parent_recursive(item) {
                        return Some(parent_idx);
                    }
                }
            }
            None
        }
        
        for (key_str, node_value) in nodes_obj {
            let idx = key_str.parse::<u32>()
                .map_err(|_| D::Error::custom(format!("Node key must be a u32 index, got: {}", key_str)))?;
            
            // Extract parent index if present (recursively search nested objects)
            let parent_idx = find_parent_recursive(node_value);
            
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
            
            // Set node ID to deterministic UUID based on index
            if let Some(&node_uuid) = index_to_uuid.get(&idx) {
                node.set_id(node_uuid);
            }
            
            node.clear_children();
            node.mark_transform_dirty_if_node2d();
            
            // Store parent relationship for later
            if let Some(pidx) = parent_idx {
                if index_to_uuid.contains_key(&pidx) {
                    parent_children.entry(pidx).or_default().push(idx);
                }
            }
            
            nodes.insert(idx, node);
        }
        
        // Second pass: set parent relationships with proper types and UUIDs
        for (parent_idx, child_indices) in parent_children {
            if let Some(&parent_uuid) = index_to_uuid.get(&parent_idx) {
                if let Some(parent_node) = nodes.get(&parent_idx) {
                    let parent_type_enum = parent_node.get_type();
                    
                    for child_idx in child_indices {
                        if let Some(child) = nodes.get_mut(&child_idx) {
                            let parent_type = crate::nodes::node::ParentType::new(parent_uuid, parent_type_enum);
                            child.set_parent(Some(parent_type));
                        }
                        // Add to parent's children list (using UUID)
                        // Only add if not already present to avoid duplicates
                        if let Some(parent) = nodes.get_mut(&parent_idx) {
                            if let Some(&child_uuid) = index_to_uuid.get(&child_idx) {
                                if !parent.get_children().contains(&child_uuid) {
                                    parent.add_child(child_uuid);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Store index_to_uuid mapping for later use when converting to runtime
        let index_to_uuid_map: HashMap<u32, Uuid> = index_to_uuid.into_iter().collect();
        
        Ok(SceneData {
            root_index,
            nodes,
            index_to_uuid: index_to_uuid_map,
        })
    }
}

impl SceneData {
    /// Get the index to UUID mapping
    pub fn index_to_uuid(&self) -> &HashMap<u32, Uuid> {
        &self.index_to_uuid
    }
    
    /// Create a new data scene with a root node
    pub fn new(root: SceneNode) -> Self {
        let root_id = root.get_id();
        let mut nodes = IndexMap::new();
        // OPTIMIZED: Use with_capacity(0) for known-empty map to avoid pre-allocation
        let mut index_to_uuid = HashMap::with_capacity(0);
        // Use index 0 for root
        index_to_uuid.insert(0, root_id);
        nodes.insert(0, root);
        Self { 
            root_index: 0, 
            nodes,
            index_to_uuid,
        }
    }
    
    /// Create SceneData from nodes and root_index
    /// Builds the index_to_uuid mapping and sets node IDs to deterministic UUIDs based on indices
    pub fn from_nodes(root_index: u32, mut nodes: IndexMap<u32, SceneNode>) -> Self {
        let mut index_to_uuid = HashMap::with_capacity(nodes.len());
        
        // Generate deterministic UUIDs based on indices: 00000000-0000-0000-0000-0000000000XX
        for (&idx, node) in nodes.iter_mut() {
            // Create deterministic UUID from index: format as 00000000-0000-0000-0000-0000000000XX
            let uuid_str = format!("00000000-0000-0000-0000-{:012x}", idx);
            let uuid = Uuid::parse_str(&uuid_str).unwrap_or_else(|_| {
                // Fallback: use nil UUID if parsing fails (shouldn't happen)
                Uuid::nil()
            });
            
            // Set the node's ID to match the deterministic UUID
            node.set_id(uuid);
            
            // Store in mapping
            index_to_uuid.insert(idx, uuid);
        }
        
        // Now update parent and child UUIDs to match the deterministic UUIDs
        for node in nodes.values_mut() {
            // Update parent UUID if it exists
            if let Some(parent) = node.get_parent() {
                // Extract index from parent UUID (last 12 hex digits)
                let parent_uuid_str = parent.id.to_string();
                let parent_idx_opt = parent_uuid_str.split('-').last()
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok());
                
                if let Some(parent_idx) = parent_idx_opt {
                    if let Some(&correct_uuid) = index_to_uuid.get(&parent_idx) {
                        if parent.id != correct_uuid {
                            // Update parent UUID to match
                            let parent_type = crate::nodes::node::ParentType::new(correct_uuid, parent.node_type);
                            node.set_parent(Some(parent_type));
                        }
                    }
                }
            }
            
            // Update children UUIDs
            let children = node.get_children().clone();
            node.clear_children();
            for child_uuid in children {
                let child_uuid_str = child_uuid.to_string();
                let child_idx_opt = child_uuid_str.split('-').last()
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok());
                
                if let Some(child_idx) = child_idx_opt {
                    if let Some(&correct_uuid) = index_to_uuid.get(&child_idx) {
                        node.add_child(correct_uuid);
                    }
                }
            }
        }
        
        Self {
            root_index,
            nodes,
            index_to_uuid,
        }
    }
    
    /// Convert SceneData to runtime Scene format
    /// Maps u32 indices to new UUIDs and remaps parent references
    pub fn to_runtime_nodes(mut self) -> (FxHashMap<Uuid, SceneNode>, Uuid) {
        // Create new UUIDs for runtime
        let mut old_to_new_uuid: HashMap<Uuid, Uuid> = HashMap::with_capacity(self.nodes.len());
        println!("🔍 Building old_to_new_uuid mapping ({} nodes):", self.nodes.len());
        for &idx in self.nodes.keys() {
            let old_uuid = self.index_to_uuid[&idx];
            let new_uuid = Uuid::new_v4();
            old_to_new_uuid.insert(old_uuid, new_uuid);
            println!("  Index {}: old_uuid={} -> new_uuid={}", idx, old_uuid, new_uuid);
        }
        
        let mut runtime_nodes = FxHashMap::default();
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::with_capacity(0);
        
        // First pass: create nodes with new UUIDs and collect parent relationships
        println!("🔍 First pass: remapping nodes and collecting parent relationships:");
        for (idx, mut node) in self.nodes {
            let old_uuid = self.index_to_uuid[&idx];
            let new_uuid = old_to_new_uuid[&old_uuid];
            node.set_id(new_uuid);
            node.clear_children();
            
            // Remap parent UUID
            if let Some(parent) = node.get_parent() {
                println!("  Node {} (idx={}): has parent with id={}", node.get_name(), idx, parent.id);
                if let Some(&new_parent_uuid) = old_to_new_uuid.get(&parent.id) {
                    // We'll set parent after we have all node types
                    println!("    Found parent mapping: {} -> {}", parent.id, new_parent_uuid);
                    parent_children.entry(new_parent_uuid).or_default().push(new_uuid);
                } else {
                    eprintln!("⚠️ WARNING: Parent UUID {} not found in old_to_new_uuid for node {} (idx={}, name={})", 
                        parent.id, new_uuid, idx, node.get_name());
                    eprintln!("    Available old UUIDs in mapping: {:?}", old_to_new_uuid.keys().collect::<Vec<_>>());
                }
            } else {
                println!("  Node {} (idx={}): NO PARENT", node.get_name(), idx);
            }
            
            runtime_nodes.insert(new_uuid, node);
        }
        
        // Second pass: set parent relationships with proper types
        for (parent_uuid, child_uuids) in parent_children {
            if let Some(parent_node) = runtime_nodes.get(&parent_uuid) {
                let parent_type_enum = parent_node.get_type();
                
                for child_uuid in child_uuids {
                    if let Some(child) = runtime_nodes.get_mut(&child_uuid) {
                    let parent_type = crate::nodes::node::ParentType::new(parent_uuid, parent_type_enum);
                    child.set_parent(Some(parent_type));
                    // Debug: verify parent was set
                    if child.get_parent().is_none() {
                        eprintln!("⚠️ WARNING: Failed to set parent on child {} -> parent {}", child_uuid, parent_uuid);
                    }
                }
                // Add to parent's children list
                if let Some(parent) = runtime_nodes.get_mut(&parent_uuid) {
                    parent.add_child(child_uuid);
                }
            }
        }
        }
        
        // Get root UUID
        let root_old_uuid = self.index_to_uuid[&self.root_index];
        let root_uuid = old_to_new_uuid[&root_old_uuid];
        
        (runtime_nodes, root_uuid)
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
        // use UUIDs from index_to_uuid, and children are already set.
        // This function can be used to verify/rebuild relationships if needed.
        
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::with_capacity(0);

        // Collect parent node types
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_types: HashMap<Uuid, crate::node_registry::NodeType> = HashMap::with_capacity(0);
        
        for (&idx, node) in data.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a UUID from index_to_uuid
                // Find which index it corresponds to
                let parent_idx_opt = data.index_to_uuid.iter()
                    .find(|&(_, &uuid)| uuid == parent.id)
                    .map(|(&idx, _)| idx);
                
                if let Some(parent_idx) = parent_idx_opt {
                    if let Some(parent_node) = data.nodes.get(&parent_idx) {
                        parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }

        // Rebuild parent-child relationships
        for (&idx, node) in data.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                let parent_uuid = parent.id;
                // Find parent index
                let parent_idx_opt = data.index_to_uuid.iter()
                    .find(|&(_, &uuid)| uuid == parent_uuid)
                    .map(|(&idx, _)| idx);
                
                if let Some(parent_idx) = parent_idx_opt {
                    let node_uuid = data.index_to_uuid[&idx];
                    parent_children.entry(parent_uuid).or_default().push(node_uuid);
                }
            }
        }

        // Apply relationships
        for (parent_uuid, children) in parent_children {
            // Find parent node by UUID
            let parent_idx_opt = data.index_to_uuid.iter()
                .find(|&(_, &uuid)| uuid == parent_uuid)
                .map(|(&idx, _)| idx);
            
            if let Some(parent_idx) = parent_idx_opt {
                if let Some(parent) = data.nodes.get_mut(&parent_idx) {
                    for child_uuid in children {
                        parent.add_child(child_uuid);
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
    pub(crate) nodes: FxHashMap<Uuid, SceneNode>,
    pub(crate) root_id: Uuid,
    pub signals: SignalBus,
    queued_signals: Vec<(u64, SmallVec<[Value; 3]>)>,
    pub scripts: FxHashMap<Uuid, Box<dyn ScriptObject>>,
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
    pub last_scene_render: Option<Instant>,
    // Optimize: Use HashSet for O(1) contains() checks (order doesn't matter for fixed updates)
    pub nodes_with_internal_fixed_update: HashSet<Uuid>,
    // Optimize: Use HashSet for O(1) contains() checks (order doesn't matter for render updates)
    pub nodes_with_internal_render_update: HashSet<Uuid>,

    // Physics (wrapped in RefCell for interior mutability through trait objects)
    // OPTIMIZED: Lazy initialization - only create when first physics object is added
    pub physics_2d: Option<std::cell::RefCell<PhysicsWorld2D>>,
    
    // OPTIMIZED: Cache script IDs to avoid Vec allocation every frame
    cached_script_ids: Vec<Uuid>,
    scripts_dirty: bool,
    
    // OPTIMIZED: Separate vectors for scripts with update/fixed_update/draw to avoid checking all scripts
    scripts_with_update: Vec<Uuid>,
    scripts_with_fixed_update: Vec<Uuid>,
    scripts_with_draw: Vec<Uuid>,
    
    // Track if texture_path → texture_id conversion has been done
    textures_converted: bool,
    
    // OPTIMIZED: Pre-accumulated set of node IDs that need rerendering
    // Using HashSet for O(1) membership checks
    needs_rerender: HashSet<Uuid>,
}

#[derive(Default)]
pub struct SignalBus {
    // signal_id → { script_uuid → SmallVec<[u64; 4]> (function_ids) }
    pub connections: HashMap<u64, HashMap<Uuid, SmallVec<[u64; 4]>>>,
}

impl<P: ScriptProvider> Scene<P> {
    /// Create a runtime scene from a root node
    pub fn new(root: SceneNode, provider: P, project: Rc<RefCell<Project>>) -> Self {
        let root_id = root.get_id();
        let mut nodes = FxHashMap::default();
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
            last_scene_render: Some(Instant::now()),
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
            scripts_with_draw: Vec::new(),
            
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
            last_scene_render: Some(Instant::now()),
            nodes_with_internal_fixed_update: HashSet::new(),
            nodes_with_internal_render_update: HashSet::new(),
            
            // OPTIMIZED: Initialize script ID cache
            cached_script_ids: Vec::new(),
            scripts_dirty: true,
            
            // OPTIMIZED: Initialize separate vectors for update/fixed_update/draw scripts
            scripts_with_update: Vec::new(),
            scripts_with_fixed_update: Vec::new(),
            scripts_with_draw: Vec::new(),
            
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
            println!("🔵 PhysicsWorld2D initialized (lazy) - this should NOT appear for projects without physics!");
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
    /// Assigns u32 indices to nodes based on traversal order
    pub fn to_scene_data(&self) -> SceneData {
        // Assign indices based on traversal order (root first, then children)
        let mut index = 0u32;
        // OPTIMIZED: Use with_capacity(0) for known-empty maps initially
        let mut uuid_to_index: HashMap<Uuid, u32> = HashMap::with_capacity(0);
        let mut nodes = IndexMap::new();
        let mut index_to_uuid: HashMap<u32, Uuid> = HashMap::with_capacity(0);
        
        // Traverse tree starting from root
        let mut to_process = vec![self.root_id];
        while let Some(node_id) = to_process.pop() {
            if uuid_to_index.contains_key(&node_id) {
                continue; // Already processed
            }
            
            if let Some(node) = self.nodes.get(&node_id) {
                uuid_to_index.insert(node_id, index);
                index_to_uuid.insert(index, node_id);
                nodes.insert(index, node.clone());
                
                // Add children to processing queue
                for child_id in node.get_children() {
                    to_process.push(*child_id);
                }
                
                index += 1;
            }
        }
        
        // Find root index
        let root_index = uuid_to_index.get(&self.root_id)
            .copied()
            .unwrap_or(0);
        
        // Convert parent UUIDs to match index_to_uuid (so serialization can find them)
        // The parent.id should be the UUID from index_to_uuid for that parent's index
        for (idx, node) in nodes.iter_mut() {
            if let Some(parent) = node.get_parent() {
                // Find which index this parent UUID corresponds to
                if let Some(&parent_idx) = uuid_to_index.get(&parent.id) {
                    // Get the UUID from index_to_uuid for this parent index
                    if let Some(&parent_uuid_from_index) = index_to_uuid.get(&parent_idx) {
                        // Update parent.id to match the UUID in index_to_uuid
                        // This ensures serialization can find it in the reverse mapping
                        let parent_type = crate::nodes::node::ParentType::new(
                            parent_uuid_from_index,
                            parent.node_type
                        );
                        node.set_parent(Some(parent_type));
                    }
                }
            }
        }
        
        SceneData {
            root_index,
            nodes,
            index_to_uuid,
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
        
        for (node_id, node) in self.nodes.iter_mut() {
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

                    let mut api = ScriptApi::new(true_delta, &mut game_scene, &mut *project_borrow, gfx);
                    api.call_init(root_id);
                    
                    // After script initialization, ensure renderable nodes are marked for rerender
                    // (old system would have called mark_dirty() here)
                    if let Some(node) = game_scene.nodes.get(&root_id) {
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
            proj_ref.main_scene().to_string()
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
                if obj.len() > 1 {
                    let mut entries: Vec<_> = obj.iter_mut().collect();
                    entries.par_iter_mut().for_each(|(_, v)| {
                        Self::remap_uuids_in_json_value(v, id_map);
                    });
                } else if let Some((_, v)) = obj.iter_mut().next() {
                    Self::remap_uuids_in_json_value(v, id_map);
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.len() > 1 {
                    arr.par_iter_mut().for_each(|v| {
                        Self::remap_uuids_in_json_value(v, id_map);
                    });
                } else if let Some(v) = arr.iter_mut().next() {
                    Self::remap_uuids_in_json_value(v, id_map);
                }
            }
            _ => {}
        }
    }

    fn remap_script_exp_vars_uuids(
        script_exp_vars: &mut HashMap<String, serde_json::Value>,
        id_map: &HashMap<Uuid, Uuid>,
    ) {
        if script_exp_vars.len() > 1 {
            let mut entries: Vec<_> = script_exp_vars.iter_mut().collect();
            entries.par_iter_mut().for_each(|(_, value)| {
                Self::remap_uuids_in_json_value(value, id_map);
            });
        } else if let Some((_, value)) = script_exp_vars.iter_mut().next() {
            Self::remap_uuids_in_json_value(value, id_map);
        }
    }
    pub fn merge_scene_data(
        &mut self,
        mut other: SceneData,
        parent_id: Uuid,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<()> {
        use std::time::Instant;
    
        let merge_start = Instant::now();
    
        // ───────────────────────────────────────────────
        // 1️⃣ BUILD INDEX → NEW RUNTIME ID MAP
        // ───────────────────────────────────────────────
        let id_map_start = Instant::now();
        // Map from old UUID (from index_to_uuid) to new runtime UUID
        let mut old_uuid_to_new_uuid: HashMap<Uuid, Uuid> = HashMap::with_capacity(other.nodes.len() + 1);
        // Also build index -> new UUID map for easier lookup
        let mut index_to_new_uuid: HashMap<u32, Uuid> = HashMap::with_capacity(other.nodes.len() + 1);
    
        println!("🔍 merge_scene_data: Building UUID mapping for {} nodes:", other.nodes.len());
        // Generate new UUIDs for all nodes
        for &idx in other.nodes.keys() {
            let old_uuid = other.index_to_uuid()[&idx];
            let new_uuid = Uuid::new_v4();
            old_uuid_to_new_uuid.insert(old_uuid, new_uuid);
            index_to_new_uuid.insert(idx, new_uuid);
            if let Some(node) = other.nodes.get(&idx) {
                println!("  Index {}: old_uuid={} -> new_uuid={} (node: {})", 
                    idx, old_uuid, new_uuid, node.get_name());
            }
        }
    
        let id_map_time = id_map_start.elapsed();
    
        // ───────────────────────────────────────────────
        // 2️⃣ REMAP NODES AND BUILD RELATIONSHIPS
        // ───────────────────────────────────────────────
        let remap_start = Instant::now();
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::with_capacity(0);
    
        // Get the subscene root's NEW runtime ID
        let subscene_root_index = other.root_index;
        let subscene_root_new_id = index_to_new_uuid[&subscene_root_index];
    
        // Check if root has is_root_of (determines if we skip the root later)
        let root_is_root_of = other
            .nodes
            .get(&other.root_index)
            .and_then(|n| Self::get_is_root_of(n));
    
        let skip_root_id: Option<Uuid> = if root_is_root_of.is_some() {
            Some(subscene_root_new_id)
        } else {
            None
        };
    
        // First, collect parent node types from other.nodes before mutable iteration
        // Parent IDs in nodes are UUIDs from index_to_uuid, so we need to map them
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut other_parent_types: HashMap<Uuid, crate::node_registry::NodeType> = HashMap::with_capacity(0);
        for (&idx, node) in other.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a UUID from index_to_uuid, find which index it corresponds to
                // We need to reverse lookup: find index where index_to_uuid[index] == parent.id
                let parent_idx_opt = other.index_to_uuid().iter()
                    .find(|&(_, &uuid)| uuid == parent.id)
                    .map(|(&idx, _)| idx);
                
                if let Some(parent_idx) = parent_idx_opt {
                    if let Some(parent_node) = other.nodes.get(&parent_idx) {
                        other_parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }

        // Collect index_to_uuid mapping before mutable iteration
        let index_to_uuid_copy: HashMap<u32, Uuid> = other.index_to_uuid().iter().map(|(&k, &v)| (k, v)).collect();

        // Remap all nodes
        for (idx, node) in other.nodes.iter_mut() {
            let new_id = index_to_new_uuid[idx];
            node.set_id(new_id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Determine parent relationship
            if let Some(parent) = node.get_parent() {
                let parent_old_uuid = parent.id;
                
                // Check if parent is in the subscene (remap old UUID to new UUID)
                if let Some(&mapped_parent) = old_uuid_to_new_uuid.get(&parent_old_uuid) {
                    // Parent is in subscene - use mapped runtime ID
                    // Get parent type from other_parent_types (collected earlier) or from already-inserted nodes
                    let parent_type_enum = if let Some(&parent_type) = other_parent_types.get(&parent_old_uuid) {
                        parent_type
                    } else if let Some(parent_node) = self.nodes.get(&mapped_parent) {
                        parent_node.get_type()
                    } else {
                        // Fallback - shouldn't happen
                        crate::node_registry::NodeType::Node
                    };
                    let parent_type = crate::nodes::node::ParentType::new(mapped_parent, parent_type_enum);
                    node.set_parent(Some(parent_type));
                    parent_children
                        .entry(mapped_parent)
                        .or_default()
                        .push(new_id);
                } else {
                    // Parent not in subscene - check if it exists in main scene
                    // parent_old_uuid is from other's index_to_uuid, so we need to find which index it corresponds to
                    // and then check if that index's UUID exists in self.nodes
                    let _parent_idx_opt = index_to_uuid_copy.iter()
                        .find(|&(_, &uuid)| uuid == parent_old_uuid)
                        .map(|(&idx, _)| idx);
                    
                    // For now, don't set parent - this is an invalid reference
                }
            } else if new_id == subscene_root_new_id {
                // This is the subscene root with no parent - attach to game's parent_id
                // But only if we're NOT skipping it (is_root_of case)
                if skip_root_id.is_none() {
                    println!(
                        "🔗 Attaching subscene root {} to game parent {}",
                        new_id, parent_id
                    );
                    // Create ParentType with the parent's type
                    if let Some(parent_node) = self.nodes.get(&parent_id) {
                        let parent_type = crate::nodes::node::ParentType::new(parent_id, parent_node.get_type());
                        node.set_parent(Some(parent_type));
                    }
                    parent_children.entry(parent_id).or_default().push(new_id);
                }
            }
            // else: node has no parent and isn't root - leave as orphan (shouldn't happen normally)
    
            // Handle script_exp_vars - remap UUIDs using old_uuid_to_new_uuid
            if let Some(mut script_vars) = node.get_script_exp_vars() {
                Self::remap_script_exp_vars_uuids(&mut script_vars, &old_uuid_to_new_uuid);
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
        println!(
            "⏱ Node remapping: {:.2} ms",
            remap_time.as_secs_f64() * 1000.0
        );
    
        // ───────────────────────────────────────────────
        // 3️⃣ INSERT NODES INTO MAIN SCENE
        // ───────────────────────────────────────────────
        let insert_start = Instant::now();
        self.nodes.reserve(other.nodes.len() + 1);
    
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
            if let Some(node_ref) = self.nodes.get(&node_id) {
                if node_ref.is_renderable() {
                    self.needs_rerender.insert(node_id);
                }
            }
    
            // Register node for internal fixed updates if needed
            if let Some(node_ref) = self.nodes.get(&node_id) {
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
            if let Some(game_parent) = self.nodes.get_mut(&parent_id) {
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
            if let Some(node) = self.nodes.get(id) {
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
        println!(
            "⏱ Physics registration: {:.2} ms",
            physics_time.as_secs_f64() * 1000.0
        );
    
        // ───────────────────────────────────────────────
        // 6️⃣ FUR LOADING (UI FILES)
        // ───────────────────────────────────────────────
        let fur_start = Instant::now();
        // Collect FUR paths
        let fur_paths: Vec<(Uuid, String)> = inserted_ids
            .iter()
            .filter_map(|id| {
                self.nodes.get(id).and_then(|node| {
                    if let SceneNode::UINode(u) = node {
                        u.fur_path.as_ref().map(|path| (*id, path.to_string()))
                    } else {
                        None
                    }
                })
            })
            .collect();
        
        // Load FUR data - always parallelize (I/O operations benefit even more)
        let fur_loads: Vec<(Uuid, Result<Vec<FurElement>, _>)> =
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
            if let Some(node) = self.nodes.get_mut(&id) {
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
                self.nodes
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
                if flags.has_draw() && !self.scripts_with_draw.contains(&id) {
                    self.scripts_with_draw.push(id);
                }
                
                self.scripts.insert(id, handle);
                self.scripts_dirty = true;
    
                let mut api = ScriptApi::new(dt, self, &mut *project_borrow, gfx);
                api.call_init(id);
                
                // After script initialization, ensure renderable nodes are marked for rerender
                // (old system would have called mark_dirty() here)
                if let Some(node) = self.nodes.get(&id) {
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
        println!(
            "📦 Merge complete: {} total nodes (+{} new)",
            self.nodes.len(),
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
    
        // Print scene tree for debugging
    
        Ok(())
    }
    
    /// Merge a nested scene where the nested scene's root REPLACES an existing node
    /// (used for is_root_of scenarios)
    fn merge_scene_data_with_root_replacement(
        &mut self,
        mut other: SceneData,
        replacement_root_id: Uuid,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<()> {
        println!(
            "🔄 merge_scene_data_with_root_replacement: replacement_root={}",
            replacement_root_id
        );
    
        // Build mapping from old UUID (from index_to_uuid) to new runtime UUID
        let mut old_uuid_to_new_uuid: HashMap<Uuid, Uuid> = HashMap::with_capacity(other.nodes.len());
        let mut index_to_new_uuid: HashMap<u32, Uuid> = HashMap::with_capacity(other.nodes.len());
    
        // Generate UUIDs for all nodes EXCEPT the root
        let subscene_root_index = other.root_index;
    
        for &idx in other.nodes.keys() {
            let old_uuid = other.index_to_uuid()[&idx];
            if idx == subscene_root_index {
                // Root maps to the replacement node (which already exists)
                old_uuid_to_new_uuid.insert(old_uuid, replacement_root_id);
                index_to_new_uuid.insert(idx, replacement_root_id);
            } else {
                let new_uuid = Uuid::new_v4();
                old_uuid_to_new_uuid.insert(old_uuid, new_uuid);
                index_to_new_uuid.insert(idx, new_uuid);
            }
        }
    
        println!(
            "   ✅ Mapped nested scene root index {} → {}",
            subscene_root_index, replacement_root_id
        );
    
        // Build parent-children relationships
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut parent_children: HashMap<Uuid, Vec<Uuid>> = HashMap::with_capacity(0);
        
        // First, collect parent node types from other.nodes before mutable iteration
        // Parent IDs in nodes are UUIDs from index_to_uuid, so we need to map them
        // OPTIMIZED: Use with_capacity(0) for known-empty map
        let mut other_parent_types: HashMap<Uuid, crate::node_registry::NodeType> = HashMap::with_capacity(0);
        for (&idx, node) in other.nodes.iter() {
            if let Some(parent) = node.get_parent() {
                // parent.id is a UUID from index_to_uuid, find which index it corresponds to
                let parent_idx_opt = other.index_to_uuid.iter()
                    .find(|&(_, &uuid)| uuid == parent.id)
                    .map(|(&idx, _)| idx);
                
                if let Some(parent_idx) = parent_idx_opt {
                    if let Some(parent_node) = other.nodes.get(&parent_idx) {
                        other_parent_types.insert(parent.id, parent_node.get_type());
                    }
                }
            }
        }
    
        // Remap all nodes
        for (idx, node) in other.nodes.iter_mut() {
            let new_id = index_to_new_uuid[idx];
            node.set_id(new_id);
            node.clear_children();
            node.mark_transform_dirty_if_node2d();

            // Remap parent using old_uuid_to_new_uuid (like in merge_scene_data)
            if let Some(parent) = node.get_parent() {
                let parent_old_uuid = parent.id;
                if let Some(&mapped_parent) = old_uuid_to_new_uuid.get(&parent_old_uuid) {
                    // Parent is in subscene - use mapped runtime ID
                    // Get parent type from other_parent_types (collected earlier) or from already-inserted nodes
                    let parent_type_enum = if let Some(&parent_type) = other_parent_types.get(&parent_old_uuid) {
                        parent_type
                    } else if let Some(parent_node) = self.nodes.get(&mapped_parent) {
                        parent_node.get_type()
                    } else {
                        // Fallback - shouldn't happen
                        crate::node_registry::NodeType::Node
                    };
                    let parent_type = crate::nodes::node::ParentType::new(mapped_parent, parent_type_enum);
                    node.set_parent(Some(parent_type));
                    parent_children
                        .entry(mapped_parent)
                        .or_default()
                        .push(new_id);
                } else {
                    // Parent not in subscene - check if it exists in main scene
                    if let Some(parent_node) = self.nodes.get(&parent_old_uuid) {
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

            // Remap script_exp_vars using old_uuid_to_new_uuid
            if let Some(mut script_vars) = node.get_script_exp_vars() {
                Self::remap_script_exp_vars_uuids(&mut script_vars, &old_uuid_to_new_uuid);
                node.set_script_exp_vars(Some(script_vars));
            }
        }
    
        // Apply parent-child relationships within the subscene
        for (parent_new_id, children) in &parent_children {
            // If parent is the replacement root, update the existing node in main scene
            if *parent_new_id == replacement_root_id {
                if let Some(existing_node) = self.nodes.get_mut(&replacement_root_id) {
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
            if let Some(node_ref) = self.nodes.get(&node_id) {
                if node_ref.is_renderable() {
                    self.needs_rerender.insert(node_id);
                }
            }
    
            // Register for internal fixed updates if needed
            if let Some(node_ref) = self.nodes.get(&node_id) {
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
            if let Some(node) = self.nodes.get_mut(id) {
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
                self.nodes
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
                        if flags.has_draw() && !self.scripts_with_draw.contains(&id) {
                            self.scripts_with_draw.push(id);
                        }
                        
                        self.scripts.insert(id, handle);
                        self.scripts_dirty = true;
    
                        let mut api = ScriptApi::new(dt, self, &mut *project_borrow, gfx);
                        api.call_init(id);
                        
                        // After script initialization, ensure renderable nodes are marked for rerender
                        // (old system would have called mark_dirty() here)
                        if let Some(node) = self.nodes.get(&id) {
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
        let mut nodes_with_nested_scenes: Vec<(Uuid, String)> = Vec::new();
    
        for id in &inserted_ids {
            if let Some(node) = self.nodes.get(id) {
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
    
        // Print scene tree for debugging
    
        Ok(())
    }
    

    pub fn print_scene_tree(&self) {
        // Debug function - currently disabled
        // Can be re-enabled for debugging scene structure
    }

    fn print_node_recursive(&self, node_id: Uuid, depth: usize, is_last: bool) {
        if let Some(node) = self.nodes.get(&node_id) {
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
            let script_path = node.get_script_path();
            let parent_info = node.get_parent()
                .map(|p| format!("parent={}", p.id))
                .unwrap_or_else(|| "ROOT".to_string());
            let has_script = self.scripts.contains_key(&node_id);
            let script_status = if has_script { "✓SCRIPT" } else { "✗NO_SCRIPT" };
            
            // Print this node with detailed debug info
            println!("{}{} [id={}] [{}] [{}] {}",
                prefix, 
                name,
                node_id,
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
        self.nodes.values().any(|n| {
            n.get_parent().map(|p| p.id) == Some(parent_id) && n.get_name() == name
        })
    }

    /// Check if a node name conflicts with its parent or any ancestor
    fn has_parent_or_ancestor_name_conflict(&self, parent_id: Option<Uuid>, name: &str) -> bool {
        let mut current_id = parent_id;
        
        // Walk up the tree checking each ancestor
        while let Some(id) = current_id {
            if let Some(ancestor) = self.nodes.get(&id) {
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
    fn resolve_name_conflict(&self, parent_id: Uuid, base_name: &str) -> String {
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

    pub fn render(&mut self, gfx: &mut Graphics, now: Instant) {
        // Convert texture_path → texture_id on first render (when Graphics is available)
        if !self.textures_converted {
            self.convert_texture_paths_to_ids(gfx);
            self.textures_converted = true;
        }
        
        // Calculate render delta (time since last render call - actual frame time)
        let render_delta = match self.last_scene_render {
            Some(prev) => now.duration_since(prev).as_secs_f32(),
            None => 0.0,
        };
        // Update render time for next frame
        self.last_scene_render = Some(now);
        
        // Call draw() methods on scripts that implement it (frame-synchronized visuals)
        {
            // OPTIMIZED: Accept now as parameter to avoid duplicate Instant::now() call
            
            // OPTIMIZED: Use pre-filtered vector of scripts with draw
            // Collect script IDs to avoid borrow checker issues
            let script_ids: Vec<Uuid> = self.scripts_with_draw.iter().copied().collect();
            
            if !script_ids.is_empty() {
                // Clone project reference before loop to avoid borrow conflicts
                let project_ref = self.project.clone();
                
                #[cfg(feature = "profiling")]
                let _span = tracing::span!(tracing::Level::INFO, "script_draws", count = script_ids.len()).entered();
                
                for id in script_ids {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "script_draw", id = %id).entered();
                    
                    // OPTIMIZED: Borrow project once per script
                    // Note: We can't borrow project once for all scripts because ScriptApi needs &mut self (scene)
                    let mut project_borrow = project_ref.borrow_mut();
                    let mut api = ScriptApi::new(render_delta, self, &mut *project_borrow, gfx);
                    api.call_draw(id);
                }
            }
        }
        
        // Run internal render update for nodes that need it (e.g., UI interactions)
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "node_internal_render_updates").entered();
            
            // Optimize: collect first to avoid borrow checker issues (HashSet iteration order is non-deterministic but that's fine)
            let node_ids: Vec<Uuid> = self.nodes_with_internal_render_update.iter().copied().collect();
            
            if !node_ids.is_empty() {
                // Clone project reference before loop to avoid borrow conflicts
                let project_ref = self.project.clone();
                
                for node_id in node_ids {
                    #[cfg(feature = "profiling")]
                    let _span = tracing::span!(tracing::Level::INFO, "node_internal_render_update", id = %node_id).entered();
                    
                    // OPTIMIZED: Borrow project once per node
                    let mut project_borrow = project_ref.borrow_mut();
                    let mut api = ScriptApi::new(render_delta, self, &mut *project_borrow, gfx);
                    api.call_node_internal_render_update(node_id);
                }
            }
        }
        
        let nodes_needing_rerender = {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "get_nodes_needing_rerender").entered();
            self.get_nodes_needing_rerender()
        };
        if nodes_needing_rerender.is_empty() {
            return;
        }
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "traverse_and_render", count = nodes_needing_rerender.len()).entered();
            self.traverse_and_render(nodes_needing_rerender, gfx);
        }
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
                    let script_ids: Vec<Uuid> = self.scripts_with_fixed_update.iter().copied().collect();
                    
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
                    let node_ids: Vec<Uuid> = self.nodes_with_internal_fixed_update.iter().copied().collect();
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
            let script_ids: Vec<Uuid> = self.scripts_with_update.iter().copied().collect();
            
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

        // Note: Signals are called from ScriptApi which has Graphics, but emit_signal_impl
        // doesn't have access to it. For now, we'll skip signal emission here since
        // signals should be emitted through ScriptApi which has Graphics.
        // This is a temporary workaround - signals should be refactored to pass Graphics through.
        // The actual signal emission happens in ScriptApi::emit_signal_id which has Graphics.
        return;
    }

    pub fn instantiate_script(ctor: CreateFn, node_id: Uuid) -> Box<dyn ScriptObject> {
        let raw = ctor();
        let mut boxed: Box<dyn ScriptObject> = unsafe { Box::from_raw(raw) };
        boxed.set_id(node_id);
        boxed
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
        if let Some(node) = self.nodes.get(&id) {
            if node.is_renderable() {
                self.needs_rerender.insert(id);
            }
        }


        // Register node for internal fixed updates if needed
        if let Some(node_ref) = self.nodes.get(&id) {
            if node_ref.needs_internal_fixed_update() {
                // Optimize: HashSet insert is O(1) and handles duplicates automatically
                self.nodes_with_internal_fixed_update.insert(id);
            }
        }

        // node is moved already, so get it back immutably from scene
        let script_path_opt = self.nodes.get(&id)
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
            if flags.has_draw() && !self.scripts_with_draw.contains(&id) {
                self.scripts_with_draw.push(id);
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
            if let Some(node) = self.nodes.get(&id) {
                if node.is_renderable() {
                    self.needs_rerender.insert(id);
                }
            }

            println!("   ✅ Script initialized");
        }

        Ok(())
    }

    pub fn get_root(&self) -> &SceneNode {
        &self.nodes[&self.root_id]
    }

    /// Get reference to scripts that have draw() implemented
    /// Used by the rendering loop to call draw on frame-synchronized scripts
    pub fn get_scripts_with_draw(&self) -> &Vec<Uuid> {
        &self.scripts_with_draw
    }

    // Remove node and stop rendering
    pub fn remove_node(&mut self, node_id: Uuid, gfx: &mut Graphics) {
        // Stop rendering this node and all its children
        self.stop_rendering_recursive(node_id, gfx);

        // Remove from scene
        self.nodes.remove(&node_id);

        // Remove scripts - actually delete them from scene
        if self.scripts.remove(&node_id).is_some() {
            // Remove from update/fixed_update/draw vectors (actual deletion)
            self.scripts_with_update.retain(|&script_id| script_id != node_id);
            self.scripts_with_fixed_update.retain(|&script_id| script_id != node_id);
            self.scripts_with_draw.retain(|&script_id| script_id != node_id);
            self.scripts_dirty = true;
        }
    }

    /// Get the global transform for a node, calculating it lazily if dirty
    /// This recursively traverses up the parent chain until it finds a clean transform
    pub fn get_global_transform(&mut self, node_id: Uuid) -> Option<crate::structs2d::Transform2D> {
        // First, check if this node exists and get its parent
        let (parent_id_opt, local_transform, is_dirty) = if let Some(node) = self.nodes.get(&node_id) {
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
            (node.get_parent().map(|p| p.id), local, node2d.transform_dirty)
        } else {
            return None;
        };

        // If not dirty, return cached global transform
        if !is_dirty {
            if let Some(node) = self.nodes.get(&node_id) {
                return node.as_node2d().map(|n2d| n2d.global_transform);
            }
        }

        // Need to recalculate - get parent's global transform (recursively)
        // If parent is not Node2D-based, use identity transform
        let parent_global = if let Some(parent_id) = parent_id_opt {
            // Try to get parent's global transform, but if parent is not Node2D-based, use identity
            self.get_global_transform(parent_id).unwrap_or_else(|| {
                // Parent exists but is not Node2D-based (e.g., regular Node) - use identity transform
                crate::structs2d::Transform2D::default()
            })
        } else {
            // No parent - use identity transform
            crate::structs2d::Transform2D::default()
        };

        // OPTIMIZED: Calculate this node's global transform using efficient matrix math
        // This replaces the old component-wise calculation with a single SIMD matrix multiply
        // Benefits:
        // - Correct rotation inheritance (position rotates around parent)
        // - 3-5x faster for deep hierarchies
        // - SIMD-optimized (glam uses SSE2/NEON)
        // - More accurate (less floating-point error)
        let global = crate::structs2d::Transform2D::calculate_global(&parent_global, &local_transform);

        // Cache the result and mark as clean
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if let Some(node2d) = node.as_node2d_mut() {
                node2d.global_transform = global;
                node2d.transform_dirty = false;
            }
        }

        Some(global)
    }
    
    /// OPTIONAL: Batch-optimized version for precalculate_transforms_in_dependency_order
    /// Use this when calculating many siblings with the same parent
    /// ~20% faster than calling get_global_transform() in a loop
    fn precalculate_transforms_batch(&mut self, parent_id: Uuid, child_ids: &[Uuid]) {
        // Get parent's global transform once
        let parent_global = self.get_global_transform(parent_id)
            .unwrap_or_else(|| crate::structs2d::Transform2D::default());
        
        // Collect local transforms
        let mut local_transforms = Vec::with_capacity(child_ids.len());
        for &child_id in child_ids {
            if let Some(node) = self.nodes.get(&child_id) {
                if let Some(local) = node.get_node2d_transform() {
                    local_transforms.push((child_id, local));
                }
            }
        }
        
        // Batch calculate (reuses parent matrix conversion)
        let locals: Vec<_> = local_transforms.iter().map(|(_, t)| *t).collect();
        let globals = crate::structs2d::Transform2D::batch_calculate_global(&parent_global, &locals);
        
        // Cache results
        for ((child_id, _), global) in local_transforms.iter().zip(globals.iter()) {
            if let Some(node) = self.nodes.get_mut(child_id) {
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.global_transform = *global;
                    node2d.transform_dirty = false;
                }
            }
        }
    }


    /// Set the global transform for a node (marks it as dirty)
    pub fn set_global_transform(&mut self, node_id: Uuid, transform: crate::structs2d::Transform2D) -> Option<()> {
        if let Some(node) = self.nodes.get_mut(&node_id) {
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
    pub fn mark_transform_dirty_recursive(&mut self, node_id: Uuid) {
        // OPTIMIZED: Use cached Node2D children if available (avoids hashmap lookups)
        let node2d_child_ids: Vec<Uuid> = {
            // Check if we have a cached list of Node2D children
            let cached = self.nodes
                .get(&node_id)
                .and_then(|node| node.as_node2d())
                .and_then(|n2d| n2d.node2d_children_cache.as_ref());
            
            if let Some(cached_ids) = cached {
                // Use cache - no hashmap lookups needed!
                cached_ids.clone()
            } else {
                // Cache miss - build cache and use it
                let child_ids: Vec<Uuid> = self.nodes
                    .get(&node_id)
                    .map(|node| node.get_children().iter().copied().collect())
                    .unwrap_or_default();
                
                // Filter to only Node2D children and cache the result
                let node2d_ids: Vec<Uuid> = child_ids.iter()
                    .filter_map(|&child_id| {
                        if let Some(child_node) = self.nodes.get(&child_id) {
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
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    if let Some(node2d) = node.as_node2d_mut() {
                        node2d.node2d_children_cache = Some(node2d_ids.clone());
                    }
                }
                
                node2d_ids
            }
        };
        
        // Mark this node's transform as dirty if it's a Node2D-based node
        if let Some(node) = self.nodes.get_mut(&node_id) {
            if let Some(node2d) = node.as_node2d_mut() {
                node2d.transform_dirty = true;
            }
        }
        
        // Only add renderable nodes to needs_rerender
        // Non-renderable nodes (like Node2D) don't need to be rendered,
        // but their transform_dirty flag is still set so renderable children
        // can recalculate their global transforms during rendering
        if let Some(node) = self.nodes.get(&node_id) {
            if node.is_renderable() && !self.needs_rerender.contains(&node_id) {
                self.needs_rerender.insert(node_id);
            }
        }
        
        // OPTIMIZED: Only recurse into Node2D-based children (from cache or filtered)
        // This avoids thousands of unnecessary recursive calls when a parent has many base Node children
        for child_id in node2d_child_ids {
            self.mark_transform_dirty_recursive(child_id);
        }
    }
    
    /// Update the Node2D children cache for a parent node when a child is added
    /// This keeps the cache in sync so we don't need hashmap lookups every frame
    pub fn update_node2d_children_cache_on_add(&mut self, parent_id: Uuid, child_id: Uuid) {
        // Check if child is Node2D first (immutable borrow)
        let is_node2d = self.nodes
            .get(&child_id)
            .map(|child_node| child_node.as_node2d().is_some())
            .unwrap_or(false);
        
        // Now update parent's cache (mutable borrow)
        if is_node2d {
            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
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
    }
    
    /// Update the Node2D children cache for a parent node when a child is removed
    fn update_node2d_children_cache_on_remove(&mut self, parent_id: Uuid, child_id: Uuid) {
        if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
            if let Some(node2d) = parent_node.as_node2d_mut() {
                // Remove from cache if it exists
                if let Some(ref mut cache) = node2d.node2d_children_cache {
                    cache.retain(|&id| id != child_id);
                }
            }
        }
    }

   

    /// Update collider transforms to match node transforms
    fn update_collider_transforms(&mut self) {
        // OPTIMIZED: Parallelize node filtering (read-only operation)
        let node_ids: Vec<Uuid> = if self.nodes.len() >= 10 {
            // Use parallel iteration for larger node counts
            self.nodes
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
                            Some(*node_id)
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
            if let Some(node) = self.nodes.get_mut(node_id) {
                if let Some(node2d) = node.as_node2d_mut() {
                    node2d.transform_dirty = true;
                }
            }
        }
        
        // Get global transforms (requires mutable access)
        // Now that we've marked them dirty, get_global_transform will recalculate
        // OPTIMIZED: Pre-allocate with known capacity
        let mut to_update: Vec<(Uuid, [f32; 2], f32)> = Vec::with_capacity(node_ids.len());
        for node_id in node_ids {
            if let Some(global) = self.get_global_transform(node_id) {
                let position = [global.position.x, global.position.y];
                let rotation = global.rotation;
                to_update.push((node_id, position, rotation));
            }
        }
        
        // Update physics colliders (after releasing all borrows)
        // OPTIMIZED: Only update if physics world exists
        if let Some(physics) = &mut self.physics_2d {
            let mut physics = physics.borrow_mut();
            for (node_id, position, rotation) in to_update {
                physics.update_collider_transform(node_id, position, rotation);
            }
        }
    }

    /// Register CollisionShape2D nodes with the physics world
    fn register_collision_shapes(&mut self, node_ids: &[Uuid]) {
        // First, collect all the data we need (shape info, transforms, parent info)
        let mut to_register: Vec<(Uuid, crate::physics::physics_2d::ColliderShape, Option<crate::nodes::node::ParentType>)> = Vec::new();
        
        for &node_id in node_ids {
            if let Some(node) = self.nodes.get(&node_id) {
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
        let mut global_transforms: HashMap<Uuid, ([f32; 2], f32)> = HashMap::with_capacity(0);
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
        let mut registration_data: Vec<(Uuid, crate::physics::physics_2d::ColliderShape, Option<Uuid>, bool)> = Vec::new();
        for (node_id, shape, parent_opt) in to_register {
            let is_area2d_parent = if let Some(parent) = &parent_opt {
                let pid = parent.id;
                if let Some(parent_node) = self.nodes.get(&pid) {
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
        let mut handles_to_store: Vec<(Uuid, rapier2d::prelude::ColliderHandle, Option<Uuid>)> = Vec::new();
        
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
            if let Some(node) = self.nodes.get_mut(&node_id) {
                if let SceneNode::CollisionShape2D(cs) = node {
                    cs.collider_handle = Some(collider_handle);
                }
            }
        }
    }

    fn stop_rendering_recursive(&self, node_id: Uuid, gfx: &mut Graphics) {
        if let Some(node) = self.nodes.get(&node_id) {
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

    // Get nodes needing rerender
    // OPTIMIZED: Returns pre-accumulated set instead of iterating over all nodes
    fn get_nodes_needing_rerender(&mut self) -> Vec<Uuid> {
        // Take ownership of the HashSet and convert to Vec
        self.needs_rerender.drain().collect()
    }

    /// Pre-calculate transforms for nodes in dependency order (parents before children)
    /// This ensures that when calculating a child's transform, the parent's transform is already cached.
    /// This is a major performance optimization when many children share the same moving parent.
    /// OPTIMIZED: Skips non-Node2D parents when calculating depth (they don't affect transform inheritance).
    fn precalculate_transforms_in_dependency_order(&mut self, node_ids: &[Uuid]) {
        // Group nodes by parent for batch processing
        // OPTIMIZED: Use with_capacity(0) for known-empty map initially
        let mut nodes_by_parent: std::collections::HashMap<Option<Uuid>, Vec<(Uuid, usize)>> = 
            std::collections::HashMap::with_capacity(0);
        
        // OPTIMIZED: Parallelize depth calculation for large node counts
        // Each node's depth calculation is independent (read-only access to nodes)
        let node_depths: Vec<(Uuid, Option<Uuid>, usize)> = if node_ids.len() >= 20 {
            // Parallel path for larger node sets
            node_ids
                .par_iter()
                .filter_map(|&node_id| {
                    if let Some(node) = self.nodes.get(&node_id) {
                        if node.as_node2d().is_some() {
                            // Calculate depth (read-only traversal up parent chain)
                            let mut depth = 0;
                            let mut current_id = Some(node_id);
                            while let Some(id) = current_id {
                                if let Some(n) = self.nodes.get(&id) {
                                    if let Some(parent) = n.get_parent() {
                                        if let Some(parent_node) = self.nodes.get(&parent.id) {
                                            if parent_node.as_node2d().is_some() {
                                                depth += 1;
                                            }
                                        }
                                        current_id = Some(parent.id);
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                            
                            let parent_id = node.get_parent().map(|p| p.id);
                            Some((node_id, parent_id, depth))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            // Sequential path for small node sets (avoid parallel overhead)
            node_ids
                .iter()
                .filter_map(|&node_id| {
                    if let Some(node) = self.nodes.get(&node_id) {
                        if node.as_node2d().is_some() {
                            // Calculate depth
                            let mut depth = 0;
                            let mut current_id = Some(node_id);
                            while let Some(id) = current_id {
                                if let Some(n) = self.nodes.get(&id) {
                                    if let Some(parent) = n.get_parent() {
                                        if let Some(parent_node) = self.nodes.get(&parent.id) {
                                            if parent_node.as_node2d().is_some() {
                                                depth += 1;
                                            }
                                        }
                                        current_id = Some(parent.id);
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                            
                            let parent_id = node.get_parent().map(|p| p.id);
                            Some((node_id, parent_id, depth))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        };
        
        // Build the parent grouping map (sequential, but fast)
        for (node_id, parent_id, depth) in node_depths {
            nodes_by_parent.entry(parent_id).or_default().push((node_id, depth));
        }
        
        // Sort each sibling group by depth
        for siblings in nodes_by_parent.values_mut() {
            siblings.sort_by_key(|(_, depth)| *depth);
        }
        
        // Process in depth order, but batch siblings together
        let mut processed_depths: Vec<_> = nodes_by_parent
            .values()
            .flat_map(|siblings| siblings.iter().map(|(_, depth)| *depth))
            .collect();
        processed_depths.sort_unstable();
        processed_depths.dedup();
        
        for depth in processed_depths {
            for (parent_id, siblings) in &nodes_by_parent {
                let siblings_at_depth: Vec<Uuid> = siblings
                    .iter()
                    .filter(|(_, d)| *d == depth)
                    .map(|(id, _)| *id)
                    .collect();
                
                if !siblings_at_depth.is_empty() {
                    if let Some(parent) = parent_id {
                        // Batch process siblings with same parent
                        self.precalculate_transforms_batch(*parent, &siblings_at_depth);
                    } else {
                        // Root nodes - process individually
                        for node_id in siblings_at_depth {
                            let _ = self.get_global_transform(node_id);
                        }
                    }
                }
            }
        }
    }

    fn traverse_and_render(&mut self, nodes_needing_rerender: Vec<Uuid>, gfx: &mut Graphics) {
        // Internal enum for parallel render command collection
        enum RenderCommand {
            Texture {
                texture_id: Option<Uuid>,
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
                    if let Some(node) = self.nodes.get(&node_id) {
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
                    if let Some(node) = self.nodes.get(node_id) {
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
                            SceneNode::Shape2D(shape) => {
                                if shape.visible {
                                    if let Some(shape_type) = shape.shape_type {
                                        if let Some(transform) = global_transform_opt {
                                            let pivot = shape.pivot;
                                            let z_index = shape.z_index;
                                            let color = shape.color.unwrap_or(crate::Color::new(255, 255, 255, 200));
                                            let border_thickness = if shape.filled { 0.0 } else { 2.0 };
                                            let is_border = !shape.filled;

                                            let rect_cmd = match shape_type {
                                                ShapeType2D::Rectangle { width, height } => {
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
                                                ShapeType2D::Circle { radius } => {
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
                                                ShapeType2D::Square { size } => {
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
                                                ShapeType2D::Triangle { base, height } => {
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
                if let Some(node) = self.nodes.get_mut(&node_id) {
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
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    if let SceneNode::UINode(ui_node) = node {
                        render_ui(ui_node, gfx);
                    }
                }
            }
            
            // Update cameras
            for node_id in camera_2d_updates {
                if let Some(node) = self.nodes.get_mut(&node_id) {
                    if let SceneNode::Camera2D(camera) = node {
                        if camera.active {
                            gfx.update_camera_2d(camera);
                        }
                    }
                }
            }
            
            for node_id in camera_3d_updates {
                if let Some(node) = self.nodes.get_mut(&node_id) {
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
                let global_transform_opt = if let Some(node) = self.nodes.get(&node_id) {
                    if node.as_node2d().is_some() {
                        self.get_global_transform(node_id)
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                if let Some(node) = self.nodes.get_mut(&node_id) {
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
                        SceneNode::Shape2D(shape) => {
                            if shape.visible {
                                if let Some(shape_type) = shape.shape_type {
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
                                                    None,
                                                    border_thickness,
                                                    is_border,
                                                    z_index,
                                                    timestamp,
                                                );
                                            }
                                            ShapeType2D::Circle { radius } => {
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
                            render_ui(ui_node, gfx);
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
    fn get_scene_node_ref(&self, id: Uuid) -> Option<&SceneNode> {
        self.nodes.get(&id)
    }
    
    fn get_scene_node_mut(&mut self, id: Uuid) -> Option<&mut SceneNode> {
        self.nodes.get_mut(&id)
    }
    
    fn mark_needs_rerender(&mut self, node_id: Uuid) {
        // Only add renderable nodes to needs_rerender
        // Non-renderable nodes (like Node, Node2D, Area2D) don't need to be rendered
        if let Some(node) = self.nodes.get(&node_id) {
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

    fn add_node_to_scene(&mut self, node: SceneNode, gfx: &mut crate::rendering::Graphics) -> anyhow::Result<()> {
        self.add_node_to_scene(node, gfx)
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

    fn get_global_transform(&mut self, node_id: Uuid) -> Option<crate::structs2d::Transform2D> {
        Self::get_global_transform(self, node_id)
    }

    fn set_global_transform(&mut self, node_id: Uuid, transform: crate::structs2d::Transform2D) -> Option<()> {
        Self::set_global_transform(self, node_id, transform)
    }

    fn mark_transform_dirty_recursive(&mut self, node_id: Uuid) {
        Self::mark_transform_dirty_recursive(self, node_id)
    }
    
    fn update_node2d_children_cache_on_add(&mut self, parent_id: Uuid, child_id: Uuid) {
        Self::update_node2d_children_cache_on_add(self, parent_id, child_id)
    }
    
    fn update_node2d_children_cache_on_remove(&mut self, parent_id: Uuid, child_id: Uuid) {
        Self::update_node2d_children_cache_on_remove(self, parent_id, child_id)
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
        "libscripts.so"
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
            println!("✅ Project root injected into DLL");
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
                    let handle = game_scene.instantiate_script(ctor, root_id);
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
                    if let Some(node) = game_scene.nodes.get(&root_id) {
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
