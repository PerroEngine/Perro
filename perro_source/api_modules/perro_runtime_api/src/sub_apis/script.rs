//! Runtime script API.
//!
//! Attaches and detaches scripts, controls script schedules, accesses script
//! vars/methods, and exposes typed state closures.

use perro_ids::{NodeID, ScriptMemberID};
use perro_resource_api::ResPathSource;
use perro_variant::Variant;
use std::borrow::Cow;

pub trait IntoScriptMemberID {
    fn into_script_member(self) -> ScriptMemberID;
}

impl IntoScriptMemberID for ScriptMemberID {
    fn into_script_member(self) -> ScriptMemberID {
        self
    }
}

impl IntoScriptMemberID for &str {
    fn into_script_member(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self)
    }
}

impl IntoScriptMemberID for String {
    fn into_script_member(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self.as_str())
    }
}

impl IntoScriptMemberID for &String {
    fn into_script_member(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self.as_str())
    }
}

impl IntoScriptMemberID for Cow<'_, str> {
    fn into_script_member(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self.as_ref())
    }
}

impl IntoScriptMemberID for &Cow<'_, str> {
    fn into_script_member(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self.as_ref())
    }
}

pub trait ScriptAPI {
    fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V;
    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V;
    fn script_attach(&mut self, node_id: NodeID, script_path: &str) -> bool;
    fn script_attach_with_vars(
        &mut self,
        node_id: NodeID,
        script_path: &str,
        vars: Vec<(ScriptMemberID, Variant)>,
    ) -> bool {
        let ok = self.script_attach(node_id, script_path);
        if ok {
            for (member, value) in vars {
                self.set_var(node_id, member, value);
            }
        }
        ok
    }
    fn script_attach_hashed(&mut self, node_id: NodeID, script_path_hash: u64) -> bool;
    fn script_detach(&mut self, node_id: NodeID) -> bool;
    fn remove_script(&mut self, script_id: NodeID) -> bool;
    fn script_set_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool;
    fn script_set_fixed_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool;
    fn get_var(&mut self, script_id: NodeID, member: ScriptMemberID) -> Variant;
    fn set_var(&mut self, script_id: NodeID, member: ScriptMemberID, value: Variant);

    fn call_method(
        &mut self,
        script_id: NodeID,
        method: ScriptMemberID,
        params: &[Variant],
    ) -> Variant;
}

pub struct ScriptModule<'rt, R: ScriptAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: ScriptAPI + ?Sized> ScriptModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        self.rt.with_state(script_id, f)
    }

    pub fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        self.rt.with_state_mut(script_id, f)
    }

    pub fn script_attach<P: ResPathSource>(&mut self, node_id: NodeID, script_path: P) -> bool {
        self.rt
            .script_attach(node_id, script_path.as_res_path_str())
    }

    pub fn script_attach_hashed(&mut self, node_id: NodeID, script_path_hash: u64) -> bool {
        self.rt.script_attach_hashed(node_id, script_path_hash)
    }

    pub fn script_detach(&mut self, node_id: NodeID) -> bool {
        self.rt.script_detach(node_id)
    }

    pub fn remove(&mut self, script_id: NodeID) -> bool {
        self.rt.remove_script(script_id)
    }

    pub fn set_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool {
        self.rt.script_set_update_enabled(script_id, enabled)
    }

    pub fn set_fixed_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool {
        self.rt.script_set_fixed_update_enabled(script_id, enabled)
    }

    pub fn get_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M) -> Variant {
        self.rt.get_var(script_id, member.into_script_member())
    }

    pub fn set_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M, value: Variant) {
        self.rt
            .set_var(script_id, member.into_script_member(), value);
    }

    pub fn call_method<M: IntoScriptMemberID>(
        &mut self,
        script_id: NodeID,
        method: M,
        params: &[Variant],
    ) -> Variant {
        self.rt
            .call_method(script_id, method.into_script_member(), params)
    }
}

/// Script state macros.
///
/// These macros provide typed access to script state through closure-scoped borrows.
///
/// Typed read access to script state through a closure.
/// Returns `None` if `id` is invalid or state type does not match `state_ty`.
///
/// Internals:
/// - The runtime resolves `script_id`, downcasts to `state_ty`, then calls your closure.
/// - The state reference is only valid inside the closure, preventing leaked borrows.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `state_ty`: concrete script state type
/// - `id`: script `NodeID`
/// - closure arg: `&state_ty`
#[macro_export]
macro_rules! with_state {
    ($ctx:expr, $state_ty:ty, $id:expr, $f:expr) => {
        $ctx.Scripts().with_state::<$state_ty, _, _>($id, $f)
    };
}

