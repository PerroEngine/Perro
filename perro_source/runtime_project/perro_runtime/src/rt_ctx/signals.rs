use perro_ids::{NodeID, ScriptMemberID, SignalID};
use perro_input_api::InputWindow;
use perro_resource_api::ResourceWindow;
use perro_runtime_api::{RuntimeWindow, sub_apis::SignalAPI};
use perro_scripting::ScriptContext;
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

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
        .insert(id, Arc::new(BenchNoopSignalScript), Box::new(()));
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
            self.pop_active_script(instance_index, connection.script_id);
            calls = 1;
            return calls;
        }

        let mut pending = std::mem::take(&mut self.signal_runtime.emit_scratch);
        pending.clear();
        self.signal_runtime
            .registry
            .copy_signal_connections(signal, &mut pending);
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
            self.push_active_script_with_context(instance_index, connection.script_id, context);
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
            self.pop_active_script(instance_index, connection.script_id);
            calls += 1;
        }

        self.signal_runtime.param_scratch = param_scratch;
        pending.clear();
        self.signal_runtime.emit_scratch = pending;
        calls
    }
}

impl Runtime {
    pub(crate) fn queue_ui_signal(&mut self, signal: SignalID, params: &[Variant]) {
        self.signal_runtime
            .queued_ui_signals
            .push((signal, Arc::from(params)));
    }

    pub(crate) fn flush_queued_ui_signals(&mut self) -> usize {
        if self.signal_runtime.queued_ui_signals.is_empty() {
            return 0;
        }

        let mut queued = std::mem::take(&mut self.signal_runtime.queued_ui_signals);
        let mut calls = 0usize;
        for (signal, params) in queued.iter() {
            calls += SignalAPI::signal_emit(self, *signal, params.as_ref());
        }
        queued.clear();
        self.signal_runtime.queued_ui_signals = queued;
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
    fn signal_emit_connected_scripts_returns_call_count() {
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

        let mut calls = 0usize;
        for _ in 0..1024 {
            calls += black_box(runtime.signal_emit(signal, &emit_params));
        }

        assert_eq!(calls, 1024 * 4);
    }
}
