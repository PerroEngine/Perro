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
    arr_inferred: Vec<Value>,
    map_inferred: HashMap<String, Value>,
    arr_dynamic: Vec<Value>,
    map_dynamic: HashMap<String, Value>,
    arr_static: Vec<i32>,
    map_static: HashMap<String, f32>,
    score: i32,
    player_meta: Value,
}

// ========================================================================
// TypesPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn types_pup_create_script() -> *mut dyn ScriptObject {
    let node = Node2D::new("TypesPup");
    let arr_inferred = vec![json!(1f32), json!(2.5f32), json!(String::from("three"))];
    let map_inferred = HashMap::from([(String::from("hp"), json!(100f32)), (String::from("name"), json!(String::from("dog")))]);
    let arr_dynamic = vec![json!(String::from("mix")), json!(123f32), json!(true)];
    let map_dynamic = HashMap::from([(String::from("flag"), json!(true)), (String::from("val"), json!(55.5f32))]);
    let arr_static = vec![1i32, 2i32, 3i32, 4i32];
    let map_static = HashMap::from([(String::from("x"), 1.0f32), (String::from("y"), 2.5f32)]);
    let score = 42i32;
    let player_meta = json!({ "name": String::from("Doggo"), "lvl": 3f32 });

    Box::into_raw(Box::new(TypesPupScript {
        node,
        arr_inferred,
        map_inferred,
        arr_dynamic,
        map_dynamic,
        arr_static,
        map_static,
        score,
        player_meta,
    })) as *mut dyn ScriptObject
}

// ========================================================================
// Supporting Struct Definitions
// ========================================================================

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub hp: i32,
}

impl std::fmt::Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "name: {:?}, ", self.name)?;
        write!(f, "hp: {:?} ", self.hp)?;
        write!(f, "}}")
    }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SuperPlayer {
    pub base: Player,
    pub energy: f32,
}

impl std::fmt::Display for SuperPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        // Flatten base Display
        let base_str = format!("{}", self.base);
        let base_inner = base_str.trim_matches(|c| c == '{' || c == '}').trim();
        if !base_inner.is_empty() {
            write!(f, "{}", base_inner)?;
            write!(f, ", ")?;
        }
        write!(f, "energy: {:?} ", self.energy)?;
        write!(f, "}}")
    }
}

impl std::ops::Deref for SuperPlayer {
    type Target = Player;
    fn deref(&self) -> &Self::Target { &self.base }
}

impl std::ops::DerefMut for SuperPlayer {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SuperDuperPlayer {
    pub base: SuperPlayer,
    pub farting: bool,
}

impl std::fmt::Display for SuperDuperPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        // Flatten base Display
        let base_str = format!("{}", self.base);
        let base_inner = base_str.trim_matches(|c| c == '{' || c == '}').trim();
        if !base_inner.is_empty() {
            write!(f, "{}", base_inner)?;
            write!(f, ", ")?;
        }
        write!(f, "farting: {:?} ", self.farting)?;
        write!(f, "}}")
    }
}

impl std::ops::Deref for SuperDuperPlayer {
    type Target = SuperPlayer;
    fn deref(&self) -> &Self::Target { &self.base }
}

impl std::ops::DerefMut for SuperDuperPlayer {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.base }
}



// ========================================================================
// TypesPup - Script Init & Update Implementation
// ========================================================================

impl Script for TypesPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        self.test_dynamic_containers(api, false);
        self.test_static_containers(api, false);
        self.test_structs_array_map_mixed(api, false);
        self.test_casting(api, false);
        self.test_api(api, false);
        api.print(&String::from("-- INIT DONE --"));
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// TypesPup - Script-Defined Methods
// ========================================================================

