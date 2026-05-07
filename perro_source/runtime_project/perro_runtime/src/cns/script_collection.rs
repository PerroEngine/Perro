use perro_ids::NodeID;
use perro_scripting::ScriptBehavior;
use std::any::{Any, TypeId};
use std::sync::Arc;

pub(crate) struct ScriptInstance {
    pub(crate) behavior: Arc<dyn ScriptBehavior<crate::runtime::RuntimeScriptApi>>,
    pub(crate) state_type: TypeId,
    pub(crate) state: Box<dyn Any>,
}

pub(crate) struct ScriptCollection {
    instances: Vec<ScriptInstance>,
    ids: Vec<NodeID>,
    // NodeID.index() -> instance index. Lookup validates full NodeID equality.
    index: Vec<u32>,

    update: Vec<usize>,
    fixed: Vec<usize>,

    // instance index -> schedule position
    update_pos: Vec<u32>,
    fixed_pos: Vec<u32>,
    schedule_epoch: u64,
}

const NONE_SLOT: u32 = u32::MAX;

impl ScriptCollection {
    pub(crate) fn new() -> Self {
        Self {
            instances: Vec::new(),
            ids: Vec::new(),
            index: Vec::new(),
            update: Vec::new(),
            fixed: Vec::new(),
            update_pos: Vec::new(),
            fixed_pos: Vec::new(),
            schedule_epoch: 0,
        }
    }

    pub(crate) fn get_instance(&self, id: NodeID) -> Option<&ScriptInstance> {
        let i = self.instance_index_for(id)?;
        self.instances.get(i)
    }

    #[inline]
    pub(crate) fn instance_index_for_id(&self, id: NodeID) -> Option<usize> {
        self.instance_index_for(id)
    }

    #[inline]
    pub(crate) fn get_instance_scheduled_indexed(
        &self,
        instance_index: usize,
        id: NodeID,
    ) -> Option<&ScriptInstance> {
        if self.ids.get(instance_index).copied() != Some(id) {
            return None;
        }
        self.instances.get(instance_index)
    }

    #[inline]
    pub(crate) fn with_instance<V, F>(&self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&ScriptInstance) -> V,
    {
        let i = self.instance_index_for(id)?;
        Some(f(self.instances.get(i)?))
    }

