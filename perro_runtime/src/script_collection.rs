use perro_api::api::RuntimeAPI;
use perro_ids::NodeID;
use perro_scripting::ScriptBehavior;
use std::any::{Any, TypeId};
use std::sync::Arc;

pub(crate) struct ScriptInstance<R: RuntimeAPI + ?Sized> {
    pub(crate) behavior: Arc<dyn ScriptBehavior<R>>,
    pub(crate) state_type: TypeId,
    pub(crate) state: Box<dyn Any>,
}

pub(crate) struct ScriptCollection<R: RuntimeAPI + ?Sized> {
    instances: Vec<ScriptInstance<R>>,
    ids: Vec<NodeID>,
    // NodeID.index() -> instance index. Lookup validates full NodeID equality.
    index: Vec<Option<usize>>,

    update: Vec<usize>,
    fixed: Vec<usize>,

    // instance index -> schedule position
    update_pos: Vec<Option<usize>>,
    fixed_pos: Vec<Option<usize>>,
}

impl<R: RuntimeAPI + ?Sized> ScriptCollection<R> {
    pub(crate) fn new() -> Self {
        Self {
            instances: Vec::new(),
            ids: Vec::new(),
            index: Vec::new(),
            update: Vec::new(),
            fixed: Vec::new(),
            update_pos: Vec::new(),
            fixed_pos: Vec::new(),
        }
    }

    pub(crate) fn get_instance(&self, id: NodeID) -> Option<&ScriptInstance<R>> {
        let i = self.instance_index_for_id(id)?;
        self.instances.get(i)
    }

    #[inline]
    pub(crate) fn get_instance_scheduled_indexed(
        &self,
        instance_index: usize,
        id: NodeID,
    ) -> Option<&ScriptInstance<R>> {
        if self.ids.get(instance_index).copied() != Some(id) {
            return None;
        }
        self.instances.get(instance_index)
    }

    #[inline]
    pub(crate) fn with_instance<V, F>(&self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&ScriptInstance<R>) -> V,
    {
        let i = self.instance_index_for_id(id)?;
        Some(f(self.instances.get(i)?))
    }

    #[inline]
    pub(crate) fn with_instance_mut<V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut ScriptInstance<R>) -> V,
    {
        let i = self.instance_index_for_id(id)?;
        Some(f(self.instances.get_mut(i)?))
    }

    pub(crate) fn insert(
        &mut self,
        id: NodeID,
        behavior: Arc<dyn ScriptBehavior<R>>,
        state: Box<dyn Any>,
    ) {
        let flags = behavior.script_flags();
        let state_type = state.as_ref().type_id();

        if let Some(i) = self.instance_index_for_id(id) {
            self.instances[i] = ScriptInstance {
                behavior,
                state_type,
                state,
            };
            self.rebuild_schedules_for_index(i, flags);
            return;
        }

        // If this slot maps to a stale generation, remove that stale instance first.
        let slot = id.index() as usize;
        if let Some(Some(existing_i)) = self.index.get(slot).copied() {
            if self.ids.get(existing_i).copied() != Some(id) {
                let stale_id = self.ids[existing_i];
                let _ = self.remove(stale_id);
            }
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
    }

    pub(crate) fn remove(&mut self, id: NodeID) -> Option<ScriptInstance<R>> {
        let i = self.instance_index_for_id(id)?;
        self.set_index_slot(id.index() as usize, None);
        self.remove_from_schedules_by_index(i);

        let last = self.instances.len() - 1;
        self.instances.swap(i, last);
        self.ids.swap(i, last);

        let removed = self.instances.pop().unwrap();
        let removed_id = self.ids.pop().unwrap();
        debug_assert!(removed_id == id);

        if i != last {
            let moved_id = self.ids[i];
            self.set_index_slot(moved_id.index() as usize, Some(i));

            if let Some(pos) = Self::take_reverse_slot(&mut self.update_pos, last) {
                self.update[pos] = i;
                Self::set_reverse_slot(&mut self.update_pos, i, Some(pos));
            }
            if let Some(pos) = Self::take_reverse_slot(&mut self.fixed_pos, last) {
                self.fixed[pos] = i;
                Self::set_reverse_slot(&mut self.fixed_pos, i, Some(pos));
            }
        }

        Some(removed)
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

    #[inline]
    pub(crate) fn fixed_schedule_len(&self) -> usize {
        self.fixed.len()
    }

    pub(crate) fn with_state<T: 'static, V, F>(&self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        let i = self.instance_index_for_id(id)?;
        let instance = self.instances.get(i)?;
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
        let i = self.instance_index_for_id(id)?;
        let instance = self.instances.get_mut(i)?;
        if instance.state_type != TypeId::of::<T>() {
            return None;
        }

        let state = unsafe { &mut *(instance.state.as_mut() as *mut dyn Any as *mut T) };
        Some(f(state))
    }

    #[inline]
    fn instance_index_for_id(&self, id: NodeID) -> Option<usize> {
        let slot = id.index() as usize;
        let i = (*self.index.get(slot)?)?;
        (self.ids.get(i).copied() == Some(id)).then_some(i)
    }

    #[inline]
    fn set_index_slot(&mut self, slot: usize, value: Option<usize>) {
        if self.index.len() <= slot {
            self.index.resize(slot + 1, None);
        }
        self.index[slot] = value;
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
    fn set_reverse_slot(slots: &mut Vec<Option<usize>>, index: usize, value: Option<usize>) {
        if slots.len() <= index {
            slots.resize(index + 1, None);
        }
        slots[index] = value;
    }

    #[inline]
    fn take_reverse_slot(slots: &mut [Option<usize>], index: usize) -> Option<usize> {
        if index >= slots.len() {
            return None;
        }
        slots[index].take()
    }
}

impl<R: RuntimeAPI + ?Sized> Default for ScriptCollection<R> {
    fn default() -> Self {
        Self::new()
    }
}
