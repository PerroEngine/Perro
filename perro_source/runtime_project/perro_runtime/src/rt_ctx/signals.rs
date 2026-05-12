use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_input_api::InputWindow;
use perro_resource_api::ResourceWindow;
use perro_runtime_api::{RuntimeWindow, sub_apis::SignalAPI};
use perro_scripting::ScriptContext;
use perro_variant::Variant;
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
            let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
                ResourceWindow::new(resource_api.as_ref());
            let input_ptr = std::ptr::addr_of!(self.input);
            // SAFETY: During callback dispatch, input is treated as immutable runtime state.
            // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
            let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
                unsafe { InputWindow::new(&*input_ptr) };
            self.script_runtime
                .active_script_stack
                .push((instance_index, connection.script_id));
            let mut param_scratch = std::mem::take(&mut self.signal_runtime.param_scratch);
            {
                let mut run = RuntimeWindow::new(self);
                let call_params =
                    merged_signal_params(params, connection.params.as_ref(), &mut param_scratch);
                let mut sctx = ScriptContext {
                    run: &mut run,
                    res: &res,
                    ipt: &ipt,
                    id: connection.script_id,
                };
                let _ = behavior.call_method(connection.method, &mut sctx, call_params);
            }
            param_scratch.clear();
            self.signal_runtime.param_scratch = param_scratch;
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
        let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
            ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
            unsafe { InputWindow::new(&*input_ptr) };
        let mut param_scratch = std::mem::take(&mut self.signal_runtime.param_scratch);

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
            {
                let mut run = RuntimeWindow::new(self);
                let call_params =
                    merged_signal_params(params, connection.params.as_ref(), &mut param_scratch);
                let mut sctx = ScriptContext {
                    run: &mut run,
                    res: &res,
                    ipt: &ipt,
                    id: connection.script_id,
                };
                let _ = behavior.call_method(connection.method, &mut sctx, call_params);
            }
            param_scratch.clear();
            let _ = self.script_runtime.active_script_stack.pop();
            calls += 1;
        }

        self.signal_runtime.param_scratch = param_scratch;
        pending.clear();
        self.signal_runtime.emit_scratch = pending;
        calls
    }
}

fn merged_signal_params_into<'a, 'scratch>(
    emit_params: &'a [Variant],
    connect_params: &'a [Variant],
    scratch: &'scratch mut Vec<Variant>,
) -> &'scratch [Variant] {
    scratch.clear();
    scratch.reserve(emit_params.len() + connect_params.len());
    scratch.extend_from_slice(emit_params);
    scratch.extend_from_slice(connect_params);
    scratch.as_slice()
}

