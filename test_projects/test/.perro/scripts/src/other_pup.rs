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
// OtherPup - Main Script Structure
// ========================================================================

pub struct OtherPupScript {
    node: Node,
}

// ========================================================================
// OtherPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn other_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(OtherPupScript {
        node: Node::new("OtherPup", None),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// OtherPup - Script Init & Update Implementation
// ========================================================================

impl Script for OtherPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("bob"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("bob"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("bob"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("bob"));
        api.connect_signal(&String::from("TestSignal"), self.node.id, &String::from("bob"));
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// OtherPup - Script-Defined Methods
// ========================================================================

impl OtherPupScript {
    fn bob(&mut self, mut c: f32, api: &mut ScriptApi<'_>) {
        api.print_info(&format!("{} {}", String::from("Wow I am being called from a signal with this parameter: "), c));
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
            "bob" => {
                let c = params.get(0)
                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                    .unwrap_or_default() as f32;
                self.bob(c, api);
            },
            _ => {}
        }
    }
}
