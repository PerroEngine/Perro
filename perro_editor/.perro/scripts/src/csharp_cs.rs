#![allow(improper_ctypes_definitions)]

#![allow(unused)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::{Value, json};
use uuid::Uuid;
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, nodes::* };

#[unsafe(no_mangle)]
pub extern "C" fn csharp_cs_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(CsharpCsScript {
        node_id: Uuid::nil(),
    })) as *mut dyn Script
}

pub struct CsharpCsScript {
    node_id: Uuid,
}

impl Script for CsharpCsScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        let mut self_node = api.get_node_clone::<Node>(&self.node_id);
        api.print("Hello World".to_string());
        self_node.name = "Bob".to_string();

        api.merge_nodes(vec![self_node.to_scene_node()]);
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
