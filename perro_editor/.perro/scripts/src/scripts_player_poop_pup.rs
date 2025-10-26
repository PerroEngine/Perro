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
use std::{rc::Rc, cell::RefCell};

use perro_core::prelude::*;

// ========================================================================
// ScriptsPlayerPoopPup - Main Script Structure
// ========================================================================

pub struct ScriptsPlayerPoopPupScript {
    node_id: Uuid,
}

// ========================================================================
// ScriptsPlayerPoopPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn scripts_player_poop_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ScriptsPlayerPoopPupScript {
        node_id: Uuid::nil(),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// Supporting Struct Definitions
// ========================================================================

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub hp: i32,
}

impl Player {
    pub fn new() -> Self { Self::default() }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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



// ========================================================================
// ScriptsPlayerPoopPup - Script Init & Update Implementation
// ========================================================================

impl Script for ScriptsPlayerPoopPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("Hello World");
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsPlayerPoopPup - Script-Defined Methods
// ========================================================================

impl ScriptsPlayerPoopPupScript {
    fn bob(&mut self, poop: f32, bob: i32, james: String, api: &mut ScriptApi<'_>) {
        let mut poop = poop;
        let mut bob = bob;
        let mut james = james;
        api.JSON.parse("bob: 1");
    }

}


impl ScriptObject for ScriptsPlayerPoopPupScript {
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
