#![allow(improper_ctypes_definitions)]
use std::{any::Any, collections::HashMap, cell::RefCell, rc::Rc};
use std::ops::{Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr};
use std::sync::mpsc::{Sender, Receiver, channel};

use serde_json::Value;
use uuid::Uuid;

use crate::app_command::AppCommand;

/// A dynamic variable type for script fields/exports
#[derive(Clone, Debug)]
pub enum Var {
    F32(f32),
    I32(i32),
    Bool(bool),
    String(String),
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
    fn get_node_mut_any(&mut self, id: &Uuid) -> Option<&mut dyn Any>;
    fn update_script_var(
        &mut self,
        node_id: &Uuid,
        name: &str,
        op: UpdateOp,
        val: Var,
    ) -> Option<()>;
    fn get_script(&self, id: Uuid) -> Option<Rc<RefCell<Box<dyn Script>>>>;
    fn get_command_sender(&self) -> Option<&Sender<AppCommand>>;
}

//
// Operator implementations for Var
//

impl Add for Var {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a + b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a + b),
            (Var::String(mut a), Var::String(b)) => { a.push_str(&b); Var::String(a) }
            _ => panic!("Add not supported"),
        }
    }
}

impl Sub for Var {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a - b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a - b),
            _ => panic!("Sub not supported"),
        }
    }
}

impl Mul for Var {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a * b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a * b),
            _ => panic!("Mul not supported"),
        }
    }
}

impl Div for Var {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a / b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a / b),
            _ => panic!("Div not supported"),
        }
    }
}

impl Rem for Var {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a % b),
            _ => panic!("Rem not supported"),
        }
    }
}

impl BitAnd for Var {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a & b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a & b),
            _ => panic!("BitAnd not supported"),
        }
    }
}

impl BitOr for Var {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a | b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a | b),
            _ => panic!("BitOr not supported"),
        }
    }
}

impl BitXor for Var {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a ^ b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a ^ b),
            _ => panic!("BitXor not supported"),
        }
    }
}

impl Shl for Var {
    type Output = Self;
    fn shl(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a << b),
            _ => panic!("Shl not supported"),
        }
    }
}

impl Shr for Var {
    type Output = Self;
    fn shr(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a >> b),
            _ => panic!("Shr not supported"),
        }
    }
}