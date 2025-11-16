#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use smallvec::{SmallVec, smallvec};
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::ops::{Deref, DerefMut};
use rust_decimal::{Decimal, prelude::*};
use num_bigint::BigInt;
use std::str::FromStr;
use std::{rc::Rc, cell::RefCell};

use perro_core::prelude::*;

/// @PerroScript
pub struct RustScript {
    node: Node,
    pub x: f32,
    /// @expose
    y: i32,
    /// @expose
    pub z: i32
}

// ========================================================================
// OtherPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn rust_rs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(RustScript {
        node: Node::new("OtherPup", None),
        x: 0.0f32,
        y: 0i32,
        z: 0i32
    })) as *mut dyn ScriptObject
}

// ========================================================================
// OtherPup - Script Init & Update Implementation
// ========================================================================

impl Script for RustScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}



impl ScriptObject for RustScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node.id
    }

    fn get_var(&self, var_id: u64) -> Option<Value> {
            VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {
        for (var_id, val) in hashmap.iter() {
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {
                f(self, val);
            }
        }
    }

    fn call_function(&mut self, id: u64, api: &mut ScriptApi<'_>, params: &SmallVec<[Value; 3]>) {
        if let Some(f) = DISPATCH_TABLE.get(&id) {
            f(self, params, api);
        }
    }
}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&RustScript) -> Option<Value>> = phf::phf_map! {
        12638216887369603693u64 => |script: &RustScript| -> Option<Value> {
                        Some(json!(script.z))
                    },
        12638214688346347271u64 => |script: &RustScript| -> Option<Value> {
                        Some(json!(script.x))
                    },
};

static VAR_SET_TABLE: phf::Map<u64, fn(&mut RustScript, Value) -> Option<()>> = phf::phf_map! {
        12638216887369603693u64 => |script: &mut RustScript, val: Value| -> Option<()> {
                            if let Some(v) = val.as_i64() {
                                script.z = v as i32;
                                return Some(());
                            }
                            None
                        },
        12638214688346347271u64 => |script: &mut RustScript, val: Value| -> Option<()> {
                            if let Some(v) = val.as_f64() {
                                script.x = v as f32;
                                return Some(());
                            }
                            None
                        },
};

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut RustScript, &Value)> = phf::phf_map! {
        12638213588834719060u64 => |script: &mut RustScript, val: &Value| {
                            if let Some(v) = val.as_i64() {
                                script.y = v as i32;
                            }
                        },
        12638216887369603693u64 => |script: &mut RustScript, val: &Value| {
                            if let Some(v) = val.as_i64() {
                                script.z = v as i32;
                            }
                        },
};

static DISPATCH_TABLE: phf::Map<u64, fn(&mut RustScript, &[Value], &mut ScriptApi<'_>)> = phf::phf_map! {
};