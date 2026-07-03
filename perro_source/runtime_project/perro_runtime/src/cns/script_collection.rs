use perro_ids::NodeID;
use perro_scripting::{ScriptBehavior, state_mut_unchecked, state_ref_unchecked};
use std::any::{Any, TypeId};
use std::sync::Arc;

pub(crate) struct ScriptInstance {
    /// Shared behavior/vtable for this script definition.
    ///
    /// The behavior object is stored in an `Arc` so callback dispatch can clone
    /// a stable handle before borrowing the runtime mutably for `ScriptContext`.
    /// Per-node mutable data does not live here; it lives in `state`.
    pub(crate) behavior: Arc<dyn ScriptBehavior<crate::runtime::RuntimeScriptApi>>,
    /// Cached concrete state type.
    ///
    /// `with_state*` compares this to `TypeId::of::<T>()` before using the
    /// unchecked cast helpers. This keeps hot script state access to one type
    /// id compare instead of `Any` downcast dispatch.
    pub(crate) state_type: TypeId,
    /// Per-instance script state owned by the attached node.
    ///
    /// One behavior can serve many nodes, but each node owns its own boxed state.
    /// Runtime state helpers check `state_type`, then cast this box for the
    /// duration of a closure-scoped borrow.
    pub(crate) state: Box<dyn Any>,
}

/// Dense script instance store plus side indexes for hot runtime lookups.
///
/// Behavior/state model:
/// - `behavior` is shared through `Arc`; it holds generated method/field glue.
/// - `state` is a per-node `Box<dyn Any>` created by that behavior.
/// - `state_type` caches `TypeId` so hot state access avoids repeated downcast
///   dispatch.
/// - `with_state*` checks `TypeId`, then uses the unchecked cast helpers from
///   `perro_scripting`; the returned borrow only lives inside the provided
///   closure.
///
/// Layout:
/// - `instances[i]` owns behavior + state.
/// - `ids[i]` owns the node id for the same instance index.
/// - `index[node_id.index()]` maps a node slot to `i`, then full `NodeID`
///   equality rejects stale generations.
/// - `update` and `fixed` store instance indexes. Reverse arrays store each
///   schedule position, so enable/disable/remove stay O(1) via swap-remove.
///
/// Removing an instance may move the last instance into the removed slot. All
/// side indexes must be updated with the moved instance index in the same step.
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

/// Which per-frame schedule a scheduled lookup targets.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleKind {
    Update,
    Fixed,
}

impl ScriptCollection {
    // ---- Construction ----

    /// Create an empty script collection.
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

    // ---- Instance lookup ----

    /// Return a script instance by script node id.
    pub(crate) fn get_instance(&self, id: NodeID) -> Option<&ScriptInstance> {
        let i = self.instance_index_for(id)?;
        self.instances.get(i)
    }

    /// Return dense instance index for a script node id.
    #[inline]
    pub(crate) fn instance_index_for_id(&self, id: NodeID) -> Option<usize> {
        self.instance_index_for(id)
    }

    /// Return a scheduled instance when both dense index and node id still match.
    ///
    /// This protects scheduler snapshots from stale ids after removals and
    /// swap-remove compaction.
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

    /// Return a scheduled instance in one pass when it is still valid for the
    /// requested schedule.
    ///
    /// Fuses the dense-index/id revalidation of [`Self::get_instance_scheduled_indexed`]
    /// with the schedule-membership check the dispatch fns used to do separately
    /// (via `is_update_scheduled_indexed` / `is_fixed_update_scheduled_indexed`).
    /// Returns `None` when the id is stale/moved (swap-remove) OR when the script
    /// was removed from the requested schedule mid-frame (enable/disable). This is
    /// exactly the combined semantics of the two prior checks: a single reverse-pos
    /// != NONE_SLOT read plus the id match, then the instance.
    #[inline]
    pub(crate) fn scheduled_instance(
        &self,
        instance_index: usize,
        id: NodeID,
        schedule: ScheduleKind,
    ) -> Option<&ScriptInstance> {
        if self.ids.get(instance_index).copied() != Some(id) {
            return None;
        }
        let reverse = match schedule {
            ScheduleKind::Update => &self.update_pos,
            ScheduleKind::Fixed => &self.fixed_pos,
        };
        let scheduled = reverse
            .get(instance_index)
            .copied()
            .is_some_and(|pos| pos != NONE_SLOT);
        if !scheduled {
            return None;
        }
        self.instances.get(instance_index)
    }

