#![allow(improper_ctypes_definitions)]
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Shr, Sub};
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use std::{fmt, io};

use serde_json::Value;
use smallvec::SmallVec;
use uuid::Uuid;

use crate::SceneData;
use crate::api::ScriptApi;
use crate::app_command::AppCommand;
use crate::fur_ast::FurElement;
use crate::node_registry::SceneNode;

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
    pub const HAS_DRAW: u8 = 1 << 3;
    
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
    
    #[inline(always)]
    pub const fn has_draw(self) -> bool {
        self.0 & Self::HAS_DRAW != 0
    }
}

pub trait ScriptProvider: Sync {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
    fn load_scene_data(&self, path: &str) -> io::Result<SceneData>;
    fn load_fur_data(&self, path: &str) -> io::Result<Vec<FurElement>>;
}

/// A dynamic variable type for script fields/exposed fields
#[derive(Clone, Debug)]
pub enum Var {
    F32(f32),
    I32(i32),
    Bool(bool),
    String(String),
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Var::F32(v) => write!(f, "{}", v),
            Var::I32(v) => write!(f, "{}", v),
            Var::Bool(v) => write!(f, "{}", v),
            Var::String(v) => write!(f, "{}", v),
        }
    }
}

/// Trait implemented by all user scripts (dyn‑safe)
pub trait Script {
    fn init(&mut self, api: &mut ScriptApi) {}
    fn update(&mut self, api: &mut ScriptApi) {}
    fn fixed_update(&mut self, _api: &mut ScriptApi) {}
    fn draw(&mut self, _api: &mut ScriptApi) {}
}

pub trait ScriptObject: Script {
    fn get_var(&self, var_id: u64) -> Option<Value>;
    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()>;
    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>);
    fn call_function(&mut self, func: u64, api: &mut ScriptApi, params: &[Value]);

    fn set_id(&mut self, id: Uuid);
    fn get_id(&self) -> Uuid;

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
    fn engine_draw(&mut self, api: &mut ScriptApi) {
        self.draw(api)
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
    fn get_scene_node_ref(&self, id: Uuid) -> Option<&SceneNode>;
    
    /// Get mutable reference to a node (compile-time borrow checking)
    fn get_scene_node_mut(&mut self, id: Uuid) -> Option<&mut SceneNode>;
    
    /// Legacy method for compatibility
    fn get_scene_node(&mut self, id: Uuid) -> Option<&mut SceneNode> {
        self.get_scene_node_mut(id)
    }
    
    fn add_node_to_scene(&mut self, node: SceneNode) -> anyhow::Result<()>;
    fn get_script(&mut self, id: Uuid) -> Option<&mut Box<dyn ScriptObject>>;
    
    /// Get mutable reference to a script (for direct update calls)
    fn get_script_mut(&mut self, id: Uuid) -> Option<&mut Box<dyn ScriptObject>>;
    
    /// Temporarily take a script out of storage, returns None if not found
    fn take_script(&mut self, id: Uuid) -> Option<Box<dyn ScriptObject>>;
    
    /// Put a script back into storage
    fn insert_script(&mut self, id: Uuid, script: Box<dyn ScriptObject>);
    
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>>;
    fn get_controller_manager(&self) -> Option<&Mutex<ControllerManager>>;
    fn get_input_manager(&self) -> Option<&Mutex<InputManager>>;
    fn get_physics_2d(&self) -> Option<&CellRefCell<PhysicsWorld2D>>;

    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
    fn instantiate_script(
        &mut self,
        ctor: CreateFn,
        node_id: Uuid,
    ) -> Box<dyn ScriptObject>;

    fn connect_signal_id(&mut self, signal: u64, target_id: Uuid, function: u64);
    
    /// Get signal connections for a signal ID (for direct emission within API context)
    fn get_signal_connections(&self, signal: u64) -> Option<&std::collections::HashMap<Uuid, smallvec::SmallVec<[u64; 4]>>>;
    
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
    fn get_global_transform(&mut self, node_id: Uuid) -> Option<crate::structs2d::Transform2D>;
    
    /// Set the global transform for a node (marks it as dirty)
    fn set_global_transform(&mut self, node_id: Uuid, transform: crate::structs2d::Transform2D) -> Option<()>;
    
    /// Mark a node's transform as dirty (and all its children)
    fn mark_transform_dirty_recursive(&mut self, node_id: Uuid);
}

