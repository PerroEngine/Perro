use perro_ids::string_to_u64;
use perro_ids::{NodeID, ScriptMemberID};
use perro_input_api::InputWindow;
use perro_io::{DlcSelfContextGuard, push_dlc_self_context};
use perro_resource_api::ResourceWindow;
use perro_runtime_api::{RuntimeWindow, sub_apis::ScriptAPI};
use perro_scripting::ScriptContext;
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

#[cfg(feature = "bench")]
#[derive(Clone, Debug, Default)]
pub struct BenchScriptState {
    pub frame: u64,
    pub hp: i32,
    pub pos: [f32; 3],
}

#[cfg(feature = "bench")]
pub fn bench_insert_state_script(runtime: &mut Runtime, id: NodeID) {
    use crate::RuntimeScriptApi;
    use perro_scripting::{ScriptBehavior, ScriptFlags, ScriptLifecycle};
    use std::any::Any;

    struct BenchStateScript;

    impl ScriptLifecycle<RuntimeScriptApi> for BenchStateScript {}

    impl ScriptBehavior<RuntimeScriptApi> for BenchStateScript {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(ScriptFlags::HAS_UPDATE | ScriptFlags::HAS_FIXED_UPDATE)
        }

        fn create_state(&self) -> Box<dyn Any> {
            Box::<BenchScriptState>::default()
        }

        fn get_var(&self, state: &dyn Any, var: ScriptMemberID) -> Variant {
            let Some(state) = state.downcast_ref::<BenchScriptState>() else {
                return Variant::Null;
            };
            match var.0 {
                1 => Variant::from(state.frame as i64),
                2 => Variant::from(state.hp),
                _ => Variant::Null,
            }
        }

        fn set_var(&self, state: &mut dyn Any, var: ScriptMemberID, value: Variant) {
            let Some(state) = state.downcast_mut::<BenchScriptState>() else {
                return;
            };
            match var.0 {
                1 => {
                    if let Some(value) = value.as_i64() {
                        state.frame = value.max(0) as u64;
                    }
                }
                2 => {
                    if let Some(value) = value.as_i32() {
                        state.hp = value;
                    }
                }
                _ => {}
            }
        }

        fn call_method(
            &self,
            _method: ScriptMemberID,
            _ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            Variant::Null
        }
    }

    runtime.scripts.insert(
        id,
        Arc::new(BenchStateScript),
        Box::<BenchScriptState>::default(),
    );
}

#[cfg(feature = "bench")]
pub fn bench_with_active_script<V, F>(runtime: &mut Runtime, id: NodeID, f: F) -> Option<V>
where
    F: FnOnce(&mut Runtime) -> V,
{
    let instance_index = runtime.scripts.instance_index_for_id(id)?;
    runtime.push_active_script_with_context(instance_index, id, runtime.script_callback_context());
    let value = f(runtime);
    runtime.pop_active_script(instance_index, id);
    Some(value)
}

impl Runtime {
    /// Push active script frame for recursive script/runtime calls.
    ///
    /// `with_state(ctx.id, ..)` and node self-lookups read only the stack top.
    /// Nested script calls push another frame and pop back to the parent frame
    /// when the callback returns.
    /// Push active script frame and cache callback context for nested script calls.
    #[inline(always)]
    pub(crate) fn push_active_script_with_context(
        &mut self,
        instance_index: usize,
        id: NodeID,
        context: crate::runtime::ScriptCallbackContext,
    ) {
        if self.script_runtime.active_script_stack.is_empty() {
            self.script_runtime.active_callback_context = Some(context);
        }
        self.script_runtime
            .active_script_stack
            .push((instance_index, id));
    }

    #[inline(always)]
    pub(crate) fn pop_active_script(&mut self, instance_index: usize, id: NodeID) {
        let popped = self.script_runtime.active_script_stack.pop();
        debug_assert_eq!(popped, Some((instance_index, id)));
        if self.script_runtime.active_script_stack.is_empty() {
            self.script_runtime.active_callback_context = None;
        }
    }

    #[inline(always)]
    pub(crate) fn current_script_callback_context(
        &self,
    ) -> Option<crate::runtime::ScriptCallbackContext> {
        self.script_runtime.active_callback_context
    }

