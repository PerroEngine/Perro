use ahash::AHashMap;
use perro_api::api::RuntimeAPI;
use perro_ids::NodeID;
use perro_scripting::{ScriptBehavior, ScriptState};
use std::sync::Arc;

type IdMap = AHashMap<NodeID, usize>;

pub struct ScriptInstance<R: RuntimeAPI + ?Sized> {
    pub behavior: Arc<dyn ScriptBehavior<R>>,
    pub state: Option<Box<dyn ScriptState>>,
}

pub struct ScriptCollection<R: RuntimeAPI + ?Sized> {
    instances: Vec<ScriptInstance<R>>,
    ids: Vec<NodeID>,
    index: IdMap,

    update: Vec<usize>,
    fixed: Vec<usize>,

    // Reverse indices for O(1) schedule updates
    update_pos: AHashMap<usize, usize>,
    fixed_pos: AHashMap<usize, usize>,
}

impl<R: RuntimeAPI + ?Sized> ScriptCollection<R> {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            ids: Vec::new(),
            index: AHashMap::default(),
            update: Vec::new(),
            fixed: Vec::new(),
            update_pos: AHashMap::default(),
            fixed_pos: AHashMap::default(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            instances: Vec::with_capacity(capacity),
            ids: Vec::with_capacity(capacity),
            index: AHashMap::with_capacity(capacity),
            update: Vec::with_capacity(capacity),
            fixed: Vec::with_capacity(capacity),
            update_pos: AHashMap::with_capacity(capacity),
            fixed_pos: AHashMap::with_capacity(capacity),
        }
    }

    pub fn get_instance(&self, id: NodeID) -> Option<&ScriptInstance<R>> {
        let &i = self.index.get(&id)?;
        self.instances.get(i)
    }

    pub fn get_instance_mut(&mut self, id: NodeID) -> Option<&mut ScriptInstance<R>> {
        let &i = self.index.get(&id)?;
        self.instances.get_mut(i)
    }

    pub fn insert(
        &mut self,
        id: NodeID,
        behavior: Arc<dyn ScriptBehavior<R>>,
        mut state: Box<dyn ScriptState>,
    ) {
        state.set_id(id);
        let flags = behavior.script_flags();

        if let Some(&i) = self.index.get(&id) {
            // replace in-place
            self.instances[i] = ScriptInstance {
                behavior,
                state: Some(state),
            };
            self.rebuild_schedules_for_index(i, flags);
            return;
        }

        let i = self.instances.len();
        self.instances.push(ScriptInstance {
            behavior,
            state: Some(state),
        });
        self.ids.push(id);
        self.index.insert(id, i);

        if flags.has_update() {
            let pos = self.update.len();
            self.update.push(i);
            self.update_pos.insert(i, pos);
        }
        if flags.has_fixed_update() {
            let pos = self.fixed.len();
            self.fixed.push(i);
            self.fixed_pos.insert(i, pos);
        }
    }

    pub fn remove(&mut self, id: NodeID) -> Option<ScriptInstance<R>> {
        let i = self.index.remove(&id)?;
        self.remove_from_schedules_by_index(i);

        let last = self.instances.len() - 1;
        self.instances.swap(i, last);
        self.ids.swap(i, last);

        let removed = self.instances.pop().unwrap();
        let removed_id = self.ids.pop().unwrap();
        debug_assert!(removed_id == id);

        if i != last {
            // moved entry now at i
            let moved_id = self.ids[i];
            self.index.insert(moved_id, i);

            // O(1) schedule updates
            if let Some(pos) = self.update_pos.remove(&last) {
                self.update[pos] = i;
                self.update_pos.insert(i, pos);
            }
            if let Some(pos) = self.fixed_pos.remove(&last) {
                self.fixed[pos] = i;
                self.fixed_pos.insert(i, pos);
            }
        }

        Some(removed)
    }

    pub fn get_update_ids(&self) -> Vec<NodeID> {
        self.update.iter().map(|&i| self.ids[i]).collect()
    }

    pub fn get_fixed_update_ids(&self) -> Vec<NodeID> {
        self.fixed.iter().map(|&i| self.ids[i]).collect()
    }

    pub fn take_state(
        &mut self,
        id: NodeID,
    ) -> Option<(Arc<dyn ScriptBehavior<R>>, Box<dyn ScriptState>)> {
        let &i = self.index.get(&id)?;
        let instance = self.instances.get_mut(i)?;
        let state = instance.state.take()?;
        let behavior = Arc::clone(&instance.behavior);
        Some((behavior, state))
    }

    pub fn put_state(&mut self, id: NodeID, state: Box<dyn ScriptState>) -> bool {
        let Some(&i) = self.index.get(&id) else {
            return false;
        };
        let Some(instance) = self.instances.get_mut(i) else {
            return false;
        };
        if instance.state.is_some() {
            return false;
        }
        instance.state = Some(state);
        true
    }

    fn remove_from_schedules_by_index(&mut self, i: usize) {
        // Remove from update schedule
        if let Some(pos) = self.update_pos.remove(&i) {
            let last_pos = self.update.len() - 1;
            self.update.swap_remove(pos);

            if pos != last_pos {
                let moved_idx = self.update[pos];
                self.update_pos.insert(moved_idx, pos);
            }
        }

        // Remove from fixed schedule
        if let Some(pos) = self.fixed_pos.remove(&i) {
            let last_pos = self.fixed.len() - 1;
            self.fixed.swap_remove(pos);

            if pos != last_pos {
                let moved_idx = self.fixed[pos];
                self.fixed_pos.insert(moved_idx, pos);
            }
        }
    }

    fn rebuild_schedules_for_index(&mut self, i: usize, flags: perro_scripting::ScriptFlags) {
        self.remove_from_schedules_by_index(i);

        if flags.has_update() {
            let pos = self.update.len();
            self.update.push(i);
            self.update_pos.insert(i, pos);
        }
        if flags.has_fixed_update() {
            let pos = self.fixed.len();
            self.fixed.push(i);
            self.fixed_pos.insert(i, pos);
        }
    }
}

impl<R: RuntimeAPI + ?Sized> Default for ScriptCollection<R> {
    fn default() -> Self {
        Self::new()
    }
}
