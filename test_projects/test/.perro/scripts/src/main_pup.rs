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
// MainPup - Main Script Structure
// ========================================================================

pub struct MainPupScript {
    node: UINode,
    bob: String,
    script_updates: i32,
}

// ========================================================================
// MainPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn main_pup_create_script() -> *mut dyn ScriptObject {
    let node = UINode::new("MainPup");
    let bob = String::new();
    let script_updates = 0i32;

    Box::into_raw(Box::new(MainPupScript {
        node,
        bob,
        script_updates,
    })) as *mut dyn ScriptObject
}

// ========================================================================
// MainPup - Script Init & Update Implementation
// ========================================================================

impl Script for MainPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        self.script_updates = 0i32;
        let mut b = String::from("Fart");
        let mut a = 12f32;
        let mut s = a.to_string();
        a = 235f32;
        api.connect_signal_id(string_to_u64(&b), self.node.id, 6114839014446982845u64);
        api.connect_signal_id(string_to_u64(&s), self.node.id, 6114835715912098212u64);
        api.connect_signal_id(16982373988457314558u64, self.node.id, 6114836815423726423u64);
        api.connect_signal_id(16982373988457314558u64, self.node.id, 6114836815423726423u64);
        api.connect_signal_id(16982377286992199191u64, self.node.id, 6114833516888841790u64);
        api.connect_signal_id(16982376187480570980u64, self.node.id, 6114834616400470001u64);
        api.connect_signal_id(16982379486015455613u64, self.node.id, 6114831317865585368u64);
        api.connect_signal_id(16982378386503827402u64, self.node.id, 6114832417377213579u64);
        api.connect_signal_id(16982364092852660659u64, self.node.id, 6114829118842328946u64);
        api.connect_signal_id(16982362993341032448u64, self.node.id, 6114830218353957157u64);
        api.connect_signal_id(5532710869573397624u64, self.node.id, 157896838186582423u64);
        api.connect_signal_id(5532711969085025835u64, self.node.id, 157895738674954212u64);
        api.connect_signal_id(5532713068596654046u64, self.node.id, 157899037209838845u64);
        api.connect_signal_id(5532714168108282257u64, self.node.id, 157897937698210634u64);
        api.connect_signal_id(5532715267619910468u64, self.node.id, 157892440140069579u64);
        api.connect_signal_id(5532716367131538679u64, self.node.id, 157891340628441368u64);
        api.connect_signal_id(18280479406616694178u64, self.node.id, 4878057757827333139u64);
        api.connect_signal_id(18280478307105065967u64, self.node.id, 4878058857338961350u64);
        api.connect_signal_id(18280477207593437756u64, self.node.id, 4878059956850589561u64);
        api.connect_signal_id(18280476108081809545u64, self.node.id, 4878061056362217772u64);
        api.connect_signal_id(18280475008570181334u64, self.node.id, 4878062155873845983u64);
        api.connect_signal_id(18280473909058553123u64, self.node.id, 4878063255385474194u64);
        api.connect_signal_id(18280472809546924912u64, self.node.id, 4878064354897102405u64);
        api.connect_signal_id(18280489302221348077u64, self.node.id, 4878065454408730616u64);
        api.connect_signal_id(18280488202709719866u64, self.node.id, 4878066553920358827u64);
        api.connect_signal_id(5735078259587338006u64, self.node.id, 3100484552619093881u64);
        api.connect_signal_id(5735079359098966217u64, self.node.id, 3100483453107465670u64);
        api.connect_signal_id(5735076060564081584u64, self.node.id, 3100482353595837459u64);
        api.connect_signal_id(5735077160075709795u64, self.node.id, 3100481254084209248u64);
        api.connect_signal_id(5735082657633850850u64, self.node.id, 3100488950665606725u64);
        api.connect_signal_id(5735083757145479061u64, self.node.id, 3100487851153978514u64);
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        self.script_updates += 1i32;
    }

}

// ========================================================================
// MainPup - Script-Defined Methods
// ========================================================================