    #[inline(always)]
    pub(crate) fn script_callback_context(&self) -> crate::runtime::ScriptCallbackContext {
        crate::runtime::ScriptCallbackContext {
            resource_api: self.resource_api.as_ref() as *const crate::RuntimeResourceApi,
            input: std::ptr::addr_of!(self.input),
        }
    }

    /// Push the DLC `self` context frame for a script dispatch, skipping all work
    /// in the common no-DLC case.
    ///
    /// When no script anywhere has a mount (`script_instance_dlc_mounts` empty),
    /// the DLC-self thread-local can only already be `None`: it is set solely by
    /// this push path, so with an empty map no outer `push(Some(..))` frame can be
    /// live above us on the stack. `push_dlc_self_context(None)` would therefore
    /// just replace `None` with `None`, a no-op. We skip the map lookup, the
    /// `Option<String>` clone, and the thread-local push entirely and return
    /// `None` (no guard needed).
    ///
    /// When the map is non-empty we preserve exact prior behavior: look up this
    /// script's mount (possibly `None`) and push it. Pushing `None` here still
    /// matters — it shadows an outer script's `Some(..)` frame during nested
    /// dispatch. The returned guard must be held in a local for the whole callback
    /// so the frame is popped on drop.
    #[inline(always)]
    pub(crate) fn push_script_dlc_self_context(&self, id: NodeID) -> Option<DlcSelfContextGuard> {
        if self.script_runtime.script_instance_dlc_mounts.is_empty() {
            return None;
        }
        let mount = self
            .script_runtime
            .script_instance_dlc_mounts
            .get(&id)
            .cloned();
        Some(push_dlc_self_context(mount.as_deref()))
    }

    #[inline(always)]
    pub(crate) fn queue_start_script(&mut self, id: NodeID) {
        let slot = id.index() as usize;
        if self.script_runtime.pending_start_flags.len() <= slot {
            self.script_runtime
                .pending_start_flags
                .resize(slot + 1, None);
        }
        if self.script_runtime.pending_start_flags[slot] == Some(id) {
            return;
        }
        self.script_runtime.pending_start_flags[slot] = Some(id);
        self.script_runtime.pending_start_scripts.push(id);
    }

    #[inline(always)]
    pub(crate) fn unqueue_start_script(&mut self, id: NodeID) {
        let slot = id.index() as usize;
        if slot < self.script_runtime.pending_start_flags.len()
            && self.script_runtime.pending_start_flags[slot] == Some(id)
        {
            self.script_runtime.pending_start_flags[slot] = None;
        }
    }

    #[inline(always)]
    pub(crate) fn call_start_script(&mut self, id: NodeID) {
        let (instance_index, behavior, flags) = match self.scripts.instance_index_for_id(id) {
            Some(instance_index) => match self
                .scripts
                .get_instance_scheduled_indexed(instance_index, id)
            {
                Some(instance) => (
                    instance_index,
                    Arc::clone(&instance.behavior),
                    instance.behavior.script_flags(),
                ),
                None => return,
            },
            None => return,
        };
        if !flags.has_all_init() {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
            ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
            unsafe { InputWindow::new(&*input_ptr) };
        let _dlc_self_context = self.push_script_dlc_self_context(id);
        self.push_active_script_with_context(instance_index, id, self.script_callback_context());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id,
        };
        behavior.on_all_init(&mut sctx);
        self.pop_active_script(instance_index, id);
    }

    #[inline(always)]
    pub(crate) fn call_removal_script(&mut self, id: NodeID) {
        let (instance_index, behavior, flags) = match self.scripts.instance_index_for_id(id) {
            Some(instance_index) => match self
                .scripts
                .get_instance_scheduled_indexed(instance_index, id)
            {
                Some(instance) => (
                    instance_index,
                    Arc::clone(&instance.behavior),
                    instance.behavior.script_flags(),
                ),
                None => return,
            },
            None => return,
        };
        if !flags.has_removal() {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
            ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
            unsafe { InputWindow::new(&*input_ptr) };
        let _dlc_self_context = self.push_script_dlc_self_context(id);
        self.push_active_script_with_context(instance_index, id, self.script_callback_context());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id,
        };
        behavior.on_removal(&mut sctx);
        self.pop_active_script(instance_index, id);
    }