impl TypesPupScript {
    fn test_dynamic_containers(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print(&String::from("-- DYNAMIC CONTAINERS --"));
        let mut sdp = SuperDuperPlayer { base: SuperPlayer { base: Player { name: String::from("bob"), hp: 2i32, ..Default::default() }, energy: 3f32, ..Default::default() }, ..Default::default() };
        let mut sp = SuperPlayer { base: Player { name: String::from("b"), hp: 2i32, ..Default::default() }, energy: 3f32, ..Default::default() };
        let mut spl = SuperDuperPlayer { base: SuperPlayer { base: Player { hp: 4i32, ..Default::default() }, ..Default::default() }, farting: true, ..Default::default() };
        let mut spll = SuperPlayer { base: Player { name: String::from("f"), ..Default::default() }, energy: 2f32, ..Default::default() };
        let mut dyn_arr = vec![json!(10f32), json!(20f32), json!(String::from("thirty"))];
        dyn_arr.push(json!(99f32));
        let mut val = (dyn_arr.get(0u32 as usize).cloned().unwrap_or_default() as i32);
        let mut strval = (dyn_arr.get(2u32 as usize).cloned().unwrap_or_default() as String);
        dyn_arr.remove(1u32 as usize);
        api.print(&format!("{} {} {} {}", String::from("dyn_arr val:"), val, String::from("str:"), strval));
        let mut dyn_arr2 = vec![json!(String::from("mix")), json!(55f32), json!(true)];
        dyn_arr2.push(json!(String::from("added")));
        let mut casted_val = (dyn_arr2.get(1u32 as usize).cloned().unwrap_or_default() as i32);
        api.print(&format!("{} {}", String::from("dyn_arr2 casted_val:"), casted_val));
        let mut dyn_map = HashMap::from([(String::from("a"), json!(10f32)), (String::from("b"), json!(20f32))]);
        dyn_map.insert(String::from("c"), json!(30f32));
        let mut retrieved = dyn_map.get(String::from("a").as_str()).cloned().unwrap_or_default().as_i64().unwrap_or_default() as i32;
        let mut has_c = dyn_map.contains_key(String::from("c").as_str());
        api.print(&format!("{} {} {} {}", String::from("dyn_map get(a):"), retrieved, String::from("has c:"), has_c));
        let mut dyn_map2 = HashMap::from([(String::from("num"), json!(5.5f32)), (String::from("str"), json!(String::from("hey")))]);
        dyn_map2.insert(String::from("extra"), json!(true));
        let mut s = dyn_map2.get(String::from("str").as_str()).cloned().unwrap_or_default().as_str().unwrap_or_default().to_string();
        api.print(&format!("{} {}", String::from("dyn_map2 str:"), s));
    }

    fn test_static_containers(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print(&String::from("-- STATIC CONTAINERS --"));
        let mut arr = vec![1i32, 2i32, 3i32];
        arr.push(json!(4f32));
        let mut first = arr.get(0u32 as usize).cloned().unwrap_or_default();
        api.print(&format!("{} {}", String::from("arr[0]:"), first));
        let mut map = HashMap::from([(String::from("x"), 1.1f32), (String::from("y"), 2.2f32)]);
        map.insert(String::from("z"), 3.3f32);
        let mut val = map.get(String::from("x").as_str()).cloned().unwrap_or_default();
        api.print(&format!("{} {}", String::from("map get(x):"), val));
        let mut p1 = Player { name: String::from("Pup"), hp: 120i32, ..Default::default() };
        let mut p2 = Player { name: String::from("Dog"), hp: 99i32, ..Default::default() };
        let mut players = HashMap::from([(String::from("one"), p1)]);
        players.insert(String::from("two"), p2);
        let mut second = players.get(String::from("two").as_str()).cloned().unwrap_or_default();
        api.print(&format!("{} {}", String::from("players(two).hp ="), second["hp"].clone()));
    }

    fn test_structs_array_map_mixed(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print(&String::from("-- STRUCT CONTAINERS --"));
        let mut p = SuperPlayer { base: Player { name: String::from("Hero"), hp: 150i32, ..Default::default() }, energy: 20.5f32, ..Default::default() };
        let mut dyn_arr = vec![json!(p)];
        let mut dyn_map = HashMap::from([(String::from("main"), json!(p))]);
        let mut p_name = serde_json::from_value::<SuperPlayer>(dyn_map.get(String::from("main").as_str()).cloned().unwrap_or_default().clone()).unwrap_or_default().name;
        api.print(&format!("{} {}", String::from("dynamic struct name:"), p_name));
        let mut arr_typed = vec![p];
        arr_typed.push(SuperPlayer { base: Player { name: String::from("Sidekick"), hp: 75i32, ..Default::default() }, energy: 8.8f32, ..Default::default() });
        let mut first = arr_typed.get(0u32 as usize).cloned().unwrap_or_default();
        api.print(&format!("{} {}", String::from("first static player:"), first.name));
        let mut map_typed = HashMap::from([(String::from("owner"), p)]);
        let mut o = map_typed.get(String::from("owner").as_str()).cloned().unwrap_or_default();
        api.print(&format!("{} {}", String::from("map_typed.owner.energy ="), o["energy"].clone()));
    }

