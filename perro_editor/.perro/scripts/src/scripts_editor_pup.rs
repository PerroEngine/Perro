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
// ScriptsEditorPup - Main Script Structure
// ========================================================================

pub struct ScriptsEditorPupScript {
    node_id: Uuid,
    a: f32,
    bi: BigInt,
    dec: Decimal,
    c: Player,
}

// ========================================================================
// ScriptsEditorPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn scripts_editor_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ScriptsEditorPupScript {
        node_id: Uuid::nil(),
        a: 0.0f32,
        bi: BigInt::from_str("0").unwrap(),
        dec: Decimal::from_str("0").unwrap(),
        c: Default::default(),
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
// ScriptsEditorPup - Script Init & Update Implementation
// ========================================================================

impl Script for ScriptsEditorPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.print("Hello World I am editor.pup");
        self.bi = BigInt::from_str("").unwrap();
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsEditorPup - Script-Defined Methods
// ========================================================================

impl ScriptsEditorPupScript {
    fn test(&mut self) {
        let mut big_val = BigInt::from_str("").unwrap();
        let mut dec_val = Decimal::from_str("0").unwrap();
        let mut float_val = 0f32;
        let mut int_val = 0i32;
        big_val = BigInt::from_str("").unwrap();
        dec_val = Decimal::from_str("123.45").unwrap();
        float_val = int_val;
        int_val = (big_val as i32).to_i32().unwrap();
    }

    fn compute(&mut self, delta: f32) {
        let mut delta = delta;
        let mut poopy = 420000f32;
        self.a = 3f32;
        self.bi = BigInt::from_str("").unwrap();
        self.dec = Decimal::from_str("99").unwrap();
        self.a += (poopy as f32);
        self.bi += BigInt::from_str("").unwrap();
        self.dec += Decimal::from_str("0.5").unwrap();
        self.a = 5f32;
        self.bi += BigInt::from_str("").unwrap();
        self.dec += Decimal::from_str("1").unwrap();
    }

}


impl ScriptObject for ScriptsEditorPupScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
    }

    fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            "c" => Some(json!(self.c)),
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Value) -> Option<()> {
        match name {
            "c" => {
                if let Ok(v) = serde_json::from_value::<Player>(val) {
                    self.c = v;
                    return Some(());
                }
                None
            },
            _ => None,
        }
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<String, Value>) {
        for (key, _) in hashmap.iter() {
            match key.as_str() {
                "a" => {
                    if let Some(value) = hashmap.get("a") {
                        if let Some(v) = value.as_f64() {
                            self.a = v as f32;
                        }
                    }
                },
                "bi" => {
                    if let Some(value) = hashmap.get("bi") {
                        if let Some(v) = value.as_str() {
                            self.bi = v.parse::<BigInt>().unwrap();
                        }
                    }
                },
                "dec" => {
                    if let Some(value) = hashmap.get("dec") {
                        if let Some(v) = value.as_str() {
                            self.dec = v.parse::<Decimal>().unwrap();
                        }
                    }
                },
                _ => {},
            }
        }
    }
}
