use perro_runtime_context::{RuntimeContext, sub_apis::SignalAPI};
use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_resource_context::ResourceContext;
use perro_variant::Variant;
use std::sync::Arc;
use std::time::Instant;

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
        let emit_start = Instant::now();
        let connection_count = self.signals.signal_connection_count(signal);
        let mut calls = 0usize;
        let mut missing_scripts = 0usize;

        if let Some(connection) = self.signals.single_signal_connection(signal) {
            let lookup_from_emit_ns = emit_start.elapsed().as_nanos();
            let behavior = self
                .scripts
                .get_instance(connection.script_id)
                .map(|instance| Arc::clone(&instance.behavior));
            let first_call_from_emit_ns = emit_start.elapsed().as_nanos();
            if let Some(behavior) = behavior {
                let resource_api = self.resource_api.clone();
                let res: ResourceContext<'_, crate::RuntimeResourceApi> = ResourceContext::new(resource_api.as_ref());
                let mut ctx = RuntimeContext::new(self);
                let _ = behavior.call_method(
                    connection.method,
                    &mut ctx,
                    &res,
                    connection.script_id,
                    params,
                );
                calls = 1;
            } else {
                missing_scripts = 1;
            }
            println!(
                "[signal.emit] signal={} params={} connections={} calls={} missing_scripts={} lookup_from_emit_ns={} first_call_from_emit_ns={}",
                signal.as_u64(),
                params.len(),
                connection_count,
                calls,
                missing_scripts,
                lookup_from_emit_ns,
                first_call_from_emit_ns,
            );
            return calls;
        }

        let mut pending = std::mem::take(&mut self.signal_emit_scratch);
        pending.clear();
        self.signals.copy_signal_connections(signal, &mut pending);
        let lookup_from_emit_ns = emit_start.elapsed().as_nanos();

        let mut first_call_from_emit_ns: Option<u128> = None;
        for connection in pending.iter().copied() {
            let behavior = match self.scripts.get_instance(connection.script_id) {
                Some(instance) => Arc::clone(&instance.behavior),
                None => {
                    missing_scripts += 1;
                    continue;
                }
            };
            if first_call_from_emit_ns.is_none() {
                // Timestamp taken immediately before the first call starts.
                first_call_from_emit_ns = Some(emit_start.elapsed().as_nanos());
            }

            let resource_api = self.resource_api.clone();
            let res: ResourceContext<'_, crate::RuntimeResourceApi> = ResourceContext::new(resource_api.as_ref());
            let mut ctx = RuntimeContext::new(self);
            let _ = behavior.call_method(connection.method, &mut ctx, &res, connection.script_id, params);
            calls += 1;
        }

        let first_call_from_emit_ns = first_call_from_emit_ns.unwrap_or(0);
        println!(
            "[signal.emit] signal={} params={} connections={} calls={} missing_scripts={} lookup_from_emit_ns={} first_call_from_emit_ns={}",
            signal.as_u64(),
            params.len(),
            connection_count,
            calls,
            missing_scripts,
            lookup_from_emit_ns,
            first_call_from_emit_ns,
        );

        pending.clear();
        self.signal_emit_scratch = pending;
        calls
    }
}