    /// Read an instance by node id inside a closure.
    #[inline]
    pub(crate) fn with_instance<V, F>(&self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&ScriptInstance) -> V,
    {
        let i = self.instance_index_for(id)?;
        Some(f(self.instances.get(i)?))
    }

    /// Mutably access an instance by node id inside a closure.
    #[inline]
    pub(crate) fn with_instance_mut<V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut ScriptInstance) -> V,
    {
        let i = self.instance_index_for(id)?;
        Some(f(self.instances.get_mut(i)?))
    }

    // ---- Insert/remove ----

    /// Insert or replace a script instance for a node id.
    ///
    /// Replacement keeps the dense index and rebuilds schedules from the new
    /// behavior flags. New insertions repair any stale slot occupant first.
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

    /// Remove a script instance by node id.
    ///
    /// Removal updates node-slot lookup, schedule arrays, reverse schedule
    /// indexes, and any moved instance caused by swap-remove.
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

    // ---- Schedule snapshots ----

    /// Append update schedule entries as dense index plus validating node id.
    pub(crate) fn append_update_slots(&self, out: &mut Vec<(usize, NodeID)>) {
        for &i in &self.update {
            out.push((i, self.ids[i]));
        }
    }

    /// Return number of scripts currently scheduled for `on_update`.
    #[inline]
    pub(crate) fn update_schedule_len(&self) -> usize {
        self.update.len()
    }

    /// Append fixed-update schedule entries as dense index plus validating node id.
    pub(crate) fn append_fixed_update_slots(&self, out: &mut Vec<(usize, NodeID)>) {
        for &i in &self.fixed {
            out.push((i, self.ids[i]));
        }
    }

    /// Enable or disable `on_update` scheduling for a script.
    ///
    /// Returns `true` only when the schedule membership changed.
    pub(crate) fn set_update_enabled(&mut self, id: NodeID, enabled: bool) -> bool {
        let Some(i) = self.instance_index_for(id) else {
            return false;
        };
        if enabled && !self.instances[i].behavior.script_flags().has_update() {
            return false;
        }

        let changed = self.set_schedule_slot(i, enabled, true);
        if changed {
            self.bump_schedule_epoch();
        }
        changed
    }

    /// Enable or disable `on_fixed_update` scheduling for a script.
    ///
    /// Returns `true` only when the schedule membership changed.
    pub(crate) fn set_fixed_update_enabled(&mut self, id: NodeID, enabled: bool) -> bool {
        let Some(i) = self.instance_index_for(id) else {
            return false;
        };
        if enabled && !self.instances[i].behavior.script_flags().has_fixed_update() {
            return false;
        }

        let changed = self.set_schedule_slot(i, enabled, false);
        if changed {
            self.bump_schedule_epoch();
        }
        changed
    }

    /// Append ids for all active script instances.
    pub(crate) fn append_instance_ids(&self, out: &mut Vec<NodeID>) {
        out.extend(self.ids.iter().copied());
    }

    /// Return number of scripts currently scheduled for `on_fixed_update`.
    #[inline]
    pub(crate) fn fixed_schedule_len(&self) -> usize {
        self.fixed.len()
    }

    /// Return schedule mutation counter.
    ///
    /// Callers use this to detect when cached schedule snapshots need rebuild.
    #[inline]
    pub(crate) fn schedule_epoch(&self) -> u64 {
        self.schedule_epoch
    }

    // ---- Typed state access ----

    /// Read concrete script state by node id inside a closure.
    #[inline(always)]
    pub(crate) fn with_state<T: 'static, V, F>(&self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&T) -> V,
    {
        let i = self.instance_index_for(id)?;
        let instance = self.instances.get(i)?;
        let state = checked_state_ref::<T>(instance.state_type, instance.state.as_ref())?;
        Some(f(state))
    }

    /// Read concrete script state using scheduler snapshot keys.
    #[inline(always)]
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
        let state = checked_state_ref::<T>(instance.state_type, instance.state.as_ref())?;
        Some(f(state))
    }

    /// Mutably access concrete script state by node id inside a closure.
    #[inline(always)]
    pub(crate) fn with_state_mut<T: 'static, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        let i = self.instance_index_for(id)?;
        let instance = self.instances.get_mut(i)?;
        let state = checked_state_mut::<T>(instance.state_type, instance.state.as_mut())?;
        Some(f(state))
    }

    /// Mutably access concrete script state using scheduler snapshot keys.
    #[inline(always)]
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
        let state = checked_state_mut::<T>(instance.state_type, instance.state.as_mut())?;
        Some(f(state))
    }

    // ---- Dense index helpers ----

    /// Resolve a node id to a dense instance index and reject stale generations.
    #[inline]
    fn instance_index_for(&self, id: NodeID) -> Option<usize> {
        let slot = id.index() as usize;
        let i = self.slot_value(&self.index, slot)?;
        (self.ids.get(i).copied() == Some(id)).then_some(i)
    }

    /// Set sparse node-slot -> dense instance index mapping.
    #[inline]
    fn set_index_slot(&mut self, slot: usize, value: Option<usize>) {
        if self.index.len() <= slot {
            self.index.resize(slot + 1, NONE_SLOT);
        }
        self.index[slot] = value
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(NONE_SLOT);
    }

    // ---- Schedule maintenance ----

    /// Remove a dense instance index from both schedules.
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

    /// Add or remove one dense instance index from one schedule.
    fn set_schedule_slot(&mut self, i: usize, enabled: bool, update: bool) -> bool {
        let (slots, reverse) = if update {
            (&mut self.update, &mut self.update_pos)
        } else {
            (&mut self.fixed, &mut self.fixed_pos)
        };

        if enabled {
            if reverse.get(i).copied().is_some_and(|pos| pos != NONE_SLOT) {
                return false;
            }
            let pos = slots.len();
            slots.push(i);
            Self::set_reverse_slot(reverse, i, Some(pos));
            return true;
        }

        let Some(pos) = Self::take_reverse_slot(reverse, i) else {
            return false;
        };
        let last_pos = slots.len() - 1;
        slots.swap_remove(pos);
        if pos != last_pos {
            let moved_idx = slots[pos];
            Self::set_reverse_slot(reverse, moved_idx, Some(pos));
        }
        true
    }

    /// Rebuild both schedules for a replaced instance.
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

    /// Set dense instance index -> schedule position reverse mapping.
    #[inline]
    fn set_reverse_slot(slots: &mut Vec<u32>, index: usize, value: Option<usize>) {
        if slots.len() <= index {
            slots.resize(index + 1, NONE_SLOT);
        }
        slots[index] = value
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(NONE_SLOT);
    }

    /// Clear and return a reverse schedule slot.
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

    /// Read an optional sparse slot encoded with [`NONE_SLOT`].
    #[inline]
    fn slot_value(&self, slots: &[u32], index: usize) -> Option<usize> {
        let value = *slots.get(index)?;
        if value == NONE_SLOT {
            None
        } else {
            Some(value as usize)
        }
    }

    /// Bump schedule mutation counter with wrapping arithmetic.
    #[inline]
    fn bump_schedule_epoch(&mut self) {
        self.schedule_epoch = self.schedule_epoch.wrapping_add(1);
    }
}

