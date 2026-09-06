use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_input_api::InputWindow;
use perro_resource_api::ResourceWindow;
use perro_runtime_api::{RuntimeWindow, sub_apis::SignalAPI};
use perro_scripting::ScriptContext;
use perro_variant::Variant;
use std::rc::Rc;

use crate::Runtime;
use crate::cns::signal_registry::SignalConnectionsSnapshot;

#[cfg(feature = "bench")]
pub fn bench_insert_noop_signal_script(runtime: &mut Runtime, id: NodeID) {
    use crate::RuntimeScriptApi;
    use perro_scripting::{ScriptBehavior, ScriptFlags, ScriptLifecycle};
    use std::any::Any;
    use std::hint::black_box;

    struct BenchNoopSignalScript;

    impl ScriptLifecycle<RuntimeScriptApi> for BenchNoopSignalScript {}

    impl ScriptBehavior<RuntimeScriptApi> for BenchNoopSignalScript {
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

    runtime
        .scripts
        .insert(id, Rc::new(BenchNoopSignalScript), Box::new(()));
}

#[cfg(feature = "bench")]
pub fn bench_disconnect_signal_script(runtime: &mut Runtime, id: NodeID) -> usize {
    runtime.signal_runtime.registry.disconnect_script(id)
}

#[cfg(feature = "bench")]
pub fn bench_reset_signal_disconnect_counters(runtime: &mut Runtime) {
    runtime
        .signal_runtime
        .registry
        .reset_disconnect_script_counters();
}

#[cfg(feature = "bench")]
pub fn bench_signal_disconnect_counters(runtime: &Runtime) -> (usize, usize) {
    runtime.signal_runtime.registry.disconnect_script_counters()
}

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
        let Some(snapshot) = self
            .signal_runtime
            .registry
            .signal_connections_snapshot(signal)
        else {
            return 0;
        };

        if let SignalConnectionsSnapshot::Single(connection) = snapshot {
            let Some((instance_index, instance)) =
                self.scripts.indexed_instance(connection.script_id)
            else {
                return 0;
            };
            let behavior = Rc::clone(&instance.behavior);
            let active_context = self.current_script_callback_context();
            let resource_api = active_context.is_none().then(|| self.resource_api.clone());
            let context = active_context.unwrap_or_else(|| {
                let resource_api = resource_api.as_ref().expect("resource api present");
                crate::runtime::ScriptCallbackContext {
                    resource_api: resource_api.as_ref() as *const crate::RuntimeResourceApi,
                    input: std::ptr::addr_of!(self.input),
                }
            });
            // SAFETY: Context pointers are set only while a script callback is on
            // the stack, or from the fallback Arc/input owned by this runtime.
            let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
                unsafe { ResourceWindow::new(&*context.resource_api) };
            // SAFETY: During callback dispatch, input is treated as immutable runtime state.
            // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
            let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
                unsafe { InputWindow::new(&*context.input) };
            self.push_active_script_with_context(instance_index, connection.script_id, context);
            let connect_params = connection.params();
            let mut param_scratch = (!params.is_empty() && !connect_params.is_empty())
                .then(|| std::mem::take(&mut self.signal_runtime.param_scratch));
            {
                let mut run = RuntimeWindow::new(self);
                let call_params = if let Some(scratch) = param_scratch.as_mut() {
                    merged_signal_params_into(params, connect_params, scratch)
                } else if connect_params.is_empty() {
                    params
                } else {
                    connect_params
                };
                let mut sctx = ScriptContext {
                    run: &mut run,
                    res: &res,
                    ipt: &ipt,
                    id: connection.script_id,
                };
                let _ = behavior.call_method(connection.method, &mut sctx, call_params);
            }
            if let Some(mut scratch) = param_scratch {
                scratch.clear();
                self.signal_runtime.param_scratch = scratch;
            }
            self.pop_active_script(instance_index, connection.script_id);
            return 1;
        }

        let SignalConnectionsSnapshot::Multiple(pending) = snapshot else {
            return 0;
        };
        let active_context = self.current_script_callback_context();
        let resource_api = active_context.is_none().then(|| self.resource_api.clone());
        let context = active_context.unwrap_or_else(|| {
            let resource_api = resource_api.as_ref().expect("resource api present");
            crate::runtime::ScriptCallbackContext {
                resource_api: resource_api.as_ref() as *const crate::RuntimeResourceApi,
                input: std::ptr::addr_of!(self.input),
            }
        });
        // SAFETY: Context pointers are set only while a script callback is on
        // the stack, or from the fallback Arc/input owned by this runtime.
        let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
            unsafe { ResourceWindow::new(&*context.resource_api) };
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
            unsafe { InputWindow::new(&*context.input) };
        let mut param_scratch =
            (!params.is_empty()).then(|| std::mem::take(&mut self.signal_runtime.param_scratch));
        let mut calls = 0usize;

        for connection in pending.iter() {
            let Some((instance_index, instance)) =
                self.scripts.indexed_instance(connection.script_id)
            else {
                continue;
            };
            let behavior = Rc::clone(&instance.behavior);
            self.push_active_script_with_context(instance_index, connection.script_id, context);
            {
                let mut run = RuntimeWindow::new(self);
                let call_params = if let Some(scratch) = param_scratch.as_mut() {
                    merged_signal_params(params, connection.params(), scratch)
                } else {
                    connection.params()
                };
                let mut sctx = ScriptContext {
                    run: &mut run,
                    res: &res,
                    ipt: &ipt,
                    id: connection.script_id,
                };
                let _ = behavior.call_method(connection.method, &mut sctx, call_params);
            }
            if let Some(scratch) = param_scratch.as_mut() {
                scratch.clear();
            }
            self.pop_active_script(instance_index, connection.script_id);
            calls += 1;
        }

        if let Some(param_scratch) = param_scratch {
            self.signal_runtime.param_scratch = param_scratch;
        }
        calls
    }
}