    #[inline(always)]
    pub(crate) fn remove_script_instance(&mut self, id: NodeID) -> bool {
        if !self.script_runtime.removing_scripts.insert(id) {
            return false;
        }
        self.call_removal_script(id);
        self.unqueue_start_script(id);
        self.signal_runtime.registry.disconnect_script(id);
        self.script_runtime.script_instance_dlc_mounts.remove(&id);
        let removed = self.scripts.remove(id).is_some();
        self.script_runtime.removing_scripts.remove(&id);
        removed
    }

    #[inline(always)]
    /// Call a scheduled update script with prebuilt resource/input windows.
    ///
    /// The behavior `Arc` is cloned before creating `RuntimeWindow`. That ends
    /// the immutable borrow of `self.scripts`, so the callback can receive a
    /// mutable runtime window while still holding a stable behavior handle.
    pub(crate) fn call_update_script_scheduled_with_context(
        &mut self,
        instance_index: usize,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input_api::InputSnapshot>,
    ) {
        let behavior = match self.scripts.scheduled_instance(
            instance_index,
            id,
            crate::cns::script_collection::ScheduleKind::Update,
        ) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        self.push_active_script_with_context(instance_index, id, self.script_callback_context());
        let _dlc_self_context = self.push_script_dlc_self_context(id);
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res,
            ipt,
            id,
        };
        behavior.on_update(&mut sctx);
        self.pop_active_script(instance_index, id);
    }

    #[inline(always)]
    /// Call a scheduled fixed-update script with prebuilt resource/input windows.
    ///
    /// The `(instance_index, id)` pair comes from a schedule snapshot. The
    /// collection revalidates it before dispatch so removed/reused slots do not
    /// call stale script state.
    pub(crate) fn call_fixed_update_script_scheduled_with_context(
        &mut self,
        instance_index: usize,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input_api::InputSnapshot>,
    ) {
        let behavior = match self.scripts.scheduled_instance(
            instance_index,
            id,
            crate::cns::script_collection::ScheduleKind::Fixed,
        ) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        self.push_active_script_with_context(instance_index, id, self.script_callback_context());
        let _dlc_self_context = self.push_script_dlc_self_context(id);
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res,
            ipt,
            id,
        };
        behavior.on_fixed_update(&mut sctx);
        self.pop_active_script(instance_index, id);
    }
}