    fn test_casting(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print(&String::from("-- CASTING --"));
        let mut i = 12i32;
        let mut f = i;
        let mut s = i.to_string();
        let mut back = String::from("55").parse::<i32>().unwrap_or_default();
        let mut obj = json!({ "val": 10f32 });
        let mut v = (obj[String::from("val")].clone() as i32);
        api.print(&format!("{} {} {} {} {} {} {} {} {} {}", String::from("i:"), i, String::from("f:"), f, String::from("s:"), s, String::from("back:"), back, String::from("v:"), v));
    }

    fn test_api(&mut self, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print(&String::from("-- API RUN --"));
        let mut platform = api.OS.get_platform_name();
        api.print(&format!("{} {}", String::from("Platform:"), platform));
        let mut now = api.Time.get_unix_time_msec();
        api.Time.sleep_msec(1u64);
        let mut delta = (api.Time.get_unix_time_msec() - now);
        api.print(&format!("{} {}", String::from("Delta:"), delta));
        let mut sig = 3707351006901076138u64;
        api.emit_signal_id(sig, smallvec![]);
        let mut js = api.JSON.stringify(&json!({ "data": 1f32 }));
        let mut parsed = api.JSON.parse(&js);
        api.print(&format!("{} {}", String::from("JSON round="), js));
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
        HashMap::with_capacity(8);
        m.insert(14974265476103193718u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.arr_inferred))
        });
        m.insert(10992709884624311409u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.map_inferred))
        });
        m.insert(14737855758307226574u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.arr_dynamic))
        });
        m.insert(13185196076233401175u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.map_dynamic))
        });
        m.insert(2179245173473647707u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.arr_static))
        });
        m.insert(13288395528568420568u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.map_static))
        });
        m.insert(13911166232573650165u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.score))
        });
        m.insert(7393797037455457872u64, |script: &TypesPupScript| -> Option<Value> {
            Some(json!(script.player_meta))
        });
    m
});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut TypesPupScript, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut TypesPupScript, Value) -> Option<()>> =
        HashMap::with_capacity(8);
        m.insert(14974265476103193718u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_array() {
                script.arr_inferred = v.clone();
                return Some(());
            }
            None
        });
        m.insert(10992709884624311409u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_object() {
                script.map_inferred = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                return Some(());
            }
            None
        });
        m.insert(14737855758307226574u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_array() {
                script.arr_dynamic = v.clone();
                return Some(());
            }
            None
        });
        m.insert(13185196076233401175u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_object() {
                script.map_dynamic = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                return Some(());
            }
            None
        });
        m.insert(2179245173473647707u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_array() {
                script.arr_static = v.clone();
                return Some(());
            }
            None
        });
        m.insert(13288395528568420568u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_object() {
                script.map_static = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                return Some(());
            }
            None
        });
        m.insert(13911166232573650165u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_i64() {
                script.score = v as i32;
                return Some(());
            }
            None
        });
        m.insert(7393797037455457872u64, |script: &mut TypesPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_object() {
                script.player_meta = v.clone().into();
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
        HashMap::with_capacity(8);
        m.insert(14974265476103193718u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_array() {
                script.arr_inferred = v.clone();
            }
        });
        m.insert(10992709884624311409u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_object() {
                script.map_inferred = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            }
        });
        m.insert(14737855758307226574u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_array() {
                script.arr_dynamic = v.clone();
            }
        });
        m.insert(13185196076233401175u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_object() {
                script.map_dynamic = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            }
        });
        m.insert(2179245173473647707u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_array() {
                script.arr_static = v.clone();
            }
        });
        m.insert(13288395528568420568u64, |script: &mut TypesPupScript, val: &Value| {
            if let Some(v) = val.as_object() {
                script.map_static = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
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
        HashMap::with_capacity(7);
        m.insert(15463363911213738560u64,
            |this: &mut TypesPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.test_dynamic_containers(api, true);
        });
        m.insert(16525919704344072013u64,
            |this: &mut TypesPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.test_static_containers(api, true);
        });
        m.insert(639163719116183971u64,
            |this: &mut TypesPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.test_structs_array_map_mixed(api, true);
        });
        m.insert(602436781337375691u64,
            |this: &mut TypesPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.test_casting(api, true);
        });
        m.insert(11998248306886128250u64,
            |this: &mut TypesPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
            this.test_api(api, true);
        });
    m
});
