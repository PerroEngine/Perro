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
// ScriptsCsharpCs - Main Script Structure
// ========================================================================

pub struct ScriptsCsharpCsScript {
    node: Node,
}

// ========================================================================
// ScriptsCsharpCs - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn scripts_csharp_cs_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(ScriptsCsharpCsScript {
        node: Node::new("ScriptsCsharpCs", None),
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

impl std::fmt::Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "hp: {:?}, ", self.hp)?;
        write!(f, "name: {:?} ", self.name)?;
        write!(f, "}}")
    }
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

impl std::fmt::Display for Player2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "hp1: {:?}, ", self.hp1)?;
        write!(f, "name1: {:?} ", self.name1)?;
        write!(f, "}}")
    }
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

impl std::fmt::Display for Player3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "hp2: {:?}, ", self.hp2)?;
        write!(f, "name: {:?} ", self.name)?;
        write!(f, "}}")
    }
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
        api.print(&String::from("Hello World I am csharp.cs"));
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// ScriptsCsharpCs - Script-Defined Methods
// ========================================================================

impl ScriptsCsharpCsScript {
    fn fart(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print(&String::from("Fart"));
    }

}


impl ScriptObject for ScriptsCsharpCsScript {
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
    std::collections::HashMap<u64, fn(&ScriptsCsharpCsScript) -> Option<Value>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&ScriptsCsharpCsScript) -> Option<Value>> =
        HashMap::with_capacity(0);
    m
});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut ScriptsCsharpCsScript, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut ScriptsCsharpCsScript, Value) -> Option<()>> =
        HashMap::with_capacity(0);
    m
});

static VAR_APPLY_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut ScriptsCsharpCsScript, &Value)>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut ScriptsCsharpCsScript, &Value)> =
        HashMap::with_capacity(0);
    m
});

static DISPATCH_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64,
        fn(&mut ScriptsCsharpCsScript, &[Value], &mut ScriptApi<'_>)
    >
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m:
        HashMap<u64, fn(&mut ScriptsCsharpCsScript, &[Value], &mut ScriptApi<'_>)> =
        HashMap::with_capacity(3);
        m.insert(17246073498204514350u64,
            |this: &mut ScriptsCsharpCsScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.fart(api, true);
        });
    m
});
