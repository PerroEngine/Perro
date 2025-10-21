#![allow(improper_ctypes_definitions)]
use std::fmt;
use std::{any::Any, collections::HashMap, cell::RefCell, rc::Rc};
use std::ops::{Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr};
use std::sync::mpsc::{Sender, Receiver, channel};

use serde_json::Value;
use uuid::Uuid;

use crate::app_command::AppCommand;
use crate::scene_node::SceneNode;

pub trait ScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
}

/// A dynamic variable type for script fields/exports
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
    fn init(&mut self, api: &mut crate::api::ScriptApi);
    fn update(&mut self, api: &mut crate::api::ScriptApi);

    fn set_node_id(&mut self, id: Uuid);
    fn get_node_id(&self) -> Uuid;

    fn apply_exports(&mut self, exports: &HashMap<String, Value>);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn get_var(&self, name: &str) -> Option<Var>;
    fn set_var(&mut self, name: &str, val: Var) -> Option<()>;
}

/// Function pointer type for script constructors
pub type CreateFn = extern "C" fn() -> *mut dyn Script;

/// Trait object for scene access (dyn‑safe)
pub trait SceneAccess {
    fn get_scene_node(&mut self, id: &Uuid) -> Option<&mut SceneNode>;
    fn merge_nodes(&mut self, nodes: Vec<SceneNode>);
    fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()>;
    fn get_script(&self, id: Uuid) -> Option<Rc<RefCell<Box<dyn Script>>>>;
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>>;

    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
    fn instantiate_script(&mut self, ctor: CreateFn, node_id: Uuid) -> Rc<RefCell<Box<dyn Script>>>;
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
