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
// ScriptsEditorPup - Main Script Structure
// ========================================================================

pub struct ScriptsEditorPupScript {
    node_id: Uuid,
    a: f32,
    bi: BigInt,
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
        let mut s = String::from("Hellow World");
        api.print(s.as_str());
        api.print(s.as_str());
        let mut dghj = 12f64;
        self.bi = BigInt::from(dghj as i64);
        let mut b = api.JSON.stringify(&json!({ "foo": 5f32 }));
        api.print(b.as_str());
        self.test_casting_behavior();
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsEditorPup - Script-Defined Methods
// ========================================================================

impl ScriptsEditorPupScript {
    fn fart(&mut self, api: &mut ScriptApi<'_>) {
        let mut s = 5f32;
        api.print(s);
    }

    fn test_casting_behavior(&mut self) {
        let mut i8_val = 10i8;
        let mut i16_val = 100i16;
        let mut i32_val = 1000i32;
        let mut i64_val = 10000i64;
        let mut u8_val = 10u8;
        let mut u16_val = 100u16;
        let mut u32_val = 1000u32;
        let mut u64_val = 10000u64;
        let mut i128_val = 100000000000000000i128;
        let mut u128_val = 200000000000000000u128;
        let mut f32_val = 1.5f32;
        let mut f64_val = 2.5f64;
        let mut d_val = Decimal::from_str("10.25347895848347638934783478943748376484734394445783874664").unwrap();
        let mut big_val = BigInt::from_str("12345678901234567890").unwrap();
        let mut str_val = String::from("hello");
        i16_val = (i8_val as i16);
        i32_val = (i16_val as i32);
        i64_val = (i32_val as i64);
        i128_val = (i64_val as i128);
        u16_val = (u8_val as u16);
        u32_val = (u16_val as u32);
        u64_val = (u32_val as u64);
        u128_val = (u64_val as u128);
        i64_val = (u32_val as i64);
        i128_val = (u64_val as i128);
        f32_val = (i32_val as f32);
        f64_val = (i64_val as f64);
        f32_val = (u16_val as f32);
        f64_val = (u32_val as f64);
        f64_val = (f32_val as f64);
        big_val = BigInt::from(i32_val);
        big_val = BigInt::from(u32_val);
        d_val = Decimal::from(i32_val);
        d_val = Decimal::from(u64_val);
        i8_val = big_val.to_i8().unwrap_or_default();
        i16_val = big_val.to_i16().unwrap_or_default();
        i32_val = big_val.to_i32().unwrap_or_default();
        i64_val = big_val.to_i64().unwrap_or_default();
        u8_val = big_val.to_u8().unwrap_or_default();
        u16_val = big_val.to_u16().unwrap_or_default();
        u32_val = big_val.to_u32().unwrap_or_default();
        u64_val = big_val.to_u64().unwrap_or_default();
        f32_val = big_val.to_f32().unwrap_or_default();
        f64_val = big_val.to_f64().unwrap_or_default();
        big_val = BigInt::from(f32_val as i32);
        big_val = BigInt::from(f64_val as i64);
        i32_val = (f32_val.round() as i32);
        i64_val = (f64_val.round() as i64);
        u32_val = (f32_val.round() as u32);
        u64_val = (f64_val.round() as u64);
        f32_val = (i16_val as f32);
        f64_val = (i32_val as f64);
        f64_val = (f32_val as f64);
        f32_val = (f64_val as f32);
        i32_val = d_val.to_i32().unwrap_or_default();
        u32_val = d_val.to_u32().unwrap_or_default();
        d_val = Decimal::from(i16_val);
        d_val = Decimal::from(u32_val);
        d_val = Decimal::from(big_val.to_i64().unwrap_or_default());
        str_val = i8_val.to_string();
        str_val = i16_val.to_string();
        str_val = i32_val.to_string();
        str_val = i64_val.to_string();
        str_val = u8_val.to_string();
        str_val = u16_val.to_string();
        str_val = u32_val.to_string();
        str_val = u64_val.to_string();
        str_val = i128_val.to_string();
        str_val = u128_val.to_string();
        str_val = f32_val.to_string();
        str_val = f64_val.to_string();
        str_val = d_val.to_string();
        str_val = big_val.to_string();
        i32_val = str_val.parse::<i32>().unwrap_or_default();
        f64_val = (str_val.parse::<f32>().unwrap_or_default() as f64);
        f32_val = (str_val.parse::<i32>().unwrap_or_default() as f32);
        d_val = Decimal::from_str(str_val.as_ref()).unwrap_or_default();
        big_val = BigInt::from_str(str_val.as_ref()).unwrap_or_default();
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
                _ => {},
            }
        }
    }
}
