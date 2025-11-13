#![allow(improper_ctypes_definitions)]
use std::{fmt, io};
use std::{any::Any, collections::HashMap, cell::RefCell, rc::Rc};
use std::ops::{Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr};
use std::sync::mpsc::{Sender};

use serde_json::Value;
use smallvec::SmallVec;
use uuid::Uuid;

use crate::SceneData;
use crate::api::ScriptApi;
use crate::app_command::AppCommand;
use crate::ast::FurElement;
use crate::node_registry::SceneNode;

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

/// Update operations for script variables
pub enum UpdateOp {
    Set, Add, Sub, Mul, Div, Rem,
    And, Or, Xor, Shl, Shr,
}

/// Trait implemented by all user scripts (dyn‑safe)
pub trait Script {
    fn init(&mut self, api: &mut ScriptApi);
    fn update(&mut self, api: &mut ScriptApi);
}

pub trait ScriptObject: Script {
    fn get_var(&self, var_id: u64) -> Option<Value>;
    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()>;
    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>);
    fn call_function(&mut self, func: u64, api: &mut ScriptApi, params: &SmallVec<[Value; 3]>);

    fn set_node_id(&mut self, id: Uuid);
    fn get_node_id(&self) -> Uuid;

    // Engine-facing init/update that calls the Script version
    fn engine_init(&mut self, api: &mut ScriptApi) {
        self.init(api)
    }
    fn engine_update(&mut self, api: &mut ScriptApi) {
        self.update(api)
    }
}



/// Function pointer type for script constructors
pub type CreateFn = extern "C" fn() -> *mut dyn ScriptObject;

/// Trait object for scene access (dyn‑safe)
pub trait SceneAccess {
    fn get_scene_node(&mut self, id: Uuid) -> Option<&mut SceneNode>;
    fn merge_nodes(&mut self, nodes: Vec<SceneNode>);
    fn get_script(&self, id: Uuid) -> Option<Rc<RefCell<Box<dyn ScriptObject>>>>;
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>>;

    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
    fn instantiate_script(&mut self, ctor: CreateFn, node_id: Uuid) -> Rc<RefCell<Box<dyn ScriptObject>>>;

    fn connect_signal_id(&mut self, signal: u64, target_id: Uuid, function: u64);
    fn queue_signal_id(&mut self, signal: u64, params: SmallVec<[Value; 3]>);

}

//
// Operator implementations for Var
//

impl From<f32> for Var {
    fn from(v: f32) -> Self { Var::F32(v) }
}

impl From<i32> for Var {
    fn from(v: i32) -> Self { Var::I32(v) }
}

impl From<bool> for Var {
    fn from(v: bool) -> Self { Var::Bool(v) }
}

impl From<&str> for Var {
    fn from(v: &str) -> Self { Var::String(v.to_string()) }
}

impl From<String> for Var {
    fn from(v: String) -> Self { Var::String(v) }
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
            (Var::String(mut a), b) => { a.push_str(&b.to_string()); Var::String(a) }
            (a, Var::String(mut b)) => { b.insert_str(0, &a.to_string()); Var::String(b) }

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