impl Runtime {
    pub(crate) fn queue_ui_signal(&mut self, signal: SignalID, params: &[Variant]) {
        self.signal_runtime
            .queued_ui_signals
            .push((signal, Rc::from(params)));
    }

    pub(crate) fn flush_queued_ui_signals(&mut self) -> usize {
        if self.signal_runtime.queued_ui_signals.is_empty() {
            return 0;
        }

        let queued = std::mem::take(&mut self.signal_runtime.queued_ui_signals);
        let mut calls = 0usize;
        for (signal, params) in queued.iter() {
            calls += SignalAPI::signal_emit(self, *signal, params.as_ref());
        }
        // Signal handlers may queue more UI signals during dispatch. Keep them
        // for the next flush instead of replacing the live queue with scratch.
        crate::runtime::state::recycle_callback_queue(
            queued,
            &mut self.signal_runtime.queued_ui_signals,
        );
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
    if emit_params.is_empty() {
        return connect_params;
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NESTED_EMITTER_CALLS: AtomicUsize = AtomicUsize::new(0);
    static NESTED_RECEIVER_CALLS: AtomicUsize = AtomicUsize::new(0);
    static OUTER_RECEIVER_CALLS: AtomicUsize = AtomicUsize::new(0);

    struct NestedEmitterScript {
        inner_signal: SignalID,
    }

    struct NestedReceiverScript;

    struct OuterReceiverScript;

    struct RemovingSignalScript {
        victim: NodeID,
    }

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

    impl ScriptLifecycle<RuntimeScriptApi> for NestedEmitterScript {}

    impl ScriptBehavior<RuntimeScriptApi> for NestedEmitterScript {
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
            ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            NESTED_EMITTER_CALLS.fetch_add(1, Ordering::Relaxed);
            assert_eq!(ctx.run.Signals().emit(self.inner_signal, &[]), 1);
            Variant::Null
        }
    }

    macro_rules! impl_counting_signal_script {
        ($ty:ty, $counter:ident) => {
            impl ScriptLifecycle<RuntimeScriptApi> for $ty {}

            impl ScriptBehavior<RuntimeScriptApi> for $ty {
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
                    _params: &[Variant],
                ) -> Variant {
                    $counter.fetch_add(1, Ordering::Relaxed);
                    Variant::Null
                }
            }
        };
    }

    impl_counting_signal_script!(NestedReceiverScript, NESTED_RECEIVER_CALLS);
    impl_counting_signal_script!(OuterReceiverScript, OUTER_RECEIVER_CALLS);