//
// Operator implementations for Var
//

impl From<f32> for Var {
    fn from(v: f32) -> Self {
        Var::F32(v)
    }
}

impl From<i32> for Var {
    fn from(v: i32) -> Self {
        Var::I32(v)
    }
}

impl From<bool> for Var {
    fn from(v: bool) -> Self {
        Var::Bool(v)
    }
}

impl From<&str> for Var {
    fn from(v: &str) -> Self {
        Var::String(v.to_string())
    }
}

impl From<String> for Var {
    fn from(v: String) -> Self {
        Var::String(v)
    }
}

impl Add for Var {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // numeric coercion
            (Var::I32(a), Var::I32(b)) => Var::I32(a + b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a + b),
            (Var::I32(a), Var::F32(b)) => Var::F32(a as f32 + b),
            (Var::F32(a), Var::I32(b)) => Var::F32(a + b as f32),

            // string concatenation
            (Var::String(mut a), b) => {
                a.push_str(&b.to_string());
                Var::String(a)
            }
            (a, Var::String(mut b)) => {
                b.insert_str(0, &a.to_string());
                Var::String(b)
            }

            _ => panic!("Add not supported for variables provided"),
        }
    }
}

impl Sub for Var {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a - b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a - b),
            (Var::I32(a), Var::F32(b)) => Var::F32(a as f32 - b),
            (Var::F32(a), Var::I32(b)) => Var::F32(a - b as f32),
            _ => panic!("Sub not supported for variables provided"),
        }
    }
}

impl Mul for Var {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a * b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a * b),
            (Var::I32(a), Var::F32(b)) => Var::F32(a as f32 * b),
            (Var::F32(a), Var::I32(b)) => Var::F32(a * b as f32),
            _ => panic!("Mul not supported for variables provided"),
        }
    }
}

impl Div for Var {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a / b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a / b),
            (Var::I32(a), Var::F32(b)) => Var::F32(a as f32 / b),
            (Var::F32(a), Var::I32(b)) => Var::F32(a / b as f32),
            _ => panic!("Div not supported for variables provided"),
        }
    }
}

impl Rem for Var {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a % b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a % b),
            (Var::I32(a), Var::F32(b)) => Var::F32(a as f32 % b),
            (Var::F32(a), Var::I32(b)) => Var::F32(a % b as f32),
            _ => panic!("Rem not supported for variables provided"),
        }
    }
}

impl BitAnd for Var {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a & b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a & b),
            (Var::Bool(a), Var::I32(b)) => Var::I32((a as i32) & b),
            (Var::I32(a), Var::Bool(b)) => Var::I32(a & (b as i32)),
            _ => panic!("BitAnd not supported for variables provided"),
        }
    }
}

impl BitOr for Var {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a | b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a | b),
            (Var::Bool(a), Var::I32(b)) => Var::I32((a as i32) | b),
            (Var::I32(a), Var::Bool(b)) => Var::I32(a | (b as i32)),
            _ => panic!("BitOr not supported for variables provided"),
        }
    }
}

impl BitXor for Var {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a ^ b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a ^ b),
            (Var::Bool(a), Var::I32(b)) => Var::I32((a as i32) ^ b),
            (Var::I32(a), Var::Bool(b)) => Var::I32(a ^ (b as i32)),
            _ => panic!("BitXor not supported for variables provided"),
        }
    }
}

impl Shl for Var {
    type Output = Self;
    fn shl(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a << b),
            (Var::Bool(a), Var::I32(b)) => Var::I32((a as i32) << b),
            _ => panic!("Shl not supported for variables provided"),
        }
    }
}

impl Shr for Var {
    type Output = Self;
    fn shr(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a >> b),
            (Var::Bool(a), Var::I32(b)) => Var::I32((a as i32) >> b),
            _ => panic!("Shr not supported for variables provided"),
        }
    }
}
