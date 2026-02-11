use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;

pub trait ScriptAPI {
    fn call_init(&mut self, id: NodeID);
    fn call_update(&mut self, id: NodeID);
    fn call_fixed_update(&mut self, id: NodeID);
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

    pub fn call_init(&mut self, id: NodeID) {
        self.rt.call_init(id);
    }

    pub fn call_update(&mut self, id: NodeID) {
        self.rt.call_update(id);
    }

    pub fn call_fixed_update(&mut self, id: NodeID) {
        self.rt.call_fixed_update(id);
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
