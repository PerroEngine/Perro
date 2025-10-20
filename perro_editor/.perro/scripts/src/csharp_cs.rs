#![allow(improper_ctypes_definitions)]

#![allow(unused)]

use std::any::Any;

use std::collections::HashMap;
use serde_json::{Value, json};
use uuid::Uuid;
use std::ops::{Deref, DerefMut};
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

#[derive(Default, Debug, Clone)]
pub struct Player {
    pub hp: i32,
    pub name: String,
}

impl Player {
    pub fn new() -> Self { Self::default() }
}



#[derive(Default, Debug, Clone)]
pub struct Player2 {
    pub base: Player,
    pub hp1: i32,
    pub name1: String,
}

impl Player2 {
    pub fn new() -> Self { Self::default() }
}

impl Deref for Player2 {
    type Target = Player;
    fn deref(&self) -> &Self::Target { &self.base }
}

impl DerefMut for Player2 {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
}



#[derive(Default, Debug, Clone)]
pub struct Player3 {
    pub base: Player2,
    pub hp2: i32,
    pub name: String,
}

impl Player3 {
    pub fn new() -> Self { Self::default() }
}

impl Deref for Player3 {
    type Target = Player2;
    fn deref(&self) -> &Self::Target { &self.base }
}

impl DerefMut for Player3 {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
}



impl Script for CsharpCsScript {
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
