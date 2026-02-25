use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_input::InputContext;
use perro_resource_context::ResourceContext;
use perro_runtime_context::{RuntimeContext, sub_apis::SignalAPI};
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

impl SignalAPI for Runtime {
    fn connect_signal(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.signals.connect(signal, script_id, function)
    }

    fn disconnect_signal(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.signals.disconnect(signal, script_id, function)
    }

    fn emit_signal(&mut self, signal: SignalID, params: &[Variant]) -> usize {
        let mut calls = 0usize;

        if let Some(connection) = self.signals.single_signal_connection(signal) {
            let behavior = self
                .scripts
                .get_instance(connection.script_id)
                .map(|instance| Arc::clone(&instance.behavior));
            if let Some(behavior) = behavior {
                let resource_api = self.resource_api.clone();
                let res: ResourceContext<'_, crate::RuntimeResourceApi> =
                    ResourceContext::new(resource_api.as_ref());
                let input = self.input.clone();
                let ipt: InputContext<'_, perro_input::InputSnapshot> = InputContext::new(&input);
                let mut ctx = RuntimeContext::new(self);
                let _ = behavior.call_method(
                    connection.method,
                    &mut ctx,
                    &res,
                    &ipt,
                    connection.script_id,
                    params,
                );
                calls = 1;
            }
            return calls;
        }

        let mut pending = std::mem::take(&mut self.signal_emit_scratch);
        pending.clear();
        self.signals.copy_signal_connections(signal, &mut pending);

        for connection in pending.iter().copied() {
            let behavior = match self.scripts.get_instance(connection.script_id) {
                Some(instance) => Arc::clone(&instance.behavior),
                None => continue,
            };

            let resource_api = self.resource_api.clone();
            let res: ResourceContext<'_, crate::RuntimeResourceApi> =
                ResourceContext::new(resource_api.as_ref());
            let input = self.input.clone();
            let ipt: InputContext<'_, perro_input::InputSnapshot> = InputContext::new(&input);
            let mut ctx = RuntimeContext::new(self);
            let _ = behavior.call_method(
                connection.method,
                &mut ctx,
                &res,
                &ipt,
                connection.script_id,
                params,
            );
            calls += 1;
        }

        pending.clear();
        self.signal_emit_scratch = pending;
        calls
    }
}