    impl ScriptLifecycle<RuntimeScriptApi> for RemovingSignalScript {}

    impl ScriptBehavior<RuntimeScriptApi> for RemovingSignalScript {
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
            ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            let _ = ctx.run.Scripts().remove(self.victim);
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
    fn merged_signal_params_reuses_connect_params_when_emit_params_empty() {
        let connect_params = [Variant::from(13_i32), Variant::from(17_i32)];
        let mut scratch = vec![Variant::from(99_i32)];

        let merged = merged_signal_params(&[], &connect_params, &mut scratch);

        assert_eq!(merged, connect_params);
        assert_eq!(merged.as_ptr(), connect_params.as_ptr());
        assert_eq!(scratch, [Variant::from(99_i32)]);
    }

    #[test]
    fn signal_emit_connected_scripts_returns_call_count() {
        let signal = SignalID::from_string("bench_signal_emit");
        let method = ScriptMemberID::from_string("on_signal");
        let emit_params = [Variant::from(7_i32), Variant::from(11_i32)];
        let connect_params = [Variant::from(13_i32), Variant::from(17_i32)];
        let mut runtime = Runtime::new();

        for i in 0..4 {
            let id = NodeID::new(i + 1);
            let behavior: Rc<dyn ScriptBehavior<RuntimeScriptApi>> = Rc::new(NoopSignalScript);
            runtime.scripts.insert(id, behavior, Box::new(()));
            assert!(runtime.signal_connect(id, signal, method, &connect_params));
        }

        let mut calls = 0usize;
        for _ in 0..1024 {
            calls += black_box(runtime.signal_emit(signal, &emit_params));
        }

        assert_eq!(calls, 1024 * 4);
    }

    #[test]
    fn nested_emit_keeps_outer_emission_snapshot() {
        NESTED_EMITTER_CALLS.store(0, Ordering::Relaxed);
        NESTED_RECEIVER_CALLS.store(0, Ordering::Relaxed);
        OUTER_RECEIVER_CALLS.store(0, Ordering::Relaxed);
        let outer = SignalID::from_string("outer");
        let inner = SignalID::from_string("inner");
        let method = ScriptMemberID::from_string("handle");
        let emitter = NodeID::new(1);
        let nested_receiver = NodeID::new(2);
        let outer_receiver = NodeID::new(3);
        let mut runtime = Runtime::new();
        runtime.scripts.insert(
            emitter,
            Rc::new(NestedEmitterScript {
                inner_signal: inner,
            }),
            Box::new(()),
        );
        runtime
            .scripts
            .insert(nested_receiver, Rc::new(NestedReceiverScript), Box::new(()));
        runtime
            .scripts
            .insert(outer_receiver, Rc::new(OuterReceiverScript), Box::new(()));
        assert!(runtime.signal_connect(emitter, outer, method, &[]));
        assert!(runtime.signal_connect(outer_receiver, outer, method, &[]));
        assert!(runtime.signal_connect(nested_receiver, inner, method, &[]));

        assert_eq!(runtime.signal_emit(outer, &[]), 2);
        assert_eq!(NESTED_EMITTER_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(NESTED_RECEIVER_CALLS.load(Ordering::Relaxed), 1);
        assert_eq!(OUTER_RECEIVER_CALLS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn reentrant_script_teardown_skips_stale_snapshot_connection() {
        let outer = SignalID::from_string("reentrant_teardown");
        let victim_only = SignalID::from_string("victim_only");
        let method = ScriptMemberID::from_string("handle");
        let remover = NodeID::new(1);
        let victim = NodeID::new(2);
        let mut runtime = Runtime::new();
        runtime.scripts.insert(
            remover,
            Rc::new(RemovingSignalScript { victim }),
            Box::new(()),
        );
        runtime
            .scripts
            .insert(victim, Rc::new(NoopSignalScript), Box::new(()));
        assert!(runtime.signal_connect(remover, outer, method, &[]));
        assert!(runtime.signal_connect(victim, outer, method, &[]));
        assert!(runtime.signal_connect(victim, victim_only, method, &[]));

        assert_eq!(runtime.signal_emit(outer, &[]), 1);
        assert_eq!(runtime.signal_emit(victim_only, &[]), 0);
        assert_eq!(runtime.signal_emit(outer, &[]), 1);
    }
}
