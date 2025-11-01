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
// OtherPup - Main Script Structure
// ========================================================================

pub struct OtherPupScript {
    node: Node,
    x: f32,
    y: i32,
}

// ========================================================================
// OtherPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn other_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(OtherPupScript {
        node: Node::new("OtherPup", None),
        x: 0.0f32,
        y: 0i32,
    })) as *mut dyn ScriptObject
}

// ========================================================================
// OtherPup - Script Init & Update Implementation
// ========================================================================

impl Script for OtherPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        api.emit_signal_id(16982371789434058136u64, smallvec![json!(String::from("helloA_1"))]);
    }

}

// ========================================================================
// OtherPup - Script-Defined Methods
// ========================================================================

impl OtherPupScript {
    fn on_from_b_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_1:"), m));
    }

    fn on_from_b_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_2:"), m));
    }

    fn on_from_b_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_3:"), m));
    }

    fn on_from_b_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_4:"), m));
    }

    fn on_from_b_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_5:"), m));
    }

    fn on_from_b_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_6:"), m));
    }

    fn on_from_b_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_7:"), m));
    }

    fn on_from_b_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_8:"), m));
    }

    fn on_from_b_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_9:"), m));
    }

    fn on_from_b_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_10:"), m));
    }

    fn on_from_b_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_11:"), m));
    }

    fn on_from_b_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_12:"), m));
    }

    fn on_from_b_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_13:"), m));
    }

    fn on_from_b_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_14:"), m));
    }

    fn on_from_b_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got B_15:"), m));
    }

    fn on_from_c_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_1:"), m));
    }

    fn on_from_c_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_2:"), m));
    }

    fn on_from_c_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_3:"), m));
    }

    fn on_from_c_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_4:"), m));
    }

    fn on_from_c_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_5:"), m));
    }

    fn on_from_c_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_6:"), m));
    }

    fn on_from_c_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_7:"), m));
    }

    fn on_from_c_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_8:"), m));
    }

    fn on_from_c_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_9:"), m));
    }

    fn on_from_c_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_10:"), m));
    }

    fn on_from_c_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_11:"), m));
    }

    fn on_from_c_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_12:"), m));
    }

    fn on_from_c_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_13:"), m));
    }

    fn on_from_c_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_14:"), m));
    }

    fn on_from_c_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("[A] got C_15:"), m));
    }

}


impl ScriptObject for OtherPupScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node.id
    }

    fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            "x" => Some(json!(self.x)),
            "y" => Some(json!(self.y)),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Value) -> Option<()> {
        match name {
            "x" => {
                if let Some(v) = val.as_f64() {
                    self.x = v as f32;
                    return Some(());
                }
                None
            },
            "y" => {
                if let Some(v) = val.as_i64() {
                    self.y = v as i32;
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
            "on_from_b_1" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_1(m, api, true);
            },
            "on_from_b_2" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_2(m, api, true);
            },
            "on_from_b_3" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_3(m, api, true);
            },
            "on_from_b_4" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_4(m, api, true);
            },
            "on_from_b_5" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_5(m, api, true);
            },
            "on_from_b_6" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_6(m, api, true);
            },
            "on_from_b_7" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_7(m, api, true);
            },
            "on_from_b_8" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_8(m, api, true);
            },
            "on_from_b_9" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_9(m, api, true);
            },
            "on_from_b_10" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_10(m, api, true);
            },
            "on_from_b_11" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_11(m, api, true);
            },
            "on_from_b_12" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_12(m, api, true);
            },
            "on_from_b_13" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_13(m, api, true);
            },
            "on_from_b_14" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_14(m, api, true);
            },
            "on_from_b_15" => {
                let m = params.get(0)
                    .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                    .unwrap_or_default();
                self.on_from_b_15(m, api, true);
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