    #[inline]
    pub(crate) fn with_instance_mut<V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut ScriptInstance) -> V,
    {
        let i = self.instance_index_for(id)?;
        Some(f(self.instances.get_mut(i)?))
    }

    pub(crate) fn insert(
        &mut self,
        id: NodeID,
        behavior: Arc<dyn ScriptBehavior<crate::runtime::RuntimeScriptApi>>,
        state: Box<dyn Any>,
    ) {
        let flags = behavior.script_flags();
        let state_type = state.as_ref().type_id();

        if let Some(i) = self.instance_index_for(id) {
            self.instances[i] = ScriptInstance {
                behavior,
                state_type,
                state,
            };
            self.rebuild_schedules_for_index(i, flags);
            self.bump_schedule_epoch();
            return;
        }

        // If this slot maps to a stale generation, remove that stale instance first.
        let slot = id.index() as usize;
        if let Some(existing_i) = self.slot_value(&self.index, slot)
            && self.ids.get(existing_i).copied() != Some(id)
        {
            let stale = self.ids[existing_i];
            let _ = self.remove(stale);
        }

        let i = self.instances.len();
        self.instances.push(ScriptInstance {
            behavior,
            state_type,
            state,
        });
        self.ids.push(id);
        self.set_index_slot(slot, Some(i));

        if flags.has_update() {
            let pos = self.update.len();
            self.update.push(i);
            Self::set_reverse_slot(&mut self.update_pos, i, Some(pos));
        }
        if flags.has_fixed_update() {
            let pos = self.fixed.len();
            self.fixed.push(i);
            Self::set_reverse_slot(&mut self.fixed_pos, i, Some(pos));
        }
        self.bump_schedule_epoch();
    }

    pub(crate) fn remove(&mut self, id: NodeID) -> Option<ScriptInstance> {
        let i = self.instance_index_for(id)?;
        self.set_index_slot(id.index() as usize, None);
        self.remove_from_schedules_by_index(i);

        let last = self.instances.len() - 1;
        self.instances.swap(i, last);
        self.ids.swap(i, last);

        let removed = self.instances.pop().unwrap();
        let removed_node = self.ids.pop().unwrap();
        debug_assert!(removed_node == id);

        if i != last {
            let moved = self.ids[i];
            self.set_index_slot(moved.index() as usize, Some(i));

            if let Some(pos) = Self::take_reverse_slot(&mut self.update_pos, last) {
                self.update[pos] = i;
                Self::set_reverse_slot(&mut self.update_pos, i, Some(pos));
            }
            if let Some(pos) = Self::take_reverse_slot(&mut self.fixed_pos, last) {
                self.fixed[pos] = i;
                Self::set_reverse_slot(&mut self.fixed_pos, i, Some(pos));
            }
        }

        self.bump_schedule_epoch();
        Some(removed)
    }

    pub(crate) fn reset_state(&mut self, id: NodeID) -> bool {
        let Some(i) = self.instance_index_for(id) else {
            return false;
        };
        let Some(instance) = self.instances.get_mut(i) else {
            return false;
        };

        let state = instance.behavior.create_state();
        instance.state_type = state.as_ref().type_id();
        instance.state = state;
        true
    }

    pub(crate) fn append_update_slots(&self, out: &mut Vec<(usize, NodeID)>) {
        for &i in &self.update {
            out.push((i, self.ids[i]));
        }
    }

    #[inline]
    pub(crate) fn update_schedule_len(&self) -> usize {
        self.update.len()
    }

    pub(crate) fn append_fixed_update_slots(&self, out: &mut Vec<(usize, NodeID)>) {
        for &i in &self.fixed {
            out.push((i, self.ids[i]));
        }
    }

    pub(crate) fn append_instance_ids(&self, out: &mut Vec<NodeID>) {
        out.extend(self.ids.iter().copied());
    }

    #[inline]
    pub(crate) fn fixed_schedule_len(&self) -> usize {
        self.fixed.len()
    }

    #[inline]
    pub(crate) fn schedule_epoch(&self) -> u64 {
        self.schedule_epoch
    }

    pub(crate) fn with_state<T: 'static, V, F>(&self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        let i = self.instance_index_for(id)?;
        let instance = self.instances.get(i)?;
        if instance.state_type != TypeId::of::<T>() {
            return None;
        }

        let state = unsafe { &*(instance.state.as_ref() as *const dyn Any as *const T) };
        Some(f(state))
    }

    pub(crate) fn with_state_scheduled<T: 'static, V, F>(
        &self,
        instance_index: usize,
        id: NodeID,
        f: F,
    ) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        if self.ids.get(instance_index).copied() != Some(id) {
            return None;
        }
        let instance = self.instances.get(instance_index)?;
        if instance.state_type != TypeId::of::<T>() {
            return None;
        }

        let state = unsafe { &*(instance.state.as_ref() as *const dyn Any as *const T) };
        Some(f(state))
    }

    pub(crate) fn with_state_mut<T: 'static, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        let i = self.instance_index_for(id)?;
        let instance = self.instances.get_mut(i)?;
        if instance.state_type != TypeId::of::<T>() {
            return None;
        }

        let state = unsafe { &mut *(instance.state.as_mut() as *mut dyn Any as *mut T) };
        Some(f(state))
    }

    pub(crate) fn with_state_mut_scheduled<T: 'static, V, F>(
        &mut self,
        instance_index: usize,
        id: NodeID,
        f: F,
    ) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        if self.ids.get(instance_index).copied() != Some(id) {
            return None;
        }
        let instance = self.instances.get_mut(instance_index)?;
        if instance.state_type != TypeId::of::<T>() {
            return None;
        }

        let state = unsafe { &mut *(instance.state.as_mut() as *mut dyn Any as *mut T) };
        Some(f(state))
    }

    #[inline]
    fn instance_index_for(&self, id: NodeID) -> Option<usize> {
        let slot = id.index() as usize;
        let i = self.slot_value(&self.index, slot)?;
        (self.ids.get(i).copied() == Some(id)).then_some(i)
    }

    #[inline]
    fn set_index_slot(&mut self, slot: usize, value: Option<usize>) {
        if self.index.len() <= slot {
            self.index.resize(slot + 1, NONE_SLOT);
        }
        self.index[slot] = value
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(NONE_SLOT);
    }

    fn remove_from_schedules_by_index(&mut self, i: usize) {
        if let Some(pos) = Self::take_reverse_slot(&mut self.update_pos, i) {
            let last_pos = self.update.len() - 1;
            self.update.swap_remove(pos);

            if pos != last_pos {
                let moved_idx = self.update[pos];
                Self::set_reverse_slot(&mut self.update_pos, moved_idx, Some(pos));
            }
        }

        if let Some(pos) = Self::take_reverse_slot(&mut self.fixed_pos, i) {
            let last_pos = self.fixed.len() - 1;
            self.fixed.swap_remove(pos);

            if pos != last_pos {
                let moved_idx = self.fixed[pos];
                Self::set_reverse_slot(&mut self.fixed_pos, moved_idx, Some(pos));
            }
        }
    }

    fn rebuild_schedules_for_index(&mut self, i: usize, flags: perro_scripting::ScriptFlags) {
        self.remove_from_schedules_by_index(i);

        if flags.has_update() {
            let pos = self.update.len();
            self.update.push(i);
            Self::set_reverse_slot(&mut self.update_pos, i, Some(pos));
        }
        if flags.has_fixed_update() {
            let pos = self.fixed.len();
            self.fixed.push(i);
            Self::set_reverse_slot(&mut self.fixed_pos, i, Some(pos));
        }
    }

    #[inline]
    fn set_reverse_slot(slots: &mut Vec<u32>, index: usize, value: Option<usize>) {
        if slots.len() <= index {
            slots.resize(index + 1, NONE_SLOT);
        }
        slots[index] = value
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(NONE_SLOT);
    }

    #[inline]
    fn take_reverse_slot(slots: &mut [u32], index: usize) -> Option<usize> {
        if index >= slots.len() {
            return None;
        }
        let value = slots[index];
        slots[index] = NONE_SLOT;
        if value == NONE_SLOT {
            None
        } else {
            Some(value as usize)
        }
    }

    #[inline]
    fn slot_value(&self, slots: &[u32], index: usize) -> Option<usize> {
        let value = *slots.get(index)?;
        if value == NONE_SLOT {
            None
        } else {
            Some(value as usize)
        }
    }

    #[inline]
    fn bump_schedule_epoch(&mut self) {
        self.schedule_epoch = self.schedule_epoch.wrapping_add(1);
    }
}