/// Typed mutable access to script state through a closure.
///
/// Internals:
/// - The runtime resolves `script_id`, downcasts to `state_ty`, then calls your closure with `&mut`.
/// - Mutable access is scoped to closure execution, keeping aliasing rules enforced.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `state_ty`: concrete script state type
/// - `id`: script `NodeID`
/// - closure arg: `&mut state_ty`
#[macro_export]
macro_rules! with_state_mut {
    ($ctx:expr, $state_ty:ty, $id:expr, $f:expr) => {
        $ctx.Scripts().with_state_mut::<$state_ty, _, _>($id, $f)
    };
}

/// Script lifecycle macros.
///
/// Attaches a script resource to a scene node.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `id`: target node `NodeID`
/// - `path`: script path (for example `"res://scripts/foo.rs"`)
#[macro_export]
macro_rules! script_attach {
    ($ctx:expr, $id:expr, $path:literal) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $ctx.Scripts().script_attach_hashed($id, __PATH_HASH)
    }};
    ($ctx:expr, $id:expr, $path:expr) => {
        $ctx.Scripts().script_attach($id, $path)
    };
}

/// Detaches the current script from a scene node.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `id`: target node `NodeID`
#[macro_export]
macro_rules! script_detach {
    ($ctx:expr, $id:expr) => {
        $ctx.Scripts().script_detach($id)
    };
}

/// Enables or disables script `on_update` scheduling.
///
/// Returns `true` when schedule state changed.
#[macro_export]
macro_rules! script_set_update_enabled {
    ($ctx:expr, $id:expr, $enabled:expr) => {
        $ctx.Scripts().set_update_enabled($id, $enabled)
    };
}

/// Enables or disables script `on_fixed_update` scheduling.
///
/// Returns `true` when schedule state changed.
#[macro_export]
macro_rules! script_set_fixed_update_enabled {
    ($ctx:expr, $id:expr, $enabled:expr) => {
        $ctx.Scripts().set_fixed_update_enabled($id, $enabled)
    };
}

/// Script member access macros.
///
/// Gets a script variable by member identifier.
///
/// Signature:
/// - `get_var!(&mut RuntimeWindow<_, _>, NodeID, ScriptMemberID) -> Variant`
///
/// Usage:
/// - `get_var!(ctx, node_id, var!("health")) -> Variant`
/// - `get_var!(ctx, node_id, "health") -> Variant`
/// - `get_var!(ctx, node_id, dynamic_name_string) -> Variant`
///
/// Accepted member inputs:
/// - `var!("...")`, `ScriptMemberID`, `&str`, `String`, `Cow<str>`
#[macro_export]
macro_rules! get_var {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().get_var($id, $member)
    };
}

/// Reads a node-ref script var, returning `NodeID::nil()` when the var is
/// missing or is not a node reference.
///
/// Signature:
/// - `get_node_var!(&mut RuntimeWindow<_, _>, NodeID, member) -> NodeID`
///
/// Usage:
/// - `get_node_var!(ctx, root, var!("pause_panel")) -> NodeID`
/// - `get_node_var!(ctx, root, "pause_panel") -> NodeID`
///
/// Accepted member inputs:
/// - `var!("...")`, `ScriptMemberID`, `&str`, `String`, `Cow<str>`
#[macro_export]
macro_rules! get_node_var {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().get_var($id, $member).as_node_or_nil()
    };
}

/// Sets a script variable by member identifier.
///
/// Signature:
/// - `set_var!(&mut RuntimeWindow<_, _>, NodeID, ScriptMemberID, Variant) -> ()`
///
/// Usage:
/// - `set_var!(ctx, node_id, var!("health"), variant!(100_i32)) -> ()`
/// - `set_var!(ctx, node_id, "health", variant!(100_i32)) -> ()`
/// - `set_var!(ctx, node_id, dynamic_name_string, resolved_value) -> ()`
///
/// Accepted member inputs:
/// - `var!("...")`, `ScriptMemberID`, `&str`, `String`, `Cow<str>`
#[macro_export]
macro_rules! set_var {
    ($ctx:expr, $id:expr, $member:expr, $value:expr) => {
        $ctx.Scripts().set_var($id, $member, $value)
    };
}

/// Calls a script method with params.
///
/// Signature:
/// - `call_method!(&mut RuntimeWindow<_, _>, NodeID, ScriptMemberID, &[Variant]) -> Variant`
///
/// Usage:
/// - `call_method!(ctx, node_id, method!("take_damage"), params![10_i32]) -> Variant`
/// - `call_method!(ctx, node_id, func!("take_damage"), params![10_i32]) -> Variant`
/// - `call_method!(ctx, node_id, "take_damage", params![10_i32]) -> Variant`
/// - `call_method!(ctx, node_id, dynamic_name_string, &values) -> Variant`
///
/// Accepted method inputs:
/// - `method!("...")`, `func!("...")`, `ScriptMemberID`, `&str`, `String`, `Cow<str>`
#[macro_export]
macro_rules! call_method {
    ($ctx:expr, $id:expr, $method:expr, $params:expr) => {
        $ctx.Scripts().call_method($id, $method, $params)
    };
}
