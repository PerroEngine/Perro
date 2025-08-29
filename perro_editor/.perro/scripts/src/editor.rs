#![allow(improper_ctypes_definitions)]

#![allow(unused)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::Value;
use uuid::Uuid;
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, Node };

#[unsafe(no_mangle)]
pub extern "C" fn editor_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(EditorScript {
        node_id: Uuid::nil(),
    })) as *mut dyn Script
}

pub struct EditorScript {
    node_id: Uuid,
}

impl Script for EditorScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }
    fn apply_exports(&mut self, hashmap: &std::collections::HashMap<String, serde_json::Value>) {
    }

    fn get_var(&self, name: &str) -> Option<Var> {
        match name {
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Var) -> Option<()> {
        match (name, val) {
            _ => None,
        }
    }
}
