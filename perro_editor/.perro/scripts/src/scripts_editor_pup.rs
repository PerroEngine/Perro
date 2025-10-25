#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use uuid::Uuid;
use std::ops::{Deref, DerefMut};
use std::{rc::Rc, cell::RefCell};

use perro_core::prelude::*;

// ========================================================================
// ScriptsEditorPup - Main Script Structure
// ========================================================================

pub struct ScriptsEditorPupScript {
    node_id: Uuid,
    a: f32,
}

// ========================================================================
// ScriptsEditorPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn scripts_editor_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ScriptsEditorPupScript {
        node_id: Uuid::nil(),
        a: 0.0f32,
    })) as *mut dyn ScriptObject
}

// ========================================================================
// Supporting Struct Definitions
// ========================================================================

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



// ========================================================================
// ScriptsEditorPup - Script Init & Update Implementation
// ========================================================================

impl Script for ScriptsEditorPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("Hello World I am editor.pup");
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsEditorPup - Script-Defined Methods
// ========================================================================

impl ScriptsEditorPupScript {
    fn bob(&mut self, poop: f32, bob: i32, james: String, api: &mut ScriptApi<'_>) {
        let mut poop = poop;
        let mut bob = bob;
        let mut james = james;
        api.JSON.parse("bob: 1");
    }

    fn foo(&mut self, api: &mut ScriptApi<'_>) {
        api.print("poop and fart");
    }

}


impl ScriptObject for ScriptsEditorPupScript {
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

    fn get_var(&self, name: &str) -> Option<&dyn Any> {
        match name {
            "a" => Some(&self.a as &dyn Any),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Box<dyn Any>) -> Option<()> {
        match name {
            "a" => {
                if let Ok(v) = val.downcast::<f32>() {
                    self.a = *v;
                    return Some(());
                }
                return None;
            },
            _ => None,
        }
    }

    fn apply_exports(&mut self, hashmap: &HashMap<String, Box<dyn Any>>) {
        for (key, _) in hashmap.iter() {
            match key.as_str() {
                _ => {},
            }
        }
    }
}