impl MainPupScript {
    fn on_from_a_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_1:"), m));
    }

    fn on_from_a_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_2:"), m));
    }

    fn on_from_a_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_3:"), m));
    }

    fn on_from_a_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_4:"), m));
    }

    fn on_from_a_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_5:"), m));
    }

    fn on_from_a_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_6:"), m));
    }

    fn on_from_a_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_7:"), m));
    }

    fn on_from_a_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_8:"), m));
    }

    fn on_from_a_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_9:"), m));
    }

    fn on_from_a_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_10:"), m));
    }

    fn on_from_a_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_11:"), m));
    }

    fn on_from_a_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_12:"), m));
    }

    fn on_from_a_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_13:"), m));
    }

    fn on_from_a_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_14:"), m));
    }

    fn on_from_a_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard A_15:"), m));
    }

    fn on_from_c_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_1:"), m));
    }

    fn on_from_c_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_2:"), m));
    }

    fn on_from_c_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_3:"), m));
    }

    fn on_from_c_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_4:"), m));
    }

    fn on_from_c_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_5:"), m));
    }

    fn on_from_c_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_6:"), m));
    }

    fn on_from_c_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_7:"), m));
    }

    fn on_from_c_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_8:"), m));
    }

    fn on_from_c_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_9:"), m));
    }

    fn on_from_c_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_10:"), m));
    }

    fn on_from_c_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_11:"), m));
    }

    fn on_from_c_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_12:"), m));
    }

    fn on_from_c_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_13:"), m));
    }

    fn on_from_c_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_14:"), m));
    }

    fn on_from_c_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        api.print_error(&format!("{} {}", String::from("[B] heard C_15:"), m));
    }

}


impl ScriptObject for MainPupScript {
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
    std::collections::HashMap<u64, fn(&MainPupScript) -> Option<Value>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&MainPupScript) -> Option<Value>> =
        HashMap::with_capacity(2);
        m.insert(16121189368928505687u64, |script: &MainPupScript| -> Option<Value> {
            Some(json!(script.script_updates))
        });
        m.insert(21748447695211092u64, |script: &MainPupScript| -> Option<Value> {
            Some(json!(script.bob))
        });
    m
});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut MainPupScript, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut MainPupScript, Value) -> Option<()>> =
        HashMap::with_capacity(2);
        m.insert(16121189368928505687u64, |script: &mut MainPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_i64() {
                script.script_updates = v as i32;
                return Some(());
            }
            None
        });
        m.insert(21748447695211092u64, |script: &mut MainPupScript, val: Value| -> Option<()> {
            if let Some(v) = val.as_str() {
                script.bob = v.to_string();
                return Some(());
            }
            None
        });
    m
});

static VAR_APPLY_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut MainPupScript, &Value)>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut MainPupScript, &Value)> =
        HashMap::with_capacity(2);
        m.insert(21748447695211092u64, |script: &mut MainPupScript, val: &Value| {
            if let Some(v) = val.as_str() {
                script.bob = v.to_string();
            }
        });
    m
});

static DISPATCH_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64,
        fn(&mut MainPupScript, &[Value], &mut ScriptApi<'_>)
    >
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m:
        HashMap<u64, fn(&mut MainPupScript, &[Value], &mut ScriptApi<'_>)> =
        HashMap::with_capacity(32);
        m.insert(6114839014446982845u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_1(m, api, true);
        });
        m.insert(6114835715912098212u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_2(m, api, true);
        });
        m.insert(6114836815423726423u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_3(m, api, true);
        });
        m.insert(6114833516888841790u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_4(m, api, true);
        });
        m.insert(6114834616400470001u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_5(m, api, true);
        });
        m.insert(6114831317865585368u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_6(m, api, true);
        });
        m.insert(6114832417377213579u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_7(m, api, true);
        });
        m.insert(6114829118842328946u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_8(m, api, true);
        });
        m.insert(6114830218353957157u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_9(m, api, true);
        });
        m.insert(157896838186582423u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_10(m, api, true);
        });
        m.insert(157895738674954212u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_11(m, api, true);
        });
        m.insert(157899037209838845u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_12(m, api, true);
        });
        m.insert(157897937698210634u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_13(m, api, true);
        });
        m.insert(157892440140069579u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_14(m, api, true);
        });
        m.insert(157891340628441368u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_15(m, api, true);
        });
        m.insert(4878057757827333139u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_1(m, api, true);
        });
        m.insert(4878058857338961350u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_2(m, api, true);
        });
        m.insert(4878059956850589561u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_3(m, api, true);
        });
        m.insert(4878061056362217772u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_4(m, api, true);
        });
        m.insert(4878062155873845983u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_5(m, api, true);
        });
        m.insert(4878063255385474194u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_6(m, api, true);
        });
        m.insert(4878064354897102405u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_7(m, api, true);
        });
        m.insert(4878065454408730616u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_8(m, api, true);
        });
        m.insert(4878066553920358827u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_9(m, api, true);
        });
        m.insert(3100484552619093881u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_10(m, api, true);
        });
        m.insert(3100483453107465670u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_11(m, api, true);
        });
        m.insert(3100482353595837459u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_12(m, api, true);
        });
        m.insert(3100481254084209248u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_13(m, api, true);
        });
        m.insert(3100488950665606725u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_14(m, api, true);
        });
        m.insert(3100487851153978514u64,
            |this: &mut MainPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_c_15(m, api, true);
        });
    m
});