#[inline(always)]
fn checked_state_ref<T: 'static>(state_type: TypeId, state: &dyn Any) -> Option<&T> {
    if state_type != TypeId::of::<T>() {
        return None;
    }
    // SAFETY: state_type comes from this ScriptInstance state at insert time and matches T here.
    Some(unsafe { state_ref_unchecked::<T>(state) })
}

#[inline(always)]
fn checked_state_mut<T: 'static>(state_type: TypeId, state: &mut dyn Any) -> Option<&mut T> {
    if state_type != TypeId::of::<T>() {
        return None;
    }
    // SAFETY: state_type comes from this ScriptInstance state at insert time and matches T here.
    Some(unsafe { state_mut_unchecked::<T>(state) })
}

impl Default for ScriptCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod unsafe_state_cast_tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct TestState {
        value: u64,
        text: &'static str,
    }

    #[derive(Debug, PartialEq)]
    struct OtherState {
        value: u64,
    }

    #[test]
    fn checked_state_ref_matches_safe_downcast_ref() {
        let state: Box<dyn Any> = Box::new(TestState {
            value: 42,
            text: "perro",
        });
        let state_type = state.as_ref().type_id();

        let safe = state.as_ref().downcast_ref::<TestState>();
        let fast = checked_state_ref::<TestState>(state_type, state.as_ref());

        assert_eq!(fast, safe);
        assert_eq!(
            fast.map(|state| state as *const TestState),
            safe.map(|state| state as *const TestState)
        );
    }

    #[test]
    fn checked_state_ref_miss_matches_safe_downcast_ref() {
        let state: Box<dyn Any> = Box::new(OtherState { value: 7 });
        let state_type = state.as_ref().type_id();

        let safe = state.as_ref().downcast_ref::<TestState>();
        let fast = checked_state_ref::<TestState>(state_type, state.as_ref());

        assert_eq!(fast, safe);
    }

    #[test]
    fn checked_state_mut_matches_safe_downcast_mut() {
        let mut safe_state: Box<dyn Any> = Box::new(TestState {
            value: 42,
            text: "perro",
        });
        let mut fast_state: Box<dyn Any> = Box::new(TestState {
            value: 42,
            text: "perro",
        });
        let safe_type = safe_state.as_ref().type_id();
        let fast_type = fast_state.as_ref().type_id();

        let safe = safe_state.as_mut().downcast_mut::<TestState>();
        let fast = checked_state_mut::<TestState>(fast_type, fast_state.as_mut());

        assert_eq!(fast.as_deref(), safe.as_deref());
        assert_eq!(safe_type, fast_type);

        safe.unwrap().value += 1;
        fast.unwrap().value += 1;

        assert_eq!(
            fast_state.as_ref().downcast_ref::<TestState>(),
            safe_state.as_ref().downcast_ref::<TestState>()
        );
    }

    #[test]
    fn checked_state_mut_miss_matches_safe_downcast_mut() {
        let mut safe_state: Box<dyn Any> = Box::new(OtherState { value: 7 });
        let mut fast_state: Box<dyn Any> = Box::new(OtherState { value: 7 });
        let state_type = fast_state.as_ref().type_id();

        let safe = safe_state.as_mut().downcast_mut::<TestState>().is_none();
        let fast = checked_state_mut::<TestState>(state_type, fast_state.as_mut()).is_none();

        assert_eq!(fast, safe);
    }
}

