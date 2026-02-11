use perro_api::{API, modules::ScriptAPI};
use perro_ids::NodeID;
use perro_variant::Variant;

use crate::Runtime;

impl ScriptAPI for Runtime {
    fn call_init(&self, id: NodeID) {
        let (behavior, mut state) = {
            let mut scripts = self.scripts.borrow_mut();
            match scripts.take_state(id) {
                Some(pair) => pair,
                None => return,
            }
        };
        let api = API::new(self);
        behavior.init(&api, state.as_mut());
        let mut scripts = self.scripts.borrow_mut();
        let _ = scripts.put_state(id, state);
    }

    fn call_update(&self, id: NodeID) {
        let (behavior, mut state) = {
            let mut scripts = self.scripts.borrow_mut();
            match scripts.take_state(id) {
                Some(pair) => pair,
                None => return,
            }
        };
        let api = API::new(self);
        if behavior.script_flags().has_update() {
            behavior.update(&api, state.as_mut());
        }
        let mut scripts = self.scripts.borrow_mut();
        let _ = scripts.put_state(id, state);
    }

    fn call_fixed_update(&self, id: NodeID) {
        let (behavior, mut state) = {
            let mut scripts = self.scripts.borrow_mut();
            match scripts.take_state(id) {
                Some(pair) => pair,
                None => return,
            }
        };
        let api = API::new(self);
        if behavior.script_flags().has_fixed_update() {
            behavior.fixed_update(&api, state.as_mut());
        }
        let mut scripts = self.scripts.borrow_mut();
        let _ = scripts.put_state(id, state);
    }

    fn get_var(&self, script_id: NodeID, member: perro_ids::ScriptMemberID) -> Variant {
        let (behavior, state) = {
            let mut scripts = self.scripts.borrow_mut();
            match scripts.take_state(script_id) {
                Some(pair) => pair,
                None => return Variant::Null,
            }
        };
        let value = behavior.get_var(state.as_ref(), member);
        let mut scripts = self.scripts.borrow_mut();
        let _ = scripts.put_state(script_id, state);
        value
    }

    fn set_var(&self, script_id: NodeID, member: perro_ids::ScriptMemberID, value: Variant) {
        let (behavior, mut state) = {
            let mut scripts = self.scripts.borrow_mut();
            match scripts.take_state(script_id) {
                Some(pair) => pair,
                None => return,
            }
        };
        behavior.set_var(state.as_mut(), member, value);
        let mut scripts = self.scripts.borrow_mut();
        let _ = scripts.put_state(script_id, state);
    }

    fn call_method(
        &self,
        script_id: NodeID,
        method_id: perro_ids::ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        let (behavior, mut state) = {
            let mut scripts = self.scripts.borrow_mut();
            match scripts.take_state(script_id) {
                Some(pair) => pair,
                None => return Variant::Null,
            }
        };
        let api = API::new(self);
        let result = behavior.call_method(method_id, &api, state.as_mut(), params);
        let mut scripts = self.scripts.borrow_mut();
        let _ = scripts.put_state(script_id, state);
        result
    }
}
