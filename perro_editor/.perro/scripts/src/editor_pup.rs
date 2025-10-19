#![allow(improper_ctypes_definitions)]

#![allow(unused)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::{Value, json};
use uuid::Uuid;
use std::ops::{Deref, DerefMut};
use perro_core::{script::{UpdateOp, Var}, scripting::api::ScriptApi, scripting::script::Script, nodes::* };

#[unsafe(no_mangle)]
pub extern "C" fn editor_pup_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(EditorPupScript {
        node_id: Uuid::nil(),
    })) as *mut dyn Script
}

pub struct EditorPupScript {
    node_id: Uuid,
}

#[derive(Default, Debug, Clone)]
pub struct Player {
    pub hp: i32,
}

impl Player {
    pub fn new() -> Self { Self::default() }
}



#[derive(Default, Debug, Clone)]
pub struct Stats {
    pub base: Player,
}

impl Stats {
    pub fn new() -> Self { Self::default() }
    fn heal(&mut self, amt: i32, api: &mut ScriptApi<'_>) {
        let mut amt = amt;
        api.JSON.stringify(&json!({ "hp": amt }));
    }

}

impl Deref for Stats {
    type Target = Player;
    fn deref(&self) -> &Self::Target { &self.base }
}

impl DerefMut for Stats {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
}



impl Script for EditorPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("Hello World".to_string());
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
impl EditorPupScript {
    fn bob(&mut self, poop: f32, bob: i32, james: String) {
        let mut poop = poop;
        let mut bob = bob;
        let mut james = james;
    }

}