impl ScriptAPI for Runtime {
    fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        if let Some(&(instance_index, active_id)) = self.script_runtime.active_script_stack.last()
            && active_id == script_id
        {
            return self
                .scripts
                .with_state_scheduled(instance_index, script_id, f);
        }
        self.scripts.with_state(script_id, f)
    }

    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        if let Some(&(instance_index, active_id)) = self.script_runtime.active_script_stack.last()
            && active_id == script_id
        {
            return self
                .scripts
                .with_state_mut_scheduled(instance_index, script_id, f);
        }
        self.scripts.with_state_mut(script_id, f)
    }

    fn script_attach(&mut self, node_id: NodeID, script_path: &str) -> bool {
        self.script_attach_with_vars(node_id, script_path, Vec::new())
    }

    fn script_attach_with_vars(
        &mut self,
        node_id: NodeID,
        script_path: &str,
        vars: Vec<(ScriptMemberID, Variant)>,
    ) -> bool {
        let Some(project) = self.project() else {
            return false;
        };
        let project_root = project.root.clone();
        let project_name = project.config.name.clone();

        if self
            .ensure_dynamic_script_registry_loaded(&project_root, &project_name)
            .is_err()
        {
            return false;
        }

        self.attach_script_instance(node_id, string_to_u64(script_path), None, vars)
            .is_ok()
    }

    fn script_attach_hashed(&mut self, node_id: NodeID, script_path_hash: u64) -> bool {
        let Some(project) = self.project() else {
            return false;
        };
        let project_root = project.root.clone();
        let project_name = project.config.name.clone();

        if self
            .ensure_dynamic_script_registry_loaded(&project_root, &project_name)
            .is_err()
        {
            return false;
        }

        self.attach_script_instance(node_id, script_path_hash, None, Vec::new())
            .is_ok()
    }

    fn script_detach(&mut self, node_id: NodeID) -> bool {
        self.remove_script_instance(node_id)
    }

    fn remove_script(&mut self, script_id: NodeID) -> bool {
        self.remove_script_instance(script_id)
    }

    fn script_set_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool {
        self.scripts.set_update_enabled(script_id, enabled)
    }

    fn script_set_fixed_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool {
        self.scripts.set_fixed_update_enabled(script_id, enabled)
    }

    fn get_var(&mut self, script_id: NodeID, member: ScriptMemberID) -> Variant {
        self.scripts
            .with_instance(script_id, |instance| {
                instance.behavior.get_var(instance.state.as_ref(), member)
            })
            .unwrap_or(Variant::Null)
    }

    fn set_var(&mut self, script_id: NodeID, member: ScriptMemberID, value: Variant) {
        let _ = self.scripts.with_instance_mut(script_id, |instance| {
            instance
                .behavior
                .set_var(instance.state.as_mut(), member, value);
        });
    }

    fn call_method(
        &mut self,
        script_id: NodeID,
        method: ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        let (instance_index, behavior) = match self.scripts.instance_index_for_id(script_id) {
            Some(i) => {
                let behavior = match self.scripts.get_instance_scheduled_indexed(i, script_id) {
                    Some(instance) => Arc::clone(&instance.behavior),
                    None => return Variant::Null,
                };
                (i, behavior)
            }
            None => return Variant::Null,
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
        self.push_active_script_with_context(instance_index, script_id, context);
        let _dlc_self_context = self.push_script_dlc_self_context(script_id);
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id: script_id,
        };
        let out = behavior.call_method(method, &mut sctx, params);
        self.pop_active_script(instance_index, script_id);
        out
    }
}

#[cfg(test)]
mod active_script_stack_tests {
    use super::*;
    use perro_nodes::{NodeType, SceneNode, SceneNodeData};
    use perro_runtime_api::sub_apis::{ScriptAPI, SignalAPI};
    use perro_scene::{Scene, SceneKey, SceneNodeEntry};
    use perro_scripting::{ScriptBehavior, ScriptFlags, ScriptLifecycle};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::{any::Any, borrow::Cow};

    static REMOVAL_CALLS: AtomicUsize = AtomicUsize::new(0);
    static REATTACH_SUCCEEDED: AtomicBool = AtomicBool::new(false);
    const REPLACEMENT_HASH: u64 = 7;

    struct SelfRemovingScript;

    struct ReattachingRemovalScript;

    impl ScriptLifecycle<crate::RuntimeScriptApi> for ReattachingRemovalScript {
        fn on_removal(&self, ctx: &mut ScriptContext<'_, crate::RuntimeScriptApi>) {
            REATTACH_SUCCEEDED.store(
                ctx.run
                    .Scripts()
                    .script_attach_hashed(ctx.id, REPLACEMENT_HASH),
                Ordering::Relaxed,
            );
        }
    }

