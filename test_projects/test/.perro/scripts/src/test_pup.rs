#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
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
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("test_func"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("func"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("func"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("func"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("func"));
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// TestPup - Script-Defined Methods
// ========================================================================

impl TestPupScript {
    fn test_func(&mut self, mut b: f32, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_info(&format!("{} {}", String::from("test function called from the signal with paramter: "), b));
    }

    fn func(&mut self, mut db: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_warn(&format!("{} {}", String::from("what if i make it a string"), db));
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

    fn call_function(&mut self, name: &str, api: &mut ScriptApi<'_>, params: &Vec<Value>) {
        match name {
            "test_func" => {
                let b = params.get(0)
                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .unwrap_or_default() as f32;
                self.test_func(b, api, true);
            },
            "func" => {
                let db = params.get(0)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                self.func(db, api, true);
            },
            _ => {}
        }
    }
}
