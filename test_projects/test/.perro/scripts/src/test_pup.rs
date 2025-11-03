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
// TestPup - Main Script Structure
// ========================================================================

pub struct TestPupScript {
    node: Node,
}

// ========================================================================
// TestPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn test_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(TestPupScript {
        node: Node::new("TestPup", None),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// TestPup - Script Init & Update Implementation
// ========================================================================

impl Script for TestPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        api.connect_signal_id(16982371789434058136u64, self.node.id, 6114839014446982845u64);
        api.connect_signal_id(16982375087968942769u64, self.node.id, 6114835715912098212u64);
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
        api.connect_signal_id(299076141623528637u64, self.node.id, 4351390588547960728u64);
        api.connect_signal_id(299072843088644004u64, self.node.id, 4351393887082845361u64);
        api.connect_signal_id(299073942600272215u64, self.node.id, 4351392787571217150u64);
        api.connect_signal_id(299070644065387582u64, self.node.id, 4351396086106101783u64);
        api.connect_signal_id(299071743577015793u64, self.node.id, 4351394986594473572u64);
        api.connect_signal_id(299068445042131160u64, self.node.id, 4351398285129358205u64);
        api.connect_signal_id(299069544553759371u64, self.node.id, 4351397185617729994u64);
        api.connect_signal_id(299066246018874738u64, self.node.id, 4351382891966563251u64);
        api.connect_signal_id(299067345530502949u64, self.node.id, 4351381792454935040u64);
        api.connect_signal_id(5856347817197736855u64, self.node.id, 18281003964271794808u64);
        api.connect_signal_id(5856346717686108644u64, self.node.id, 18281005063783423019u64);
        api.connect_signal_id(5856350016220993277u64, self.node.id, 18281006163295051230u64);
        api.connect_signal_id(5856348916709365066u64, self.node.id, 18281007262806679441u64);
        api.connect_signal_id(5856343419151224011u64, self.node.id, 18281008362318307652u64);
        api.connect_signal_id(5856342319639595800u64, self.node.id, 18281009461829935863u64);
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}

// ========================================================================
// TestPup - Script-Defined Methods
// ========================================================================

impl TestPupScript {
    fn on_from_a_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut count = m;
    }

    fn on_from_a_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut value = m;
    }

    fn on_from_a_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut buffer = m;
    }

    fn on_from_a_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut doubled = m;
    }

    fn on_from_a_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut score = String::from("score");
        api.print_info(&format!("{} {} {} {}", String::from("[C] got A_8:"), m, String::from("score:"), score));
    }

    fn on_from_a_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut hash = m;
    }

    fn on_from_a_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut result = m;
    }

    fn on_from_a_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_a_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut product = m;
    }

    fn on_from_a_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_1(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = String::from("bill");
        api.print_info(&format!("{} {}", String::from("[C] got B_1:"), m));

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_2(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut sum = String::from("100");
        api.print_info(&format!("{} {} {} {}", String::from("[C] got B_2:"), m, String::from("sum:"), sum));
    }

    fn on_from_b_3(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_4(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut remainder = m;
    }

    fn on_from_b_5(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut reversed = m;
    }

    fn on_from_b_6(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
    }

    fn on_from_b_7(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_8(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut power = m;
    }

    fn on_from_b_9(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_10(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
    }

    fn on_from_b_11(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

    fn on_from_b_12(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut avg = m;
    }

    fn on_from_b_13(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
    }

    fn on_from_b_14(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        let mut bitwise = m;
    }

    fn on_from_b_15(&mut self, mut m: String, api: &mut ScriptApi<'_>, external_call: bool) {
        if external_call {
            self.node = api.get_node_clone::<Node>(self.node.id);
        }
        self.node.name = m;

        if external_call {
            api.merge_nodes(vec![self.node.clone().to_scene_node()]);
        }
    }

}


impl ScriptObject for TestPupScript {
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
    std::collections::HashMap<u64, fn(&TestPupScript) -> Option<Value>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&TestPupScript) -> Option<Value>> =
        HashMap::with_capacity(0);
    m
});

static VAR_SET_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut TestPupScript, Value) -> Option<()>>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut TestPupScript, Value) -> Option<()>> =
        HashMap::with_capacity(0);
    m
});

static VAR_APPLY_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64, fn(&mut TestPupScript, &Value)>
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m: HashMap<u64, fn(&mut TestPupScript, &Value)> =
        HashMap::with_capacity(0);
    m
});

static DISPATCH_TABLE: once_cell::sync::Lazy<
    std::collections::HashMap<u64,
        fn(&mut TestPupScript, &[Value], &mut ScriptApi<'_>)
    >
> = once_cell::sync::Lazy::new(|| {
    use std::collections::HashMap;
    let mut m:
        HashMap<u64, fn(&mut TestPupScript, &[Value], &mut ScriptApi<'_>)> =
        HashMap::with_capacity(32);
        m.insert(6114839014446982845u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_1(m, api, true);
        });
        m.insert(6114835715912098212u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_2(m, api, true);
        });
        m.insert(6114836815423726423u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_3(m, api, true);
        });
        m.insert(6114833516888841790u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_4(m, api, true);
        });
        m.insert(6114834616400470001u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_5(m, api, true);
        });
        m.insert(6114831317865585368u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_6(m, api, true);
        });
        m.insert(6114832417377213579u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_7(m, api, true);
        });
        m.insert(6114829118842328946u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_8(m, api, true);
        });
        m.insert(6114830218353957157u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_9(m, api, true);
        });
        m.insert(157896838186582423u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_10(m, api, true);
        });
        m.insert(157895738674954212u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_11(m, api, true);
        });
        m.insert(157899037209838845u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_12(m, api, true);
        });
        m.insert(157897937698210634u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_13(m, api, true);
        });
        m.insert(157892440140069579u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_14(m, api, true);
        });
        m.insert(157891340628441368u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_a_15(m, api, true);
        });
        m.insert(4351390588547960728u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_1(m, api, true);
        });
        m.insert(4351393887082845361u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_2(m, api, true);
        });
        m.insert(4351392787571217150u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_3(m, api, true);
        });
        m.insert(4351396086106101783u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_4(m, api, true);
        });
        m.insert(4351394986594473572u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_5(m, api, true);
        });
        m.insert(4351398285129358205u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_6(m, api, true);
        });
        m.insert(4351397185617729994u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_7(m, api, true);
        });
        m.insert(4351382891966563251u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_8(m, api, true);
        });
        m.insert(4351381792454935040u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_9(m, api, true);
        });
        m.insert(18281003964271794808u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_10(m, api, true);
        });
        m.insert(18281005063783423019u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_11(m, api, true);
        });
        m.insert(18281006163295051230u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_12(m, api, true);
        });
        m.insert(18281007262806679441u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_13(m, api, true);
        });
        m.insert(18281008362318307652u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_14(m, api, true);
        });
        m.insert(18281009461829935863u64,
            |this: &mut TestPupScript, params: &[Value], api: &mut ScriptApi<'_>| {
let m = params.get(0)
                .and_then(|v| serde_json::from_value::<String>(v.clone()).ok())
                .unwrap_or_default();
            this.on_from_b_15(m, api, true);
        });
    m
});
