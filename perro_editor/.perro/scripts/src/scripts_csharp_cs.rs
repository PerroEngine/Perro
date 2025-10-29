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
// ScriptsCsharpCs - Main Script Structure
// ========================================================================

pub struct ScriptsCsharpCsScript {
    node_id: Uuid,
}

// ========================================================================
// ScriptsCsharpCs - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn scripts_csharp_cs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ScriptsCsharpCsScript {
        node_id: Uuid::nil(),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// Supporting Struct Definitions
// ========================================================================

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub hp: i32,
    pub name: String,
}

impl Player {
    pub fn new() -> Self { Self::default() }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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



// ========================================================================
// ScriptsCsharpCs - Script Init & Update Implementation
// ========================================================================

impl Script for ScriptsCsharpCsScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print(String::from("Hello World I am csharp.cs").as_str());
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsCsharpCs - Script-Defined Methods
// ========================================================================

impl ScriptsCsharpCsScript {
    fn fart(&mut self, api: &mut ScriptApi<'_>) {
        api.print(String::from("Fart").as_str());
    }

}


impl ScriptObject for ScriptsCsharpCsScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
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
                _ => {},
            }
        }
    }
}
