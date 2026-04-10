use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_input::InputContext;
use perro_resource_context::ResourceContext;
use perro_runtime_context::{RuntimeContext, sub_apis::SignalAPI};
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

impl SignalAPI for Runtime {
    fn signal_connect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.signal_runtime
            .registry
            .connect(signal, script_id, function)
    }

    fn signal_disconnect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
    ) -> bool {
        self.signal_runtime
            .registry
            .disconnect(signal, script_id, function)
    }

    fn signal_emit(&mut self, signal: SignalID, params: &[Variant]) -> usize {
        let mut calls = 0usize;

        if let Some(connection) = self
            .signal_runtime
            .registry
            .single_signal_connection(signal)
        {
            let instance_index = self.scripts.instance_index_for_id(connection.script_id);
            let behavior = self.scripts.get_instance(connection.script_id).map(|instance| Arc::clone(&instance.behavior));
            if let (Some(instance_index), Some(behavior)) = (instance_index, behavior) {
                let resource_api = self.resource_api.clone();
                let res: ResourceContext<'_, crate::RuntimeResourceApi> =
                    ResourceContext::new(resource_api.as_ref());
                let input_ptr = std::ptr::addr_of!(self.input);
                // SAFETY: During callback dispatch, input is treated as immutable runtime state.
                // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
                let ipt: InputContext<'_, perro_input::InputSnapshot> =
                    unsafe { InputContext::new(&*input_ptr) };
                self.script_runtime
                    .active_script_stack
                    .push((instance_index, connection.script_id));
                let mut ctx = RuntimeContext::new(self);
                let _ = behavior.call_method(
                    connection.method,
                    &mut ctx,
                    &res,
                    &ipt,
                    connection.script_id,
                    params,
                );
                let _ = self.script_runtime.active_script_stack.pop();
                calls = 1;
            }
            return calls;
        }

        let mut pending = std::mem::take(&mut self.signal_runtime.emit_scratch);
        pending.clear();
        self.signal_runtime
            .registry
            .copy_signal_connections(signal, &mut pending);

        for connection in pending.iter().copied() {
            let instance_index = match self.scripts.instance_index_for_id(connection.script_id) {
                Some(i) => i,
                None => continue,
            };
            let behavior = match self.scripts.get_instance(connection.script_id) {
                Some(instance) => Arc::clone(&instance.behavior),
                None => continue,
            };

            let resource_api = self.resource_api.clone();
            let res: ResourceContext<'_, crate::RuntimeResourceApi> =
                ResourceContext::new(resource_api.as_ref());
            let input_ptr = std::ptr::addr_of!(self.input);
            // SAFETY: During callback dispatch, input is treated as immutable runtime state.
            // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
            let ipt: InputContext<'_, perro_input::InputSnapshot> =
                unsafe { InputContext::new(&*input_ptr) };
            self.script_runtime
                .active_script_stack
                .push((instance_index, connection.script_id));
            let mut ctx = RuntimeContext::new(self);
            let _ = behavior.call_method(
                connection.method,
                &mut ctx,
                &res,
                &ipt,
                connection.script_id,
                params,
            );
            let _ = self.script_runtime.active_script_stack.pop();
            calls += 1;
        }

        pending.clear();
        self.signal_runtime.emit_scratch = pending;
        calls
    }
}
