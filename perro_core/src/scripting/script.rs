#![allow(improper_ctypes_definitions)]
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use crate::ids::NodeID;
use serde_json::Value;

use crate::SceneData;
use crate::api::ScriptApi;
use crate::fur_ast::FurElement;
use crate::node_registry::SceneNode;
use crate::scripting::app_command::AppCommand;

/// Bitflags to track which lifecycle methods are implemented by a script
/// This allows the engine to skip calling methods that are not implemented,
/// reducing overhead significantly when scripts only implement a subset of methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptFlags(u8);

impl ScriptFlags {
    pub const NONE: u8 = 0;
    pub const HAS_INIT: u8 = 1 << 0;
    pub const HAS_UPDATE: u8 = 1 << 1;
    pub const HAS_FIXED_UPDATE: u8 = 1 << 2;

    #[inline(always)]
    pub const fn new(flags: u8) -> Self {
        ScriptFlags(flags)
    }

    #[inline(always)]
    pub const fn has_init(self) -> bool {
        self.0 & Self::HAS_INIT != 0
    }

    #[inline(always)]
    pub const fn has_update(self) -> bool {
        self.0 & Self::HAS_UPDATE != 0
    }

    #[inline(always)]
    pub const fn has_fixed_update(self) -> bool {
        self.0 & Self::HAS_FIXED_UPDATE != 0
    }
}

pub trait ScriptProvider: Sync {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
    fn load_scene_data(&self, path: &str) -> io::Result<SceneData>;
    fn load_fur_data(&self, path: &str) -> io::Result<Vec<FurElement>>;

    /// Global script identifiers in deterministic order. Root = NodeID(1); first global = 2, etc.
    /// Used when building the scene to create global nodes and attach scripts in the correct order.
    fn get_global_registry_order(&self) -> &[&str] {
        &[]
    }

    /// Global display names from @global Name (same order as get_global_registry_order). Used for node names.
    fn get_global_registry_names(&self) -> &[&str] {
        &[]
    }
}

/// Trait implemented by all user scripts (dyn‑safe)
pub trait Script {
    fn init(&mut self, _api: &mut ScriptApi) {}
    fn update(&mut self, _api: &mut ScriptApi) {}
    fn fixed_update(&mut self, _api: &mut ScriptApi) {}
}

pub trait ScriptObject: Script {
    fn get_var(&self, var_id: u64) -> Option<Value>;
    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()>;
    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>);
    fn call_function(&mut self, func: u64, api: &mut ScriptApi, params: &[Value]);

    fn set_id(&mut self, id: NodeID);
    fn get_id(&self) -> NodeID;

    fn attributes_of(&self, member: &str) -> Vec<String>;
    fn members_with(&self, attribute: &str) -> Vec<String>;
    fn has_attribute(&self, member: &str, attribute: &str) -> bool;

    /// Returns bitflags indicating which lifecycle methods are implemented
    /// This allows the engine to skip calling empty/default implementations
    fn script_flags(&self) -> ScriptFlags;

    // Engine-facing init/update that calls the Script version
    fn engine_init(&mut self, api: &mut ScriptApi) {
        self.init(api)
    }
    fn engine_update(&mut self, api: &mut ScriptApi) {
        self.update(api)
    }
    fn engine_fixed_update(&mut self, api: &mut ScriptApi) {
        self.fixed_update(api)
    }
}

/// Function pointer type for script constructors
pub type CreateFn = extern "C" fn() -> *mut dyn ScriptObject;

use crate::input::joycon::ControllerManager;
use crate::input::manager::InputManager;
use crate::physics::physics_2d::PhysicsWorld2D;
use std::cell::RefCell as CellRefCell;
use std::sync::Mutex;

/// Trait object for scene access (dyn‑safe)
/// Direct borrowing with compile-time lifetime guarantees
pub trait SceneAccess {
    /// Get immutable reference to a node (can have multiple at once)
    fn get_scene_node_ref(&self, id: NodeID) -> Option<&SceneNode>;

    /// Get mutable reference to a node (compile-time borrow checking)
    fn get_scene_node_mut(&mut self, id: NodeID) -> Option<&mut SceneNode>;

    /// Mark a node as needing rerender (only if not already in set)
    /// This is used to track nodes needing rerender without iterating over all nodes
    fn mark_needs_rerender(&mut self, node_id: NodeID);

