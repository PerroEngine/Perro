use perro_context::{RuntimeContext, sub_apis::ScriptAPI};
use perro_ids::{NodeID, ScriptMemberID};
use perro_variant::Variant;
use std::sync::Arc;

use crate::Runtime;

impl Runtime {
    #[inline(always)]
    pub(crate) fn queue_start_script(&mut self, id: NodeID) {
        let slot = id.index() as usize;
        if self.pending_start_flags.len() <= slot {
            self.pending_start_flags.resize(slot + 1, None);
        }
        if self.pending_start_flags[slot] == Some(id) {
            return;
        }
        self.pending_start_flags[slot] = Some(id);
        self.pending_start_scripts.push(id);
    }

    #[inline(always)]
    pub(crate) fn unqueue_start_script(&mut self, id: NodeID) {
        let slot = id.index() as usize;
        if slot < self.pending_start_flags.len() && self.pending_start_flags[slot] == Some(id) {
            self.pending_start_flags[slot] = None;
        }
    }

    #[inline(always)]
    pub(crate) fn call_start_script(&mut self, id: NodeID) {
        let (behavior, flags) = match self.scripts.get_instance(id) {
            Some(instance) => (Arc::clone(&instance.behavior), instance.behavior.script_flags()),
            None => return,
        };
        if !flags.has_all_init() {
            return;
        }
        let mut ctx = RuntimeContext::new(self);
        behavior.on_all_init(&mut ctx, id);
    }

    #[inline(always)]
    pub(crate) fn call_removal_script(&mut self, id: NodeID) {
        let (behavior, flags) = match self.scripts.get_instance(id) {
            Some(instance) => (Arc::clone(&instance.behavior), instance.behavior.script_flags()),
            None => return,
        };
        if !flags.has_removal() {
            return;
        }
        let mut ctx = RuntimeContext::new(self);
        behavior.on_removal(&mut ctx, id);
    }

    #[inline(always)]
    pub(crate) fn remove_script_instance(&mut self, id: NodeID) -> bool {
        self.call_removal_script(id);
        self.unqueue_start_script(id);
        self.scripts.remove(id).is_some()
    }

    #[inline(always)]
    pub(crate) fn call_update_script_scheduled(&mut self, instance_index: usize, id: NodeID) {
        let behavior = match self
            .scripts
            .get_instance_scheduled_indexed(instance_index, id)
        {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        let mut ctx = RuntimeContext::new(self);
        behavior.on_update(&mut ctx, id);
    }

    #[inline(always)]
    pub(crate) fn call_fixed_update_script_scheduled(&mut self, instance_index: usize, id: NodeID) {
        let behavior = match self
            .scripts
            .get_instance_scheduled_indexed(instance_index, id)
        {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return,
        };
        let mut ctx = RuntimeContext::new(self);
        behavior.on_fixed_update(&mut ctx, id);
    }
}

impl ScriptAPI for Runtime {
    fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        self.scripts.with_state(script_id, f)
    }

    fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        self.scripts.with_state_mut(script_id, f)
    }

    fn attach_script(&mut self, node_id: NodeID, script_path: &str) -> bool {
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

        self.attach_script_instance(node_id, script_path).is_ok()
    }

    fn detach_script(&mut self, node_id: NodeID) -> bool {
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
        method_id: ScriptMemberID,
        params: &[Variant],
    ) -> Variant {
        let behavior = match self.scripts.get_instance(script_id) {
            Some(instance) => Arc::clone(&instance.behavior),
            None => return Variant::Null,
        };
        let mut ctx = RuntimeContext::new(self);
        behavior.call_method(method_id, &mut ctx, script_id, params)
    }
}