fn merged_signal_params<'a, 'scratch>(
    emit_params: &'a [Variant],
    connect_params: &'a [Variant],
    scratch: &'scratch mut Vec<Variant>,
) -> &'scratch [Variant]
where
    'a: 'scratch,
{
    if connect_params.is_empty() {
        return emit_params;
    }
    merged_signal_params_into(emit_params, connect_params, scratch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeScriptApi;
    use perro_scripting::{ScriptBehavior, ScriptFlags, ScriptLifecycle};
    use std::any::Any;
    use std::hint::black_box;
    use std::time::Instant;

    const BENCH_ITERS: usize = 1_000_000;
    const MANY_SIGNALS: usize = 8_192;

    struct NoopSignalScript;

    impl ScriptLifecycle<RuntimeScriptApi> for NoopSignalScript {}

    impl ScriptBehavior<RuntimeScriptApi> for NoopSignalScript {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(ScriptFlags::NONE)
        }

        fn create_state(&self) -> Box<dyn Any> {
            Box::new(())
        }

        fn get_var(&self, _state: &dyn Any, _var: ScriptMemberID) -> Variant {
            Variant::Null
        }

        fn set_var(&self, _state: &mut dyn Any, _var: ScriptMemberID, _value: Variant) {}

        fn call_method(
            &self,
            _method: ScriptMemberID,
            _ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
            params: &[Variant],
        ) -> Variant {
            black_box(params.len());
            Variant::Null
        }
    }

    #[test]
    fn merged_signal_params_appends_connect_params() {
        let emit_params = [Variant::from(7_i32)];
        let connect_params = [Variant::from("right_pressed")];
        let mut scratch = Vec::new();

        let merged = merged_signal_params(&emit_params, &connect_params, &mut scratch);

        assert_eq!(
            merged,
            &[Variant::from(7_i32), Variant::from("right_pressed")]
        );
    }

    #[test]
    #[ignore = "release signal emission benchmark"]
    fn bench_signal_emit_release_allocs() {
        let signal = SignalID::from_string("bench_signal_emit");
        let method = ScriptMemberID::from_string("on_signal");
        let emit_params = [Variant::from(7_i32), Variant::from(11_i32)];
        let connect_params = [Variant::from(13_i32), Variant::from(17_i32)];
        let mut runtime = Runtime::new();

        for i in 0..4 {
            let id = NodeID::new(i + 1);
            let behavior: Arc<dyn ScriptBehavior<RuntimeScriptApi>> = Arc::new(NoopSignalScript);
            runtime.scripts.insert(id, behavior, Box::new(()));
            assert!(runtime.signal_connect(id, signal, method, &connect_params));
        }

        for _ in 0..1024 {
            black_box(runtime.signal_emit(signal, &emit_params));
        }

        crate::test_alloc::reset_allocations();
        for _ in 0..BENCH_ITERS {
            black_box(runtime.signal_emit(signal, &emit_params));
        }

        crate::test_alloc::reset_allocations();
        let start = Instant::now();
        let mut calls = 0usize;
        for _ in 0..BENCH_ITERS {
            calls += black_box(runtime.signal_emit(signal, &emit_params));
        }
        let elapsed = start.elapsed();
        let allocations = crate::test_alloc::allocations();
        let ns_per_emit = elapsed.as_nanos() as f64 / BENCH_ITERS as f64;
        let ns_per_call = elapsed.as_nanos() as f64 / calls as f64;

        eprintln!(
            "signal_emit bench: emits={BENCH_ITERS} calls={calls} ns_emit={ns_per_emit:.2} ns_call={ns_per_call:.2} allocs={allocations}"
        );

        assert_eq!(calls, BENCH_ITERS * 4);
        assert_eq!(allocations, 0);
    }

    #[test]
    #[ignore = "release signal emission benchmark matrix"]
    fn bench_signal_emit_release_matrix() {
        let method = ScriptMemberID::from_string("on_signal");
        let emit_params = [Variant::from(7_i32), Variant::from(11_i32)];
        let connect_params = [Variant::from(13_i32), Variant::from(17_i32)];

        bench_case(
            "miss_empty_registry",
            Runtime::new(),
            SignalID::from_string("missing"),
            &[],
            0,
        );

        let mut single_no_params = Runtime::new();
        insert_noop_script(&mut single_no_params, NodeID::new(1));
        let single_signal = SignalID::from_string("single_no_params");
        assert!(single_no_params.signal_connect(NodeID::new(1), single_signal, method, &[]));
        bench_case("hit_1_no_params", single_no_params, single_signal, &[], 1);

        let mut single_full_params = Runtime::new();
        insert_noop_script(&mut single_full_params, NodeID::new(1));
        let full_signal = SignalID::from_string("single_emit_connect_params");
        assert!(single_full_params.signal_connect(
            NodeID::new(1),
            full_signal,
            method,
            &connect_params
        ));
        bench_case(
            "hit_1_emit_plus_connect_params",
            single_full_params,
            full_signal,
            &emit_params,
            1,
        );

        let mut four_full_params = Runtime::new();
        let four_signal = SignalID::from_string("four_emit_connect_params");
        for i in 0..4 {
            let id = NodeID::new(i + 1);
            insert_noop_script(&mut four_full_params, id);
            assert!(four_full_params.signal_connect(id, four_signal, method, &connect_params));
        }
        bench_case(
            "hit_4_emit_plus_connect_params",
            four_full_params,
            four_signal,
            &emit_params,
            4,
        );

        let mut many_keys = Runtime::new();
        insert_noop_script(&mut many_keys, NodeID::new(1));
        let mut signals = Vec::with_capacity(MANY_SIGNALS);
        for i in 0..MANY_SIGNALS {
            let signal = SignalID::from_u64(0xCAFE_0000_0000_0000_u64 | i as u64);
            signals.push(signal);
            assert!(many_keys.signal_connect(NodeID::new(1), signal, method, &[]));
        }
        bench_case(
            "hit_1_among_8192_signals",
            many_keys,
            signals[MANY_SIGNALS - 1],
            &[],
            1,
        );

        let mut many_keys_miss = Runtime::new();
        insert_noop_script(&mut many_keys_miss, NodeID::new(1));
        for i in 0..MANY_SIGNALS {
            let signal = SignalID::from_u64(0xBEEF_0000_0000_0000_u64 | i as u64);
            assert!(many_keys_miss.signal_connect(NodeID::new(1), signal, method, &[]));
        }
        bench_case(
            "miss_among_8192_signals",
            many_keys_miss,
            SignalID::from_u64(0xDEAD_F00D),
            &[],
            0,
        );

        let mut batch_runtime = Runtime::new();
        insert_noop_script(&mut batch_runtime, NodeID::new(1));
        let mut batch_signals = Vec::with_capacity(1024);
        for i in 0..1024 {
            let signal = SignalID::from_u64(0xFA57_0000_0000_0000_u64 | i as u64);
            batch_signals.push(signal);
            assert!(batch_runtime.signal_connect(NodeID::new(1), signal, method, &[]));
        }
        bench_batch("batch_1024_distinct_signals", batch_runtime, &batch_signals);

        let mut frame_runtime = Runtime::new();
        insert_noop_script(&mut frame_runtime, NodeID::new(1));
        let mut frame_signals = Vec::with_capacity(1000);
        for i in 0..1000 {
            let signal = SignalID::from_u64(0xF000_0000_0000_0000_u64 | i as u64);
            frame_signals.push(signal);
            assert!(frame_runtime.signal_connect(NodeID::new(1), signal, method, &[]));
        }
        bench_frame_batch("frame_1000_distinct_signals", frame_runtime, &frame_signals);
    }

    fn insert_noop_script(runtime: &mut Runtime, id: NodeID) {
        let behavior: Arc<dyn ScriptBehavior<RuntimeScriptApi>> = Arc::new(NoopSignalScript);
        runtime.scripts.insert(id, behavior, Box::new(()));
    }

    fn bench_case(
        name: &str,
        mut runtime: Runtime,
        signal: SignalID,
        params: &[Variant],
        expected_calls_per_emit: usize,
    ) {
        for _ in 0..1024 {
            black_box(runtime.signal_emit(signal, params));
        }

        crate::test_alloc::reset_allocations();
        for _ in 0..BENCH_ITERS {
            black_box(runtime.signal_emit(signal, params));
        }

        crate::test_alloc::reset_allocations();
        let start = Instant::now();
        let mut calls = 0usize;
        for _ in 0..BENCH_ITERS {
            calls += black_box(runtime.signal_emit(signal, params));
        }
        let elapsed = start.elapsed();
        let allocations = crate::test_alloc::allocations();
        let ns_per_emit = elapsed.as_nanos() as f64 / BENCH_ITERS as f64;
        let ns_per_call = if calls == 0 {
            0.0
        } else {
            elapsed.as_nanos() as f64 / calls as f64
        };

        eprintln!(
            "signal_matrix {name}: emits={BENCH_ITERS} calls={calls} ns_emit={ns_per_emit:.2} ns_call={ns_per_call:.2} allocs={allocations}"
        );

        assert_eq!(calls, BENCH_ITERS * expected_calls_per_emit);
        assert_eq!(allocations, 0);
    }

    fn bench_batch(name: &str, mut runtime: Runtime, signals: &[SignalID]) {
        const BATCH_ROUNDS: usize = 10_000;
        let total_emits = BATCH_ROUNDS * 1024;

        for _ in 0..1024 {
            for &signal in signals {
                black_box(runtime.signal_emit(signal, &[]));
            }
        }

        crate::test_alloc::reset_allocations();
        for _ in 0..BATCH_ROUNDS {
            for &signal in signals {
                black_box(runtime.signal_emit(signal, &[]));
            }
        }

        crate::test_alloc::reset_allocations();
        let start = Instant::now();
        let mut calls = 0usize;
        for _ in 0..BATCH_ROUNDS {
            for &signal in signals {
                calls += black_box(runtime.signal_emit(signal, &[]));
            }
        }
        let elapsed = start.elapsed();
        let allocations = crate::test_alloc::allocations();
        let ns_per_emit = elapsed.as_nanos() as f64 / total_emits as f64;

        eprintln!(
            "signal_matrix {name}: emits={total_emits} calls={calls} ns_emit={ns_per_emit:.2} ns_call={ns_per_emit:.2} allocs={allocations}"
        );

        assert_eq!(calls, total_emits);
        assert_eq!(allocations, 0);
    }

    fn bench_frame_batch(name: &str, mut runtime: Runtime, signals: &[SignalID]) {
        const FRAMES: usize = 10_000;
        let total_emits = FRAMES * signals.len();

        for _ in 0..1024 {
            for &signal in signals {
                black_box(runtime.signal_emit(signal, &[]));
            }
        }

        crate::test_alloc::reset_allocations();
        for _ in 0..FRAMES {
            for &signal in signals {
                black_box(runtime.signal_emit(signal, &[]));
            }
        }

        crate::test_alloc::reset_allocations();
        let start = Instant::now();
        let mut calls = 0usize;
        for _ in 0..FRAMES {
            for &signal in signals {
                calls += black_box(runtime.signal_emit(signal, &[]));
            }
        }
        let elapsed = start.elapsed();
        let allocations = crate::test_alloc::allocations();
        let ns_per_emit = elapsed.as_nanos() as f64 / total_emits as f64;
        let us_per_frame = elapsed.as_micros() as f64 / FRAMES as f64;

        eprintln!(
            "signal_matrix {name}: frames={FRAMES} emits_per_frame={} emits={total_emits} calls={calls} ns_emit={ns_per_emit:.2} us_frame={us_per_frame:.2} allocs={allocations}",
            signals.len()
        );

        assert_eq!(calls, total_emits);
        assert_eq!(allocations, 0);
    }
}
