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

/// @PerroScript
pub struct RustScript {
    node: Node,
    pub x: f32,
    /// @expose
    y: i32,
    /// @expose
    pub z: i32
}

// ========================================================================
// OtherPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn rust_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(RustScript {
        node: Node::new("OtherPup", None),
        x: 0.0f32,
        y: 0i32,
        z: 0i32
    })) as *mut dyn ScriptObject
}

// ========================================================================
// OtherPup - Script Init & Update Implementation
// ========================================================================

impl Script for RustScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
    
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
    }

}
