use perro_api::modules::ScriptAPI;
use perro_variant::Variant;

use crate::Runtime;

impl ScriptAPI for Runtime {
    fn get_var(&mut self, member: perro_ids::ScriptMemberID) -> Variant {
        todo!()
    }

    fn set_var(&mut self, member: perro_ids::ScriptMemberID, value: Variant) {
        todo!()
    }

    fn call_method(&mut self, method_id: perro_ids::ScriptMemberID, params: &[Variant]) -> Variant {
        todo!()
    }

    fn call_script_update(&mut self, id: perro_ids::NodeID) {
        todo!()
    }

    fn call_script_fixed_update(&mut self, id: perro_ids::NodeID) {
        todo!()
    }

    fn call_script_init(&mut self, id: perro_ids::NodeID) {
        todo!()
    }
}