    /// Legacy method for compatibility
    fn get_scene_node(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        self.get_scene_node_mut(id)
    }

    /// Add a node to the scene. The arena always assigns the next available slot+generation.
    /// Returns the NodeID of the inserted node.
    fn add_node_to_scene(
        &mut self,
        node: SceneNode,
        gfx: &mut crate::rendering::Graphics,
    ) -> anyhow::Result<NodeID>;
    fn get_script(&mut self, id: NodeID) -> Option<Rc<UnsafeCell<Box<dyn ScriptObject>>>>;

    /// Get mutable reference to a script (for direct update calls)
    /// NOTE: This is now deprecated - use get_script() and UnsafeCell::get() instead
    fn get_script_mut(&mut self, id: NodeID) -> Option<Rc<UnsafeCell<Box<dyn ScriptObject>>>> {
        self.get_script(id)
    }

    /// Get a script by cloning its Rc (scripts are now always in memory)
    /// NOTE: This replaces take_script - scripts are no longer taken out
    fn take_script(&mut self, id: NodeID) -> Option<Rc<UnsafeCell<Box<dyn ScriptObject>>>> {
        self.get_script(id)
    }

    /// Put a script back into storage
    /// NOTE: This is now a no-op since scripts stay in memory, but kept for compatibility
    fn insert_script(&mut self, _id: NodeID, _script: Box<dyn ScriptObject>) {
        // Scripts are now stored as Rc<UnsafeCell<Box<>>>, so we don't need to insert them back
        // This method is kept for compatibility but does nothing
    }

    fn get_command_sender(&self) -> Option<&Sender<AppCommand>>;
    fn get_controller_manager(&self) -> Option<&Mutex<ControllerManager>>;
    fn enable_controller_manager(&self) -> bool;

    /// Update the Node2D children cache when a child is added
    fn update_node2d_children_cache_on_add(&mut self, parent_id: NodeID, child_id: NodeID);

    /// Update the Node2D children cache when a child is removed
    fn update_node2d_children_cache_on_remove(&mut self, parent_id: NodeID, child_id: NodeID);

    /// Clear the Node2D children cache when all children are removed
    fn update_node2d_children_cache_on_clear(&mut self, parent_id: NodeID);

    /// Get the next available node ID from the arena.
    /// Used in DLL mode to ensure unique IDs across DLLs.
    fn next_node_id(&mut self) -> NodeID;

    /// Remove a node from the scene, stopping rendering and cleaning up scripts
    fn remove_node(&mut self, node_id: NodeID, gfx: &mut crate::rendering::Graphics);

    fn get_input_manager(&self) -> Option<&Mutex<InputManager>>;
    fn get_physics_2d(&self) -> Option<&CellRefCell<PhysicsWorld2D>>;

    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
    fn instantiate_script(&mut self, ctor: CreateFn, node_id: NodeID) -> Box<dyn ScriptObject>;

    fn connect_signal_id(&mut self, signal: u64, target_id: NodeID, function: u64);

    /// Get signal connections for a signal ID (for direct emission within API context)
    fn get_signal_connections(
        &self,
        signal: u64,
    ) -> Option<&std::collections::HashMap<NodeID, smallvec::SmallVec<[u64; 4]>>>;

    /// Emit signal instantly (zero-allocation, direct call)
    /// Params are passed as compile-time slice, never stored
    fn emit_signal_id(&mut self, signal: u64, params: &[Value]);

    /// Emit signal deferred (queued, processed at end of frame)
    /// Use this when you need to emit during iteration or want frame-end processing
    fn emit_signal_id_deferred(&mut self, signal: u64, params: &[Value]);

    /// Legacy method - now calls emit_signal_id_deferred for safety
    fn queue_signal_id(&mut self, signal: u64, params: &[Value]) {
        self.emit_signal_id_deferred(signal, params);
    }

    /// Get the global transform for a node (calculates lazily if dirty)
    fn get_global_transform(&mut self, node_id: NodeID) -> Option<crate::structs2d::Transform2D>;

    /// Set the global transform for a node (marks it as dirty)
    fn set_global_transform(
        &mut self,
        node_id: NodeID,
        transform: crate::structs2d::Transform2D,
    ) -> Option<()>;

    /// Mark a node's transform as dirty (and all its children)
    fn mark_transform_dirty_recursive(&mut self, node_id: NodeID);
}
