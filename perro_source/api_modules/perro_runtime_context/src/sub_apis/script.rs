use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Attribute(&'static str);

impl Attribute {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl AsRef<str> for Attribute {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<&'static str> for Attribute {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Member {
    name: &'static str,
    id: ScriptMemberID,
}

impl Member {
    pub const fn new(name: &'static str) -> Self {
        let id = ScriptMemberID::from_string(name);
        Self { name, id }
    }

    pub const fn as_str(&self) -> &'static str {
        self.name
    }

    pub const fn id(&self) -> ScriptMemberID {
        self.id
    }
}

impl AsRef<str> for Member {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<&'static str> for Member {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

pub trait IntoScriptMemberID {
    fn into_script_member(self) -> ScriptMemberID;
}

impl IntoScriptMemberID for ScriptMemberID {
    fn into_script_member(self) -> ScriptMemberID {
        self
    }
}

impl IntoScriptMemberID for Member {
    fn into_script_member(self) -> ScriptMemberID {
        self.id
    }
}

impl IntoScriptMemberID for &Member {
    fn into_script_member(self) -> ScriptMemberID {
        self.id
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

pub trait ScriptAPI {
    fn with_state<T: 'static, V: Default, F>(&mut self, script_id: NodeID, f: F) -> V
    where
        F: FnOnce(&T) -> V;
    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V;
    fn script_attach(&mut self, node_id: NodeID, script_path: &str) -> bool;
    fn script_detach(&mut self, node_id: NodeID) -> bool;
    fn remove_script(&mut self, script_id: NodeID) -> bool;
    fn get_var(&mut self, script_id: NodeID, member: ScriptMemberID) -> Variant;
    fn set_var(&mut self, script_id: NodeID, member: ScriptMemberID, value: Variant);

    fn call_method(
        &mut self,
        script_id: NodeID,
        method: ScriptMemberID,
        params: &[Variant],
    ) -> Variant;
    fn attributes_of(&mut self, script_id: NodeID, member: &str) -> &'static [Attribute];
    fn members_with(&mut self, script_id: NodeID, attribute: &str) -> &'static [Member];
    fn has_attribute(&mut self, script_id: NodeID, member: &str, attribute: &str) -> bool;
}

pub struct ScriptModule<'rt, R: ScriptAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: ScriptAPI + ?Sized> ScriptModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn with_state<T: 'static, V: Default, F>(&mut self, script_id: NodeID, f: F) -> V
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

    pub fn script_attach(&mut self, node_id: NodeID, script_path: &str) -> bool {
        self.rt.script_attach(node_id, script_path)
    }

    pub fn script_detach(&mut self, node_id: NodeID) -> bool {
        self.rt.script_detach(node_id)
    }

    pub fn remove(&mut self, script_id: NodeID) -> bool {
        self.rt.remove_script(script_id)
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

    pub fn attributes_of<M: AsRef<str>>(
        &mut self,
        script_id: NodeID,
        member: M,
    ) -> &'static [Attribute] {
        self.rt.attributes_of(script_id, member.as_ref())
    }

    pub fn members_with<A: AsRef<str>>(
        &mut self,
        script_id: NodeID,
        attribute: A,
    ) -> &'static [Member] {
        self.rt.members_with(script_id, attribute.as_ref())
    }

    pub fn has_attribute<M: AsRef<str>, A: AsRef<str>>(
        &mut self,
        script_id: NodeID,
        member: M,
        attribute: A,
    ) -> bool {
        self.rt
            .has_attribute(script_id, member.as_ref(), attribute.as_ref())
    }
}

/// Script state macros.
///
/// These macros provide typed access to script state through closure-scoped borrows.
///
/// Typed read access to script state through a closure.
/// Returns `V::default()` if `id` is invalid or state type does not match `state_ty`.
///
/// Internals:
/// - The runtime resolves `script_id`, downcasts to `state_ty`, then calls your closure.
/// - The state reference is only valid inside the closure, preventing leaked borrows.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
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
/// - `ctx`: `&mut RuntimeContext<_>`
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
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `id`: target node `NodeID`
/// - `path`: script path (for example `"res://scripts/foo.rs"`)
#[macro_export]
macro_rules! script_attach {
    ($ctx:expr, $id:expr, $path:expr) => {
        $ctx.Scripts().script_attach($id, $path)
    };
}

/// Detaches the current script from a scene node.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `id`: target node `NodeID`
#[macro_export]
macro_rules! script_detach {
    ($ctx:expr, $id:expr) => {
        $ctx.Scripts().script_detach($id)
    };
}

/// Script member access macros.
///
/// Gets a script variable by member identifier.
///
/// Signature:
/// - `get_var!(&mut RuntimeContext<_, _>, NodeID, ScriptMemberID) -> Variant`
///
/// Usage:
/// - `get_var!(ctx, node_id, var!("health")) -> Variant`
///
/// Example:
/// - `let hp = get_var!(ctx, enemy_id, var!("health"));`
#[macro_export]
macro_rules! get_var {
    ($ctx:expr, $id:expr, var!($name:literal)) => {
        $ctx.Scripts().get_var($id, var!($name))
    };
    ($ctx:expr, $id:expr, $member:expr) => {{
        let _ = &$ctx;
        let _ = &$id;
        let _ = &$member;
        compile_error!("get_var! expects `var!(\"...\")` as member id");
    }};
}

/// Sets a script variable by member identifier.
///
/// Signature:
/// - `set_var!(&mut RuntimeContext<_, _>, NodeID, ScriptMemberID, Variant) -> ()`
///
/// Usage:
/// - `set_var!(ctx, node_id, var!("health"), variant!(100_i32)) -> ()`
///
/// Example:
/// - `set_var!(ctx, enemy_id, var!("health"), variant!(100_i32));`
#[macro_export]
macro_rules! set_var {
    ($ctx:expr, $id:expr, var!($name:literal), $value:expr) => {
        $ctx.Scripts().set_var($id, var!($name), $value)
    };
    ($ctx:expr, $id:expr, $member:expr, $value:expr) => {{
        let _ = &$ctx;
        let _ = &$id;
        let _ = &$member;
        let _ = &$value;
        compile_error!("set_var! expects `var!(\"...\")` as member id");
    }};
}

/// Calls a script method with params.
///
/// Signature:
/// - `call_method!(&mut RuntimeContext<_, _>, NodeID, ScriptMemberID, &[Variant]) -> Variant`
///
/// Usage:
/// - `call_method!(ctx, node_id, method!("take_damage"), params![10_i32]) -> Variant`
/// - `call_method!(ctx, node_id, func!("take_damage"), params![10_i32]) -> Variant`
///
/// Example:
/// - `let _ = call_method!(ctx, enemy_id, method!("take_damage"), params![10_i32]);`
/// - `let _ = call_method!(ctx, enemy_id, func!("take_damage"), params![10_i32]);`
#[macro_export]
macro_rules! call_method {
    ($ctx:expr, $id:expr, method!($name:literal), $params:expr) => {
        $ctx.Scripts().call_method($id, method!($name), $params)
    };
    ($ctx:expr, $id:expr, func!($name:literal), $params:expr) => {
        $ctx.Scripts().call_method($id, func!($name), $params)
    };
    ($ctx:expr, $id:expr, $method:expr, $params:expr) => {{
        let _ = &$ctx;
        let _ = &$id;
        let _ = &$method;
        let _ = &$params;
        compile_error!("call_method! expects `method!(\"...\")` or `func!(\"...\")` as method id");
    }};
}

/// Returns attributes declared on a script member.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `id`: script `NodeID`
/// - `member`: member name or `Member`
#[macro_export]
macro_rules! attributes_of {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().attributes_of($id, $member)
    };
}

/// Returns all members with a specific attribute.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `id`: script `NodeID`
/// - `attribute`: attribute name or `Attribute`
#[macro_export]
macro_rules! members_with {
    ($ctx:expr, $id:expr, $attribute:expr) => {
        $ctx.Scripts().members_with($id, $attribute)
    };
}

/// Checks whether a member has a specific attribute.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `id`: script `NodeID`
/// - `member`: member name or `Member`
/// - `attribute`: attribute name or `Attribute`
#[macro_export]
macro_rules! has_attribute {
    ($ctx:expr, $id:expr, $member:expr, $attribute:expr) => {
        $ctx.Scripts().has_attribute($id, $member, $attribute)
    };
}

/// Creates a typed `Member` descriptor from a static name.
///
/// Usage:
/// - `member!("health") -> Member`
#[macro_export]
macro_rules! member {
    ($name:expr) => {
        $crate::sub_apis::Member::new($name)
    };
}

/// Creates a typed `Attribute` descriptor from a static name.
///
/// Usage:
/// - `attribute!("readonly") -> Attribute`
#[macro_export]
macro_rules! attribute {
    ($value:expr) => {
        $crate::sub_apis::Attribute::new($value)
    };
}