    impl ScriptBehavior<crate::RuntimeScriptApi> for ReattachingRemovalScript {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(ScriptFlags::HAS_REMOVAL)
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
            _ctx: &mut ScriptContext<'_, crate::RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            Variant::Null
        }
    }

    struct ReplacementScript;

    impl ScriptLifecycle<crate::RuntimeScriptApi> for ReplacementScript {}

    impl ScriptBehavior<crate::RuntimeScriptApi> for ReplacementScript {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(0)
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
            _ctx: &mut ScriptContext<'_, crate::RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            Variant::Null
        }
    }

    fn replacement_script_ctor() -> *mut dyn ScriptBehavior<crate::RuntimeScriptApi> {
        Box::into_raw(Box::new(ReplacementScript))
    }

    static REPLACEMENT_REGISTRY: [(
        u64,
        perro_scripting::ScriptConstructor<crate::RuntimeScriptApi>,
    ); 1] = [(REPLACEMENT_HASH, replacement_script_ctor)];

    const BOOT_DATA: perro_scene::SceneNodeData =
        perro_scene::SceneNodeData::new(NodeType::Node, Cow::Borrowed(&[]), None);
    const BOOT_NODES: &[SceneNodeEntry] = &[SceneNodeEntry {
        data: BOOT_DATA,
        has_data_override: true,
        key: SceneKey(0),
        name: Some(Cow::Borrowed("root")),
        tags: Cow::Borrowed(&[]),
        children: Cow::Borrowed(&[]),
        parent: None,
        script: None,
        clear_script: false,
        root_of: None,
        script_vars: Cow::Borrowed(&[]),
    }];
    static BOOT_SCENE: Scene = Scene {
        nodes: Cow::Borrowed(BOOT_NODES),
        root: Some(SceneKey(0)),
        key_names: Cow::Borrowed(&[]),
    };

    fn empty_scene_lookup(_path_hash: u64) -> &'static Scene {
        &BOOT_SCENE
    }

    impl ScriptLifecycle<crate::RuntimeScriptApi> for SelfRemovingScript {
        fn on_removal(&self, ctx: &mut ScriptContext<'_, crate::RuntimeScriptApi>) {
            REMOVAL_CALLS.fetch_add(1, Ordering::Relaxed);
            assert!(!ctx.run.Scripts().remove(ctx.id));
        }
    }

    impl ScriptBehavior<crate::RuntimeScriptApi> for SelfRemovingScript {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(ScriptFlags::HAS_REMOVAL)
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
            _ctx: &mut ScriptContext<'_, crate::RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            Variant::Null
        }
    }

    #[test]
    fn removal_callback_cannot_reenter_same_script_removal() {
        REMOVAL_CALLS.store(0, Ordering::Relaxed);
        let mut runtime = Runtime::new();
        let id = NodeID::new(1);
        runtime
            .scripts
            .insert(id, Arc::new(SelfRemovingScript), Box::new(()));

        assert!(runtime.remove_script_instance(id));

        assert_eq!(REMOVAL_CALLS.load(Ordering::Relaxed), 1);
        assert!(runtime.scripts.get_instance(id).is_none());
        assert!(runtime.script_runtime.removing_scripts.is_empty());
        assert!(runtime.script_runtime.active_script_stack.is_empty());
    }

    #[test]
    fn removal_callback_cannot_replace_same_script() {
        REATTACH_SUCCEEDED.store(false, Ordering::Relaxed);
        let mut project = crate::runtime_project::RuntimeProject::new("Test", ".");
        project.static_scene_lookup = Some(empty_scene_lookup);
        let mut runtime = Runtime::from_project_with_script_registry(
            project,
            crate::runtime_project::ProviderMode::Static,
            Some(&REPLACEMENT_REGISTRY),
        );
        let id = runtime.nodes.insert(SceneNode::new(SceneNodeData::Node));
        runtime
            .scripts
            .insert(id, Arc::new(ReattachingRemovalScript), Box::new(()));

        assert!(runtime.remove_script_instance(id));

        assert!(!REATTACH_SUCCEEDED.load(Ordering::Relaxed));
        assert!(runtime.scripts.get_instance(id).is_none());
        assert!(runtime.script_runtime.removing_scripts.is_empty());
    }

    #[test]
    fn nested_active_script_pop_restores_parent_frame() {
        let mut runtime = Runtime::new();
        let parent = NodeID::new(1);
        let child = NodeID::new(2);
        let context = runtime.script_callback_context();

        runtime.push_active_script_with_context(10, parent, context);
        runtime.push_active_script_with_context(20, child, context);
        runtime.pop_active_script(20, child);

        assert_eq!(
            runtime.script_runtime.active_script_stack.last().copied(),
            Some((10, parent))
        );

        runtime.pop_active_script(10, parent);
        assert!(runtime.script_runtime.active_script_stack.is_empty());
    }

    #[test]
    fn active_callback_context_lives_until_outer_pop() {
        let mut runtime = Runtime::new();
        let parent = NodeID::new(1);
        let child = NodeID::new(2);
        let context = runtime.script_callback_context();

        runtime.push_active_script_with_context(10, parent, context);
        runtime.push_active_script_with_context(20, child, context);
        runtime.pop_active_script(20, child);

        assert!(runtime.current_script_callback_context().is_some());

        runtime.pop_active_script(10, parent);
        assert!(runtime.current_script_callback_context().is_none());
    }

    #[derive(Debug, Default)]
    struct ChainState {
        value: i64,
    }

    #[derive(Clone, Copy)]
    struct ChainScript {
        role: ChainRole,
        a: NodeID,
        b: NodeID,
        c: NodeID,
        d: NodeID,
        signal: perro_ids::SignalID,
    }

    #[derive(Clone, Copy)]
    enum ChainRole {
        A,
        B,
        C,
        D,
    }

    const GO: ScriptMemberID = ScriptMemberID(1);
    const PING: ScriptMemberID = ScriptMemberID(2);

    impl ScriptLifecycle<crate::RuntimeScriptApi> for ChainScript {}

    impl ScriptBehavior<crate::RuntimeScriptApi> for ChainScript {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(ScriptFlags::NONE)
        }

        fn create_state(&self) -> Box<dyn Any> {
            Box::<ChainState>::default()
        }

        fn get_var(&self, _state: &dyn Any, _var: ScriptMemberID) -> Variant {
            Variant::Null
        }

        fn set_var(&self, _state: &mut dyn Any, _var: ScriptMemberID, _value: Variant) {}

        fn call_method(
            &self,
            method: ScriptMemberID,
            ctx: &mut ScriptContext<'_, crate::RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            match (self.role, method) {
                (ChainRole::A, GO) => {
                    ctx.run
                        .Scripts()
                        .with_state_mut::<ChainState, _, _>(ctx.id, |state| {
                            state.value = 1;
                        });
                    let _ = ctx.run.Scripts().call_method(self.b, GO, &[]);
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.b, |state| state.value),
                        Some(10)
                    );
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.c, |state| state.value),
                        Some(50)
                    );
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.d, |state| state.value),
                        Some(100)
                    );
                }
                (ChainRole::B, GO) => {
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.a, |state| state.value),
                        Some(1)
                    );
                    ctx.run
                        .Scripts()
                        .with_state_mut::<ChainState, _, _>(ctx.id, |state| {
                            state.value = 10;
                        });
                    assert_eq!(ctx.run.Signals().emit(self.signal, &[]), 1);
                    let _ = ctx.run.Scripts().call_method(self.c, GO, &[]);
                }
                (ChainRole::C, GO) => {
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.b, |state| state.value),
                        Some(10)
                    );
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.d, |state| state.value),
                        Some(100)
                    );
                    ctx.run
                        .Scripts()
                        .with_state_mut::<ChainState, _, _>(ctx.id, |state| {
                            state.value = 50;
                        });
                }
                (ChainRole::D, PING) => {
                    assert_eq!(
                        ctx.run
                            .Scripts()
                            .with_state::<ChainState, _, _>(self.b, |state| state.value),
                        Some(10)
                    );
                    ctx.run
                        .Scripts()
                        .with_state_mut::<ChainState, _, _>(ctx.id, |state| {
                            state.value = 100;
                        });
                }
                _ => {}
            }
            Variant::Null
        }
    }

    #[test]
    fn deep_script_chain_reads_latest_cross_script_state() {
        let mut runtime = Runtime::new();
        let a = NodeID::new(1);
        let b = NodeID::new(2);
        let c = NodeID::new(3);
        let d = NodeID::new(4);
        let signal = perro_ids::SignalID::from_u64(42);

        for (id, role) in [
            (a, ChainRole::A),
            (b, ChainRole::B),
            (c, ChainRole::C),
            (d, ChainRole::D),
        ] {
            runtime.scripts.insert(
                id,
                Arc::new(ChainScript {
                    role,
                    a,
                    b,
                    c,
                    d,
                    signal,
                }),
                Box::<ChainState>::default(),
            );
        }
        assert!(SignalAPI::signal_connect(
            &mut runtime,
            d,
            signal,
            PING,
            &[]
        ));

        let out = ScriptAPI::call_method(&mut runtime, a, GO, &[]);
        assert_eq!(out, Variant::Null);
        assert!(runtime.script_runtime.active_script_stack.is_empty());
        assert!(runtime.current_script_callback_context().is_none());
    }
}
