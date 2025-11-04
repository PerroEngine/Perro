#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use smallvec::{SmallVec, smallvec};
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
// TypesPup - Main Script Structure
// ========================================================================

pub struct TypesPupScript {
    node: Node2D,
    player_speed: f32,
    player_name: String,
    tags_array: Vec<Value>,
    stat_map: HashMap<String, Value>,
    meta_object: Value,
    current_score: f32,
}

// ========================================================================
// TypesPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn types_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(TypesPupScript {
        node: Node2D::new("TypesPup"),
        player_speed: 300f32,
        player_name: String::new(),
        tags_array: vec![json!(String::from("player")), json!(String::from("friendly"))],
        stat_map: HashMap::from([("strength".to_string(), json!(5f32)), ("agility".to_string(), json!(6f32))]),
        meta_object: json!({ "level": 3f32, "zone": String::from("forest") }),
        current_score: 0f32,
    })) as *mut dyn ScriptObject
}

// ========================================================================
// Supporting Struct Definitions
// ========================================================================

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub inventory_items: Vec<Value>,
    pub inventory_slots: i32,
}

impl std::fmt::Display for Inventory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "inventory_items: {:?}, ", self.inventory_items)?;
        write!(f, "inventory_slots: {:?} ", self.inventory_slots)?;
        write!(f, "}}")
    }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SuperInventory {
    pub base: Inventory,
    pub super_owner: String,
}

impl std::fmt::Display for SuperInventory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        // Flatten base Display
        let base_str = format!("{}", self.base);
        let base_inner = base_str.trim_matches(|c| c == '{' || c == '}').trim();
        if !base_inner.is_empty() {
            write!(f, "{}", base_inner)?;
            write!(f, ", ")?;
        }
        write!(f, "super_owner: {:?} ", self.super_owner)?;
        write!(f, "}}")
    }
}

impl std::ops::Deref for SuperInventory {
    type Target = Inventory;
    fn deref(&self) -> &Self::Target { &self.base }
}

impl std::ops::DerefMut for SuperInventory {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
}



// ========================================================================
// TypesPup - Script Init & Update Implementation
// ========================================================================

impl Script for TypesPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        self.current_score = 42f32;
        self.tags_array.push(json!(String::from("start")));
        let mut b = Vec::new();
        b.push(json!(String::from("fart")));
        b.push(json!(2f32));
        let mut t = 4i32;
        b.remove(0u32 as usize);
        b.remove(t as usize);
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}


impl ScriptObject for TypesPupScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node.id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node.id
    }

    fn get_var(&self, var_id: u64) -> Option<Value> {
        VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {
        for (var_id, val) in hashmap.iter() {
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {
                f(self, val);
            }
        }
    }

    fn call_function(&mut self, id: u64, api: &mut ScriptApi<'_>, params: &SmallVec<[Value; 3]>) {
        if let Some(f) = DISPATCH_TABLE.get(&id) {
            f(self, params, api);
        }
    }
}

// =========================== Static Dispatch Tables ===========================

static VAR_GET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&TypesPupScript) -> Option<Value>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&TypesPupScript) -> Option<Value>> =
        HashMap::with_capacity(6);
        m.insert(5896136722348243128u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.player_speed))
        });
        m.insert(1415871171432867950u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.player_name))
        });
        m.insert(12235209048792209390u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.tags_array))
        });
        m.insert(3846402511506249294u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.stat_map))
        });
        m.insert(1574000717034790106u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.meta_object))
        });
        m.insert(14355197762979970291u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.current_score))
        });
    m
});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut TypesPupScript, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut TypesPupScript, Value) -> Option<()>> =
        HashMap::with_capacity(6);
        m.insert(5896136722348243128u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_f64() {
                script.player_speed = v as f32;
                return Some(());
            }
            None
        });
        m.insert(1415871171432867950u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_str() {
                script.player_name = v.to_string();
                return Some(());
            }
            None
        });
        m.insert(12235209048792209390u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_array() {
                script.tags_array = v.clone();
                return Some(());
            }
            None
        });
        m.insert(3846402511506249294u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_object() {
                script.stat_map = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                return Some(());
            }
            None
        });
        m.insert(1574000717034790106u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_object() {
                script.meta_object = v.clone().into();
                return Some(());
            }
            None
        });
        m.insert(14355197762979970291u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_f64() {
                script.current_score = v as f32;
                return Some(());
            }
            None
        });
    m
});

static VAR_APPLY_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut TypesPupScript, &Value)>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut TypesPupScript, &Value)> =
        HashMap::with_capacity(6);
        m.insert(5896136722348243128u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_f64() {
                script.player_speed = v as f32;
            }
        });
        m.insert(1415871171432867950u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_str() {
                script.player_name = v.to_string();
            }
        });
    m
});

static DISPATCH_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64,
        fn(&mut TypesPupScript, &[Value], &mut ScriptApi<'_>)
    >
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m:
        HashMap<u64, fn(&mut TypesPupScript, &[Value], &mut ScriptApi<'_>)> =
        HashMap::with_capacity(2);
    m
});
