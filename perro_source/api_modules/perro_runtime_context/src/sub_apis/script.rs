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
    fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V;
    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V;
    fn attach_script(&mut self, node_id: NodeID, script_path: &str) -> bool;
    fn detach_script(&mut self, node_id: NodeID) -> bool;
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

    pub fn attach(&mut self, node_id: NodeID, script_path: &str) -> bool {
        self.rt.attach_script(node_id, script_path)
    }

    pub fn detach(&mut self, node_id: NodeID) -> bool {
        self.rt.detach_script(node_id)
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

#[macro_export]
macro_rules! with_state {
    ($ctx:expr, $state_ty:ty, $id:expr, $f:expr) => {
        $ctx.Scripts().with_state::<$state_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_state_mut {
    ($ctx:expr, $state_ty:ty, $id:expr, $f:expr) => {
        $ctx.Scripts().with_state_mut::<$state_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! attach_script {
    ($ctx:expr, $id:expr, $path:expr) => {
        $ctx.Scripts().attach($id, $path)
    };
}

#[macro_export]
macro_rules! detach_script {
    ($ctx:expr, $id:expr) => {
        $ctx.Scripts().detach($id)
    };
}

#[macro_export]
macro_rules! get_var {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().get_var($id, $member)
    };
}

#[macro_export]
macro_rules! set_var {
    ($ctx:expr, $id:expr, $member:expr, $value:expr) => {
        $ctx.Scripts().set_var($id, $member, $value)
    };
}

#[macro_export]
macro_rules! call_method {
    ($ctx:expr, $id:expr, $method:expr, $params:expr) => {
        $ctx.Scripts().call_method($id, $method, $params)
    };
}

#[macro_export]
macro_rules! attributes_of {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().attributes_of($id, $member)
    };
}

#[macro_export]
macro_rules! members_with {
    ($ctx:expr, $id:expr, $attribute:expr) => {
        $ctx.Scripts().members_with($id, $attribute)
    };
}

#[macro_export]
macro_rules! has_attribute {
    ($ctx:expr, $id:expr, $member:expr, $attribute:expr) => {
        $ctx.Scripts().has_attribute($id, $member, $attribute)
    };
}

#[macro_export]
macro_rules! member {
    ($name:expr) => {
        ::perro_runtime_context::sub_apis::Member::new($name)
    };
}

#[macro_export]
macro_rules! attribute {
    ($value:expr) => {
        ::perro_runtime_context::sub_apis::Attribute::new($value)
    };
}



