use perro_context::{RuntimeContext, sub_apis::SignalAPI};
use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_variant::Variant;
use std::sync::Arc;
use std::time::Instant;

use crate::Runtime;

impl SignalAPI for Runtime {
    fn connect_signal(
        &mut self,
        script_id: NodeID,
        signal_id: SignalID,
        function_id: ScriptMemberID,
    ) -> bool {
        self.signals.connect(signal_id, script_id, function_id)
    }

    fn disconnect_signal(
        &mut self,
        script_id: NodeID,
        signal_id: SignalID,
        function_id: ScriptMemberID,
    ) -> bool {
        self.signals.disconnect(signal_id, script_id, function_id)
    }

    fn emit_signal(&mut self, signal_id: SignalID, params: &[Variant]) -> usize {
        let emit_start = Instant::now();

        let mut pending = std::mem::take(&mut self.signal_emit_scratch);
        pending.clear();
        self.signals.copy_signal_connections(signal_id, &mut pending);
        let lookup_from_emit_ns = emit_start.elapsed().as_nanos();

        let mut calls = 0usize;
        let mut missing_scripts = 0usize;
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

            let mut ctx = RuntimeContext::new(self);
            let _ = behavior.call_method(connection.method_id, &mut ctx, connection.script_id, params);
            calls += 1;
        }

        let first_call_from_emit_ns = first_call_from_emit_ns.unwrap_or(0);
        println!(
            "[signal.emit] signal={} params={} connections={} calls={} missing_scripts={} lookup_from_emit_ns={} first_call_from_emit_ns={}",
            signal_id.as_u64(),
            params.len(),
            pending.len(),
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
