use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;

pub trait ScriptLifecycle {
    fn init(&mut self);
    fn update(&mut self);
    fn fixed_update(&mut self);
}

pub trait ScriptObject: ScriptLifecycle {
    fn internal_init(&mut self) {
        self.init();
    }

    fn internal_update(&mut self) {
        self.update();
    }

    fn internal_fixed_update(&mut self) {
        self.fixed_update();
    }

    fn get_id(&self) -> NodeID;
    fn set_id(&mut self, id: NodeID);

    fn get_var(&self, var_id: ScriptMemberID) -> Variant;
    fn set_var(&mut self, var_id: ScriptMemberID, value: Variant);
    
    fn apply_exposed_vars(&mut self, vars: &[(ScriptMemberID, Variant)]) {
        for (var_id, value) in vars {
            self.set_var(*var_id, value.clone());
        }
    }

    fn call_method(&mut self, method_id: ScriptMemberID, params: &[Variant]) -> Variant;
}
