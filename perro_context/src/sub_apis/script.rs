use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;

pub trait ScriptAPI {
    fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V;
    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V;
    fn remove_script(&mut self, script_id: NodeID) -> bool;
    fn get_var(&mut self, script_id: NodeID, member: ScriptMemberID) -> Variant;
    fn set_var(&mut self, script_id: NodeID, member: ScriptMemberID, value: Variant);

    fn call_method(
        &mut self,
        script_id: NodeID,
        method_id: ScriptMemberID,
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

    pub fn remove(&mut self, script_id: NodeID) -> bool {
        self.rt.remove_script(script_id)
    }

    pub fn get_var(&mut self, script_id: NodeID, member: ScriptMemberID) -> Variant {
        self.rt.get_var(script_id, member)
    }

    pub fn set_var(&mut self, script_id: NodeID, member: ScriptMemberID, value: Variant) {
        self.rt.set_var(script_id, member, value);
    }

    pub fn call_method(
        &mut self,
        script_id: NodeID,
        method_id: ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        self.rt.call_method(script_id, method_id, params)
    }
}
