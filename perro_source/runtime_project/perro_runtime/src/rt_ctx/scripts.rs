use perro_ids::string_to_u64;
use perro_ids::{NodeID, ScriptMemberID};
use perro_input::InputWindow;
use perro_io::set_dlc_self_context;
use perro_resource_context::ResourceWindow;
use perro_runtime_context::{
    RuntimeWindow,
    sub_apis::{Attribute, Member, ScriptAPI},
};
use perro_scripting::ScriptContext;
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

impl Runtime {
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
        let (behavior, flags) = match self.scripts.get_instance(id) {
            Some(instance) => (
                Arc::clone(&instance.behavior),
                instance.behavior.script_flags(),
            ),
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
        let ipt: InputWindow<'_, perro_input::InputSnapshot> =
            unsafe { InputWindow::new(&*input_ptr) };
        let mount = self
            .script_runtime
            .script_instance_dlc_mounts
            .get(&id)
            .cloned();
        set_dlc_self_context(mount.as_deref());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id,
        };
        behavior.on_all_init(&mut sctx);
        set_dlc_self_context(None);
    }

    #[inline(always)]
    pub(crate) fn call_removal_script(&mut self, id: NodeID) {
        let (behavior, flags) = match self.scripts.get_instance(id) {
            Some(instance) => (
                Arc::clone(&instance.behavior),
                instance.behavior.script_flags(),
            ),
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
        let ipt: InputWindow<'_, perro_input::InputSnapshot> =
            unsafe { InputWindow::new(&*input_ptr) };
        let mount = self
            .script_runtime
            .script_instance_dlc_mounts
            .get(&id)
            .cloned();
        set_dlc_self_context(mount.as_deref());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id,
        };
        behavior.on_removal(&mut sctx);
        set_dlc_self_context(None);
    }

    #[inline(always)]
    pub(crate) fn remove_script_instance(&mut self, id: NodeID) -> bool {
        self.call_removal_script(id);
        self.unqueue_start_script(id);
        self.signal_runtime.registry.disconnect_script(id);
        self.script_runtime.script_instance_dlc_mounts.remove(&id);
        self.scripts.remove(id).is_some()
    }

    #[inline(always)]
    pub(crate) fn call_update_script_scheduled_with_context(
        &mut self,
        instance_index: usize,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input::InputSnapshot>,
    ) {
        let behavior = match self
            .scripts
            .get_instance_scheduled_indexed(instance_index, id)
        {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        let mount = self
            .script_runtime
            .script_instance_dlc_mounts
            .get(&id)
            .cloned();
        self.script_runtime
            .active_script_stack
            .push((instance_index, id));
        set_dlc_self_context(mount.as_deref());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res,
            ipt,
            id,
        };
        behavior.on_update(&mut sctx);
        set_dlc_self_context(None);
        let _ = self.script_runtime.active_script_stack.pop();
    }

    #[inline(always)]
    pub(crate) fn call_fixed_update_script_scheduled_with_context(
        &mut self,
        instance_index: usize,
        id: NodeID,
        res: &ResourceWindow<'_, crate::RuntimeResourceApi>,
        ipt: &InputWindow<'_, perro_input::InputSnapshot>,
    ) {
        let behavior = match self
            .scripts
            .get_instance_scheduled_indexed(instance_index, id)
        {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        let mount = self
            .script_runtime
            .script_instance_dlc_mounts
            .get(&id)
            .cloned();
        self.script_runtime
            .active_script_stack
            .push((instance_index, id));
        set_dlc_self_context(mount.as_deref());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res,
            ipt,
            id,
        };
        behavior.on_fixed_update(&mut sctx);
        set_dlc_self_context(None);
        let _ = self.script_runtime.active_script_stack.pop();
    }
}

impl ScriptAPI for Runtime {
    fn with_state<T: 'static, V: Default, F>(&mut self, script_id: NodeID, f: F) -> V
    where
        F: FnOnce(&T) -> V,
    {
        if let Some(&(instance_index, active_id)) = self.script_runtime.active_script_stack.last()
            && active_id == script_id
        {
            return self
                .scripts
                .with_state_scheduled(instance_index, script_id, f)
                .unwrap_or_default();
        }
        self.scripts.with_state(script_id, f).unwrap_or_default()
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

        self.attach_script_instance(node_id, string_to_u64(script_path), None, &[])
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

        self.attach_script_instance(node_id, script_path_hash, None, &[])
            .is_ok()
    }

    fn script_detach(&mut self, node_id: NodeID) -> bool {
        self.remove_script_instance(node_id)
    }

    fn remove_script(&mut self, script_id: NodeID) -> bool {
        self.remove_script_instance(script_id)
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
                .set_var(instance.state.as_mut(), member, &value);
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
        let resource_api = self.resource_api.clone();
        let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
            ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputWindow<'_, perro_input::InputSnapshot> =
            unsafe { InputWindow::new(&*input_ptr) };
        self.script_runtime
            .active_script_stack
            .push((instance_index, script_id));
        let mount = self
            .script_runtime
            .script_instance_dlc_mounts
            .get(&script_id)
            .cloned();
        set_dlc_self_context(mount.as_deref());
        let mut run = RuntimeWindow::new(self);
        let mut sctx = ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id: script_id,
        };
        let out = behavior.call_method(
            method,
            &mut sctx,
            params,
        );
        set_dlc_self_context(None);
        let _ = self.script_runtime.active_script_stack.pop();
        out
    }

    fn attributes_of(&mut self, script_id: NodeID, member: &str) -> &'static [Attribute] {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return &[],
        };
        behavior.attributes_of(member)
    }

    fn members_with(&mut self, script_id: NodeID, attribute: &str) -> &'static [Member] {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return &[],
        };
        behavior.members_with(attribute)
    }

    fn has_attribute(&mut self, script_id: NodeID, member: &str, attribute: &str) -> bool {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return false,
        };
        behavior.has_attribute(member, attribute)
    }
}

