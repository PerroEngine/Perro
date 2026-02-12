use perro_api::{API, sub_apis::ScriptAPI};
use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

impl Runtime {
    pub(crate) fn call_update_script(&mut self, id: NodeID) {
        let behavior = match self.scripts.get_instance(id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        if behavior.script_flags().has_update() {
            let mut api = API::new(self);
            behavior.update(&mut api, id);
        }
    }

    pub(crate) fn call_fixed_update_script(&mut self, id: NodeID) {
        let behavior = match self.scripts.get_instance(id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        if behavior.script_flags().has_fixed_update() {
            let mut api = API::new(self);
            behavior.fixed_update(&mut api, id);
        }
    }
}

impl ScriptAPI for Runtime {
    fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        self.scripts.with_state(script_id, f)
    }

    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        self.scripts.with_state_mut(script_id, f)
    }

    fn get_var(&mut self, script_id: NodeID, member: ScriptMemberID) -> Variant {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return Variant::Null,
        };
        self.scripts
            .with_state_dyn(script_id, |state| behavior.get_var(state, member))
            .unwrap_or(Variant::Null)
    }

    fn set_var(&mut self, script_id: NodeID, member: ScriptMemberID, value: Variant) {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        let _ = self
            .scripts
            .with_state_mut_dyn(script_id, |state| behavior.set_var(state, member, value));
    }

    fn call_method(
        &mut self,
        script_id: NodeID,
        method_id: ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return Variant::Null,
        };
        let mut api = API::new(self);
        behavior.call_method(method_id, &mut api, script_id, params)
    }
}
