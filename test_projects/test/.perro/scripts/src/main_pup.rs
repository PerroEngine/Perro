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

// ========================================================================
// MainPup - Main Script Structure
// ========================================================================

pub struct MainPupScript {
    node: UINode,
    script_updates: i32,
}

// ========================================================================
// MainPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn main_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(MainPupScript {
        node: UINode::new("MainPup"),
        script_updates: 0i32,
    })) as *mut dyn ScriptObject
}

// ========================================================================
// MainPup - Script Init & Update Implementation
// ========================================================================

impl Script for MainPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        self.script_updates = 0i32;
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        api.emit_signal_id(299076141623528637u64, smallvec![json!(String::from("pingB_1"))]);
        self.script_updates += 1i32;
    }

}

// ========================================================================
// MainPup - Script-Defined Methods
// ========================================================================

impl MainPupScript {
    fn on_from_a_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_1:"), m));
    }

    fn on_from_a_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_2:"), m));
    }

    fn on_from_a_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_3:"), m));
    }

    fn on_from_a_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_4:"), m));
    }

    fn on_from_a_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_5:"), m));
    }

    fn on_from_a_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_6:"), m));
    }

    fn on_from_a_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_7:"), m));
    }

    fn on_from_a_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_8:"), m));
    }

    fn on_from_a_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_9:"), m));
    }

    fn on_from_a_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_10:"), m));
    }

    fn on_from_a_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_11:"), m));
    }

    fn on_from_a_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_12:"), m));
    }

    fn on_from_a_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_13:"), m));
    }

    fn on_from_a_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_14:"), m));
    }

    fn on_from_a_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_15:"), m));
    }

    fn on_from_c_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_1:"), m));
    }

    fn on_from_c_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_2:"), m));
    }

    fn on_from_c_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_3:"), m));
    }

    fn on_from_c_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_4:"), m));
    }

    fn on_from_c_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_5:"), m));
    }

    fn on_from_c_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_6:"), m));
    }

    fn on_from_c_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_7:"), m));
    }

    fn on_from_c_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_8:"), m));
    }

    fn on_from_c_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_9:"), m));
    }

    fn on_from_c_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_10:"), m));
    }

    fn on_from_c_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_11:"), m));
    }

    fn on_from_c_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_12:"), m));
    }

    fn on_from_c_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_13:"), m));
    }

    fn on_from_c_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_14:"), m));
    }

    fn on_from_c_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_15:"), m));
    }

}


impl ScriptObject for MainPupScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node.id
    }

    fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            "script_updates" => Some(json!(self.script_updates)),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Value) -> Option<()> {
        match name {
            "script_updates" => {
                if let Some(v) = val.as_i64() {
                    self.script_updates = v as i32;
                    return Some(());
                }
                None
            },
            _ => None,
        }
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<String, Value>) {
        for (key, _) in hashmap.iter() {
            match key.as_str() {
                _ => {}
            }
        }
    }

    fn call_function(&mut self, name: &str, api: &mut ScriptApi<'_>, params: &SmallVec<[Value; 3]>) {
        match name {
            "on_from_a_1" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_1(m, api, true);
            },
            "on_from_a_2" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_2(m, api, true);
            },
            "on_from_a_3" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_3(m, api, true);
            },
            "on_from_a_4" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_4(m, api, true);
            },
            "on_from_a_5" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_5(m, api, true);
            },
            "on_from_a_6" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_6(m, api, true);
            },
            "on_from_a_7" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_7(m, api, true);
            },
            "on_from_a_8" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_8(m, api, true);
            },
            "on_from_a_9" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_9(m, api, true);
            },
            "on_from_a_10" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_10(m, api, true);
            },
            "on_from_a_11" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_11(m, api, true);
            },
            "on_from_a_12" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_12(m, api, true);
            },
            "on_from_a_13" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_13(m, api, true);
            },
            "on_from_a_14" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_14(m, api, true);
            },
            "on_from_a_15" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_a_15(m, api, true);
            },
            "on_from_c_1" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_1(m, api, true);
            },
            "on_from_c_2" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_2(m, api, true);
            },
            "on_from_c_3" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_3(m, api, true);
            },
            "on_from_c_4" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_4(m, api, true);
            },
            "on_from_c_5" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_5(m, api, true);
            },
            "on_from_c_6" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_6(m, api, true);
            },
            "on_from_c_7" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_7(m, api, true);
            },
            "on_from_c_8" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_8(m, api, true);
            },
            "on_from_c_9" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_9(m, api, true);
            },
            "on_from_c_10" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_10(m, api, true);
            },
            "on_from_c_11" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_11(m, api, true);
            },
            "on_from_c_12" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_12(m, api, true);
            },
            "on_from_c_13" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_13(m, api, true);
            },
            "on_from_c_14" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_14(m, api, true);
            },
            "on_from_c_15" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_c_15(m, api, true);
            },
            _ => {}
        }
    }
}
