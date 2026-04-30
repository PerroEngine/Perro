use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_input::InputContext;
use perro_resource_context::ResourceContext;
use perro_runtime_context::{RuntimeContext, sub_apis::SignalAPI};
use perro_variant::Variant;
use std::borrow::Cow;
use std::sync::Arc;

use crate::Runtime;

impl SignalAPI for Runtime {
    fn signal_connect(
        &mut self,
        script_id: NodeID,
        signal: SignalID,
        function: ScriptMemberID,
        params: &[Variant],
    ) -> bool {
        self.signal_runtime
            .registry
            .connect(signal, script_id, function, params)
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
            let Some(instance_index) = self.scripts.instance_index_for_id(connection.script_id)
            else {
                return 0;
            };
            let Some(instance) = self
                .scripts
                .get_instance_scheduled_indexed(instance_index, connection.script_id)
            else {
                return 0;
            };
            let behavior = Arc::clone(&instance.behavior);
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
            let call_params = merged_signal_params(params, &connection.params);
            let _ = behavior.call_method(
                connection.method,
                &mut ctx,
                &res,
                &ipt,
                connection.script_id,
                call_params.as_ref(),
            );
            let _ = self.script_runtime.active_script_stack.pop();
            calls = 1;
            return calls;
        }

        let mut pending = std::mem::take(&mut self.signal_runtime.emit_scratch);
        pending.clear();
        self.signal_runtime
            .registry
            .copy_signal_connections(signal, &mut pending);
        let resource_api = self.resource_api.clone();
        let res: ResourceContext<'_, crate::RuntimeResourceApi> =
            ResourceContext::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputContext<'_, perro_input::InputSnapshot> =
            unsafe { InputContext::new(&*input_ptr) };

        for connection in pending.iter() {
            let instance_index = match self.scripts.instance_index_for_id(connection.script_id) {
                Some(i) => i,
                None => continue,
            };
            let behavior = match self
                .scripts
                .get_instance_scheduled_indexed(instance_index, connection.script_id)
            {
                Some(instance) => Arc::clone(&instance.behavior),
                None => continue,
            };
            self.script_runtime
                .active_script_stack
                .push((instance_index, connection.script_id));
            let mut ctx = RuntimeContext::new(self);
            let call_params = merged_signal_params(params, &connection.params);
            let _ = behavior.call_method(
                connection.method,
                &mut ctx,
                &res,
                &ipt,
                connection.script_id,
                call_params.as_ref(),
            );
            let _ = self.script_runtime.active_script_stack.pop();
            calls += 1;
        }

        pending.clear();
        self.signal_runtime.emit_scratch = pending;
        calls
    }
}

fn merged_signal_params<'a>(
    emit_params: &'a [Variant],
    connect_params: &'a [Variant],
) -> Cow<'a, [Variant]> {
    if connect_params.is_empty() {
        return Cow::Borrowed(emit_params);
    }
    let mut out = Vec::with_capacity(emit_params.len() + connect_params.len());
    out.extend_from_slice(emit_params);
    out.extend_from_slice(connect_params);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_signal_params_appends_connect_params() {
        let emit_params = [Variant::from(7_i32)];
        let connect_params = [Variant::from("right_pressed")];

        let merged = merged_signal_params(&emit_params, &connect_params);

        assert_eq!(
            merged.as_ref(),
            &[Variant::from(7_i32), Variant::from("right_pressed")]
        );
    }
}
