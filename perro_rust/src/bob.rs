#![allow(improper_ctypes_definitions)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::Value;
use uuid::Uuid;
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, Sprite2D };

#[unsafe(no_mangle)]
pub extern "C" fn bob_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(BobScript {
        node_id: Uuid::nil(),
        foo: 0.0f32,
    })) as *mut dyn Script
}

pub struct BobScript {
    node_id: Uuid,
    pub foo: f32,
}

impl Script for BobScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        let delta = api.get_delta();
        let self_node = api.get_node_mut::<Sprite2D>(&self.node_id).unwrap();
        self_node.transform.position.x = self.foo * delta;
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
            "foo" => Some(Var::F32(self.foo)),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Var) -> Option<()> {
        match (name, val) {
            ("foo", Var::F32(v)) => { self.foo = v; Some(()) },
            _ => None,
        }
    }
}
