use perro_ids::ScriptMemberID;
use perro_variant::Variant;

pub trait ScriptAPI {
    fn get_var(&mut self, member: ScriptMemberID) -> Variant;
    fn set_var(&mut self, member: ScriptMemberID, value: Variant);

    fn call_method(&mut self, method_id: ScriptMemberID, params: &[Variant]) -> Variant;
}

pub struct ScriptModule<'rt, R: ScriptAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: ScriptAPI + ?Sized> ScriptModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn get_var(&mut self, member: ScriptMemberID) -> Variant {
        self.rt.get_var(member)
    }

    pub fn set_var(&mut self, member: ScriptMemberID, value: Variant) {
        self.rt.set_var(member, value);
    }

    pub fn call_method(&mut self, method_id: ScriptMemberID, params: &[Variant]) -> Variant {
        self.rt.call_method(method_id, params)
    }
}
