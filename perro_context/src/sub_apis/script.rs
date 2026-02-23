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
    fn into_script_member_id(self) -> ScriptMemberID;
}

impl IntoScriptMemberID for ScriptMemberID {
    fn into_script_member_id(self) -> ScriptMemberID {
        self
    }
}

impl IntoScriptMemberID for Member {
    fn into_script_member_id(self) -> ScriptMemberID {
        self.id
    }
}

impl IntoScriptMemberID for &Member {
    fn into_script_member_id(self) -> ScriptMemberID {
        self.id
    }
}

impl IntoScriptMemberID for &str {
    fn into_script_member_id(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self)
    }
}

impl IntoScriptMemberID for String {
    fn into_script_member_id(self) -> ScriptMemberID {
        ScriptMemberID::from_string(self.as_str())
    }
}

impl IntoScriptMemberID for &String {
    fn into_script_member_id(self) -> ScriptMemberID {
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
        method_id: ScriptMemberID,
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
        self.rt.get_var(script_id, member.into_script_member_id())
    }

    pub fn set_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M, value: Variant) {
        self.rt
            .set_var(script_id, member.into_script_member_id(), value);
    }

    pub fn call_method<M: IntoScriptMemberID>(
        &mut self,
        script_id: NodeID,
        method_id: M,
        params: &[Variant],
    ) -> Variant {
        self.rt
            .call_method(script_id, method_id.into_script_member_id(), params)
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
