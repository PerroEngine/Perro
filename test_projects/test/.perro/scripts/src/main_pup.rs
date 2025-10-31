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
// MainPup - Main Script Structure
// ========================================================================

pub struct MainPupScript {
    node: UINode,
}

// ========================================================================
// MainPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn main_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(MainPupScript {
        node: UINode::new("MainPup"),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// MainPup - Script Init & Update Implementation
// ========================================================================

impl Script for MainPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        self.node = api.get_node_clone::<UINode>(self.node.id);
        api.emit_signal(&String::from("TestSignal"), vec![json!(100f32)]);
        self.node.name = String::from("f");
        api.print(&self.node.name);

        api.merge_nodes(vec![self.node.clone().to_scene_node()]);
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        self.node = api.get_node_clone::<UINode>(self.node.id);
        self.ass(api, false);

        api.merge_nodes(vec![self.node.clone().to_scene_node()]);
    }

}

// ========================================================================
// MainPup - Script-Defined Methods
// ========================================================================

impl MainPupScript {
    fn fart(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<UINode>(self.node.id);
        }
        self.node.name = String::from("tim");

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn ass(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<UINode>(self.node.id);
        }
        self.fart(api, false);

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
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
            "fart" => {
                self.fart(api, true);
            },
            "ass" => {
                self.ass(api, true);
            },
            _ => {}
        }
    }
}
