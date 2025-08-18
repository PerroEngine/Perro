#![allow(improper_ctypes_definitions)]
use std::{any::Any, collections::HashMap};
use std::ops::{Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Shl, Shr};

use serde_json::Value;
use uuid::Uuid;

use crate::api::ScriptApi;

#[derive(Clone, Debug)]
pub enum Var {
  F32(f32),
  I32(i32),
  Bool(bool),
  String(String),
}

pub enum UpdateOp {
    Set,     // =
    Add,     // +=
    Sub,     // -=
    Mul,     // *=
    Div,     // /=
    Rem,     // %=
    And,     // &=
    Or,      // |=
    Xor,     // ^=
    Shl,     // <<=
    Shr,     // >>=
}

pub trait Script {
    fn init(&mut self, api: &mut ScriptApi);

    fn update(&mut self, api: &mut ScriptApi);
    
    fn set_node_id(&mut self, id: Uuid);
    fn get_node_id(&self) -> Uuid;

    fn apply_exports(&mut self, exports: &HashMap<String, Value>);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn get_var(&self, name: &str) -> Option<Var>;
    fn set_var(&mut self, name: &str, val: Var) -> Option<()>;


}

pub type CreateFn = unsafe extern "C" fn() -> *mut dyn Script;




impl Add for Var {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a + b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a + b),
            (Var::String(mut a), Var::String(b)) => {
                a.push_str(&b);
                Var::String(a)
            }
            _ => panic!("Add operation not supported for these Var types"),
        }
    }
}

impl Sub for Var {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a - b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a - b),
            _ => panic!("Sub operation not supported for these Var types"),
        }
    }
}

impl Mul for Var {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a * b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a * b),
            _ => panic!("Mul operation not supported for these Var types"),
        }
    }
}

impl Div for Var {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a / b),
            (Var::F32(a), Var::F32(b)) => Var::F32(a / b),
            _ => panic!("Div operation not supported for these Var types"),
        }
    }
}

impl Rem for Var {
    type Output = Self;
    fn rem(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a % b),
            _ => panic!("Rem operation not supported for these Var types"),
        }
    }
}

impl BitAnd for Var {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a & b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a & b),
            _ => panic!("BitAnd operation not supported for these Var types"),
        }
    }
}

impl BitOr for Var {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a | b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a | b),
            _ => panic!("BitOr operation not supported for these Var types"),
        }
    }
}

impl BitXor for Var {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a ^ b),
            (Var::Bool(a), Var::Bool(b)) => Var::Bool(a ^ b),
            _ => panic!("BitXor operation not supported for these Var types"),
        }
    }
}

impl Shl for Var {
    type Output = Self;
    fn shl(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a << b),
            _ => panic!("Shl operation not supported for these Var types"),
        }
    }
}

impl Shr for Var {
    type Output = Self;
    fn shr(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Var::I32(a), Var::I32(b)) => Var::I32(a >> b),
            _ => panic!("Shr operation not supported for these Var types"),
        }
    }
}