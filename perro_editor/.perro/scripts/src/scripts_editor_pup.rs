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
// ScriptsEditorPup - Main Script Structure
// ========================================================================

pub struct ScriptsEditorPupScript {
    node: Node,
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
        node: Node::new("ScriptsEditorPup", None),
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

impl std::fmt::Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "hp: {:?} ", self.hp)?;
        write!(f, "}}")
    }
}

impl Player {
    pub fn new() -> Self { Self::default() }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub base: Player,
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "}}")
    }
}

impl Stats {
    pub fn new() -> Self { Self::default() }
    fn heal(&mut self, mut amt: i32, api: &mut ScriptApi<'_>, external_call: bool) {
        api.JSON.stringify(&&json!({ "hp": amt }));
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
        api.print(&s);
        api.print(&s);
        let mut dghj = 12f64;
        self.bi = BigInt::from(dghj as i64);
        let mut b = api.JSON.stringify(&&json!({ "foo": 5f32 }));
        api.print(&b);
        self.test_casting_behavior(api, false);
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsEditorPup - Script-Defined Methods
// ========================================================================

impl ScriptsEditorPupScript {
    fn fart(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut s = 5f32;
        api.print(&s);
    }

    fn test_casting_behavior(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
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
    std::collections::HashMap<u64, fn(&ScriptsEditorPupScript) -> Option<Value>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&ScriptsEditorPupScript) -> Option<Value>> =
        HashMap::with_capacity(3);
        m.insert(12638187200555641996u64, |script: &ScriptsEditorPupScript| -> Option<Value> {
            Some(json!(script.a))
        });
        m.insert(623250502729981348u64, |script: &ScriptsEditorPupScript| -> Option<Value> {
            Some(json!(script.bi))
        });
        m.insert(12638189399578898418u64, |script: &ScriptsEditorPupScript| -> Option<Value> {
            Some(json!(script.c))
        });
    m
});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut ScriptsEditorPupScript, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut ScriptsEditorPupScript, Value) -> Option<()>> =
        HashMap::with_capacity(3);
        m.insert(12638187200555641996u64, |script: &mut ScriptsEditorPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_f64() {
                script.a = v as f32;
                return Some(());
            }
            None
        });
        m.insert(623250502729981348u64, |script: &mut ScriptsEditorPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_str() {
                script.bi = v.parse::<BigInt>().unwrap();
                return Some(());
            }
            None
        });
        m.insert(12638189399578898418u64, |script: &mut ScriptsEditorPupScript, val: Value| -> Option<()> {
            if let Ok(v) = serde_json::from_value::<Player>(val) {
                script.c = v;
                return Some(());
            }
            None
        });
    m
});

static VAR_APPLY_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut ScriptsEditorPupScript, &Value)>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut ScriptsEditorPupScript, &Value)> =
        HashMap::with_capacity(3);
        m.insert(12638187200555641996u64, |script: &mut ScriptsEditorPupScript, val: &Value| {
            if let Some(v) = val.as_f64() {
                script.a = v as f32;
            }
        });
        m.insert(623250502729981348u64, |script: &mut ScriptsEditorPupScript, val: &Value| {
            if let Some(v) = val.as_str() {
                script.bi = v.parse::<BigInt>().unwrap();
            }
        });
    m
});

static DISPATCH_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64,
        fn(&mut ScriptsEditorPupScript, &[Value], &mut ScriptApi<'_>)
    >
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m:
        HashMap<u64, fn(&mut ScriptsEditorPupScript, &[Value], &mut ScriptApi<'_>)> =
        HashMap::with_capacity(4);
        m.insert(17246073498204514350u64,
            |this: &mut ScriptsEditorPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.fart(api, true);
        });
        m.insert(3588896087067325934u64,
            |this: &mut ScriptsEditorPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.test_casting_behavior(api, true);
        });
    m
});
