use perro_api::{API, modules::ScriptAPI};
use perro_ids::NodeID;
use perro_variant::Variant;

use crate::Runtime;

impl ScriptAPI for Runtime {
    fn call_init(&self, id: NodeID) {
        let script_rc = {
            let scripts = self.scripts.borrow();
            scripts.get_script_rc(id)
        };
        let api = API::new(self);
        if let Some(script_rc) = script_rc {
            let script_ref = script_rc.borrow();
            script_ref.init(&api);
        }
    }

    fn call_update(&self, id: NodeID) {
        let script_rc = {
            let scripts = self.scripts.borrow();
            scripts.get_script_rc(id)
        };
        let api = API::new(self);
        if let Some(script_rc) = script_rc {
            let script_ref = script_rc.borrow();
            if script_ref.script_flags().has_update() {
                script_ref.update(&api);
            }
        }
    }

    fn call_fixed_update(&self, id: NodeID) {
        let script_rc = {
            let scripts = self.scripts.borrow();
            scripts.get_script_rc(id)
        };
        let api = API::new(self);
        if let Some(script_rc) = script_rc {
            let script_ref = script_rc.borrow();
            if script_ref.script_flags().has_fixed_update() {
                script_ref.fixed_update(&api);
            }
        }
    }
    fn get_var(&self, member: perro_ids::ScriptMemberID) -> Variant {
        todo!()
    }

    fn set_var(&self, member: perro_ids::ScriptMemberID, value: Variant) {
        todo!()
    }

    fn call_method(
        &self,
        script_id: NodeID,
        method_id: perro_ids::ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        let script_rc = {
            let scripts = self.scripts.borrow();
            scripts.get_script_rc(script_id)
        };
        let api = API::new(self);
        if let Some(script_rc) = script_rc {
            let script_ref = script_rc.borrow();
            return script_ref.call_method(method_id, &api, params);
        }
        Variant::Null
    }
}
