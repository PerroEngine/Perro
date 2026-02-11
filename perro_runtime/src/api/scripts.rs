use perro_api::{API, modules::ScriptAPI};
use perro_ids::NodeID;
use perro_variant::Variant;

use crate::Runtime;

impl ScriptAPI for Runtime {
    fn call_init(&mut self, id: NodeID) {
        let (behavior, mut state) = {
            match self.scripts.take_state(id) {
                Some(pair) => pair,
                None => {
                    let (behavior, mut state) = match self.scripts.take_reentry_clone(id) {
                        Some(pair) => pair,
                        None => return,
                    };
                    {
                        let mut api = API::new(self);
                        behavior.init(&mut api, state.as_mut());
                    }
                    let _ = self.scripts.push_reentry_result(id, state);
                    return;
                }
            }
        };
        {
            let mut api = API::new(self);
            behavior.init(&mut api, state.as_mut());
        }
        let _ = self.scripts.put_state(id, state);
    }

    fn call_update(&mut self, id: NodeID) {
        let (behavior, mut state) = {
            match self.scripts.take_state(id) {
                Some(pair) => pair,
                None => {
                    let (behavior, mut state) = match self.scripts.take_reentry_clone(id) {
                        Some(pair) => pair,
                        None => return,
                    };
                    {
                        let mut api = API::new(self);
                        if behavior.script_flags().has_update() {
                            behavior.update(&mut api, state.as_mut());
                        }
                    }
                    let _ = self.scripts.push_reentry_result(id, state);
                    return;
                }
            }
        };
        {
            let mut api = API::new(self);
            if behavior.script_flags().has_update() {
                behavior.update(&mut api, state.as_mut());
            }
        }
        let _ = self.scripts.put_state(id, state);
    }

    fn call_fixed_update(&mut self, id: NodeID) {
        let (behavior, mut state) = {
            match self.scripts.take_state(id) {
                Some(pair) => pair,
                None => {
                    let (behavior, mut state) = match self.scripts.take_reentry_clone(id) {
                        Some(pair) => pair,
                        None => return,
                    };
                    {
                        let mut api = API::new(self);
                        if behavior.script_flags().has_fixed_update() {
                            behavior.fixed_update(&mut api, state.as_mut());
                        }
                    }
                    let _ = self.scripts.push_reentry_result(id, state);
                    return;
                }
            }
        };
        {
            let mut api = API::new(self);
            if behavior.script_flags().has_fixed_update() {
                behavior.fixed_update(&mut api, state.as_mut());
            }
        }
        let _ = self.scripts.put_state(id, state);
    }

    fn get_var(&mut self, script_id: NodeID, member: perro_ids::ScriptMemberID) -> Variant {
        let (behavior, state) = {
            match self.scripts.take_state(script_id) {
                Some(pair) => pair,
                None => {
                    let (behavior, state) = match self.scripts.take_reentry_clone(script_id) {
                        Some(pair) => pair,
                        None => return Variant::Null,
                    };
                    return behavior.get_var(state.as_ref(), member);
                }
            }
        };
        let value = behavior.get_var(state.as_ref(), member);
        let _ = self.scripts.put_state(script_id, state);
        value
    }

    fn set_var(&mut self, script_id: NodeID, member: perro_ids::ScriptMemberID, value: Variant) {
        let (behavior, mut state) = {
            match self.scripts.take_state(script_id) {
                Some(pair) => pair,
                None => {
                    let (behavior, mut state) = match self.scripts.take_reentry_clone(script_id) {
                        Some(pair) => pair,
                        None => return,
                    };
                    behavior.set_var(state.as_mut(), member, value);
                    let _ = self.scripts.push_reentry_result(script_id, state);
                    return;
                }
            }
        };
        behavior.set_var(state.as_mut(), member, value);
        let _ = self.scripts.put_state(script_id, state);
    }

    fn call_method(
        &mut self,
        script_id: NodeID,
        method_id: perro_ids::ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        let (behavior, mut state) = {
            match self.scripts.take_state(script_id) {
                Some(pair) => pair,
                None => {
                    let (behavior, mut state) = match self.scripts.take_reentry_clone(script_id) {
                        Some(pair) => pair,
                        None => return Variant::Null,
                    };
                    let result = {
                        let mut api = API::new(self);
                        behavior.call_method(method_id, &mut api, state.as_mut(), params)
                    };
                    let _ = self.scripts.push_reentry_result(script_id, state);
                    return result;
                }
            }
        };
        let result = {
            let mut api = API::new(self);
            behavior.call_method(method_id, &mut api, state.as_mut(), params)
        };
        let _ = self.scripts.put_state(script_id, state);
        result
    }
}