#[cfg(test)]
mod scheduled_instance_tests {
    use super::*;
    use perro_ids::ScriptMemberID;
    use perro_scripting::{ScriptBehavior, ScriptContext, ScriptFlags, ScriptLifecycle};
    use perro_variant::Variant;
    use std::any::Any;

    /// Minimal behavior carrying an identity marker readable via `get_var`, so a
    /// scheduled lookup can be checked to return the correct instance after
    /// swap-remove compaction.
    struct MarkerScript {
        marker: i64,
        flags: ScriptFlags,
    }

    impl ScriptLifecycle<crate::runtime::RuntimeScriptApi> for MarkerScript {}

    impl ScriptBehavior<crate::runtime::RuntimeScriptApi> for MarkerScript {
        fn script_flags(&self) -> ScriptFlags {
            self.flags
        }

        fn create_state(&self) -> Box<dyn Any> {
            Box::new(())
        }

        fn get_var(&self, _state: &dyn Any, _var: ScriptMemberID) -> Variant {
            Variant::from(self.marker)
        }

        fn set_var(&self, _state: &mut dyn Any, _var: ScriptMemberID, _value: Variant) {}

        fn call_method(
            &self,
            _method: ScriptMemberID,
            _ctx: &mut ScriptContext<'_, crate::runtime::RuntimeScriptApi>,
            _params: &[Variant],
        ) -> Variant {
            Variant::Null
        }
    }

