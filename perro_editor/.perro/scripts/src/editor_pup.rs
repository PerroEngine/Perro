#![allow(improper_ctypes_definitions)]

#![allow(unused)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::{Value, json};
use uuid::Uuid;
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, nodes::* };

#[unsafe(no_mangle)]
pub extern "C" fn editor_pup_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(EditorPupScript {
        node_id: Uuid::nil(),
    b: 0.0f32,
    })) as *mut dyn Script
}

pub struct EditorPupScript {
    node_id: Uuid,
    b: f32,
}

impl Script for EditorPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        let file = api.JSON.stringify(&json!({ "foo": 24f32 }));;
        let s = api.Time.get_datetime_string();;
        println!("bob is {} and {}", s, file);
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        let delta = api.delta();
        self.b = self.b + delta;
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
            "b" => Some(Var::F32(self.b)),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Var) -> Option<()> {
        match (name, val) {
            ("b", Var::F32(v)) => { self.b = v; Some(()) },
            _ => None,
        }
    }
}