impl Default for ScriptCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::ScriptMemberID;
    use perro_runtime_context::sub_apis::{Attribute, Member};
    use perro_scripting::{ScriptContext, ScriptFlags, ScriptLifecycle};
    use perro_variant::Variant;

    struct DummyBehavior;

    impl ScriptLifecycle<crate::runtime::RuntimeScriptApi> for DummyBehavior {}

    impl perro_scripting::ScriptBehavior<crate::runtime::RuntimeScriptApi> for DummyBehavior {
        fn script_flags(&self) -> ScriptFlags {
            ScriptFlags::new(ScriptFlags::NONE)
        }

        fn create_state(&self) -> Box<dyn Any> {
            Box::new(5_i32)
        }

        fn get_var(&self, _state: &dyn Any, _var: ScriptMemberID) -> Variant {
            Variant::Null
        }

        fn set_var(&self, _state: &mut dyn Any, _var: ScriptMemberID, _value: &Variant) {}

        fn call_method(
            &self,
            _method: ScriptMemberID,
            _ctx: &mut ScriptContext<'_, crate::runtime::RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            Variant::Null
        }

        fn attributes_of(&self, _member: &str) -> &'static [Attribute] {
            &[]
        }

        fn members_with(&self, _attribute: &str) -> &'static [Member] {
            &[]
        }

        fn has_attribute(&self, _member: &str, _attribute: &str) -> bool {
            false
        }
    }

    #[test]
    fn reset_state_replaces_mutated_state_with_default() {
        let mut scripts = ScriptCollection::new();
        let id = NodeID::new(42);
        scripts.insert(id, Arc::new(DummyBehavior), Box::new(99_i32));

        let _ = scripts.with_state_mut::<i32, _, _>(id, |state| *state = 123);
        assert_eq!(
            scripts.with_state::<i32, _, _>(id, |state| *state),
            Some(123)
        );

        assert!(scripts.reset_state(id));
        assert_eq!(scripts.with_state::<i32, _, _>(id, |state| *state), Some(5));
    }
}