    fn insert_marker(coll: &mut ScriptCollection, id: NodeID, marker: i64, flags: u8) {
        coll.insert(
            id,
            Arc::new(MarkerScript {
                marker,
                flags: ScriptFlags::new(flags),
            }),
            Box::new(()),
        );
    }

    fn marker_of(instance: &ScriptInstance) -> i64 {
        instance
            .behavior
            .get_var(instance.state.as_ref(), ScriptMemberID(0))
            .as_i64()
            .unwrap()
    }

    // (a) A script removed mid-frame is skipped by a stale scheduler snapshot key.
    #[test]
    fn scheduled_instance_none_after_mid_frame_removal() {
        let mut coll = ScriptCollection::new();
        let id = NodeID::new(1);
        insert_marker(&mut coll, id, 11, ScriptFlags::HAS_UPDATE);

        // Snapshot key captured while scheduled.
        let index = coll.instance_index_for_id(id).unwrap();
        assert!(
            coll.scheduled_instance(index, id, ScheduleKind::Update)
                .is_some()
        );

        // Removed mid-frame: the same (index, id) key must no longer resolve.
        coll.remove(id);
        assert!(
            coll.scheduled_instance(index, id, ScheduleKind::Update)
                .is_none()
        );
    }

    // (b) set_update_enabled(false) mid-frame drops the script from the update
    // schedule; the snapshot key resolves for fixed but not update.
    #[test]
    fn scheduled_instance_respects_mid_frame_disable() {
        let mut coll = ScriptCollection::new();
        let id = NodeID::new(1);
        insert_marker(
            &mut coll,
            id,
            22,
            ScriptFlags::HAS_UPDATE | ScriptFlags::HAS_FIXED_UPDATE,
        );
        let index = coll.instance_index_for_id(id).unwrap();

        assert!(
            coll.scheduled_instance(index, id, ScheduleKind::Update)
                .is_some()
        );
        assert!(
            coll.scheduled_instance(index, id, ScheduleKind::Fixed)
                .is_some()
        );

        // Disable only the update schedule mid-frame.
        assert!(coll.set_update_enabled(id, false));
        assert!(
            coll.scheduled_instance(index, id, ScheduleKind::Update)
                .is_none()
        );
        // Fixed schedule membership is unaffected.
        assert!(
            coll.scheduled_instance(index, id, ScheduleKind::Fixed)
                .is_some()
        );
    }

    // (c) Removing an instance swap-moves the last instance into the freed slot;
    // a scheduled lookup with the moved instance's key must resolve to the moved
    // instance (correct id + schedule position), not the stale occupant.
    #[test]
    fn scheduled_instance_revalidates_after_swap_remove() {
        let mut coll = ScriptCollection::new();
        let a = NodeID::new(1);
        let b = NodeID::new(2);
        insert_marker(&mut coll, a, 100, ScriptFlags::HAS_UPDATE);
        insert_marker(&mut coll, b, 200, ScriptFlags::HAS_UPDATE);

        let index_a = coll.instance_index_for_id(a).unwrap();
        let index_b = coll.instance_index_for_id(b).unwrap();
        assert_eq!(index_a, 0);
        assert_eq!(index_b, 1);

        // Remove `a`: swap-remove moves `b` from dense index 1 into index 0.
        coll.remove(a);

        // The old key for `b` (index 1) is now stale and must not resolve.
        assert!(
            coll.scheduled_instance(index_b, b, ScheduleKind::Update)
                .is_none()
        );

        // `b` is now at index 0; the revalidated lookup returns the moved
        // instance with its own marker, and its update schedule membership moved
        // with it.
        let moved_index = coll.instance_index_for_id(b).unwrap();
        assert_eq!(moved_index, 0);
        let moved = coll
            .scheduled_instance(moved_index, b, ScheduleKind::Update)
            .expect("moved instance still scheduled");
        assert_eq!(marker_of(moved), 200);

        // A lookup with the moved index but the removed id must reject.
        assert!(
            coll.scheduled_instance(moved_index, a, ScheduleKind::Update)
                .is_none()
        );
    }
}
