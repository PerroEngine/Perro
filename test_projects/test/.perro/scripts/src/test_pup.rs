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
// TestPup - Main Script Structure
// ========================================================================

pub struct TestPupScript {
    node: Node,
}

// ========================================================================
// TestPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn test_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(TestPupScript {
        node: Node::new("TestPup", None),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// TestPup - Script Init & Update Implementation
// ========================================================================

impl Script for TestPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        api.emit_signal_id(18280479406616694178u64, smallvec![json!(String::from("greetC_1"))]);
    }

}

// ========================================================================
// TestPup - Script-Defined Methods
// ========================================================================

impl TestPupScript {
    fn on_from_a_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut count = m;
    }

    fn on_from_a_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut value = m;
    }

    fn on_from_a_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut buffer = m;
    }

    fn on_from_a_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut doubled = m;
    }

    fn on_from_a_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut score = String::from("score");
        api.print_info(&format!("{} {} {} {}", String::from("[C] got A_8:"), m, String::from("score:"), score));
    }

    fn on_from_a_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut hash = m;
    }

    fn on_from_a_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut result = m;
    }

    fn on_from_a_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut product = m;
    }

    fn on_from_a_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = String::from("bill");
        api.print_info(&format!("{} {}", String::from("[C] got B_1:"), m));

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut sum = String::from("100");
        api.print_info(&format!("{} {} {} {}", String::from("[C] got B_2:"), m, String::from("sum:"), sum));
    }

    fn on_from_b_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut remainder = m;
    }

    fn on_from_b_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut reversed = m;
    }

    fn on_from_b_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
    }

    fn on_from_b_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut power = m;
    }

    fn on_from_b_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
    }

    fn on_from_b_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut avg = m;
    }

    fn on_from_b_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
    }

    fn on_from_b_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut bitwise = m;
    }

    fn on_from_b_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

}


impl ScriptObject for TestPupScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node.id
    }

    fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Value) -> Option<()> {
        match name {
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
            _ => {}
        }
    }
}
