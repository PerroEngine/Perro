#![allow(improper_ctypes_definitions)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::Value;
use uuid::Uuid;
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, Sprite2D };

#[unsafe(no_mangle)]
pub extern "C" fn pup_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(PupScript {
        node_id: Uuid::nil(),
    foo: 0.0f32,
    bob: "hello".to_string(),
    bar: 12,
    })) as *mut dyn Script
}

pub struct PupScript {
    node_id: Uuid,
    pub foo: f32,
    pub bob: String,
    pub bar: i32,
}

impl Script for PupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        let delta = api.get_delta();
        let self_node = api.get_node_mut::<Sprite2D>(&self.node_id).unwrap();
        self_node.transform.position.x += 2f32 * delta;
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
        self.foo = hashmap.get("foo").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(0.0);
    }

    fn get_var(&self, name: &str) -> Option<Var> {
        match name {
            "foo" => Some(Var::F32(self.foo)),
            "bob" => Some(Var::String(self.bob.clone())),
            "bar" => Some(Var::I32(self.bar)),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Var) -> Option<()> {
        match (name, val) {
            ("foo", Var::F32(v)) => { self.foo = v; Some(()) },
            ("bob", Var::String(v)) => { self.bob = v; Some(()) },
            ("bar", Var::I32(v)) => { self.bar = v; Some(()) },
            _ => None,
        }
    }
}
