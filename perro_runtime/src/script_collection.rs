use ahash::AHashMap;
use perro_api::{API, api::RuntimeAPI};
use perro_ids::NodeID;
use perro_scripting::ScriptObject;

type IdMap = AHashMap<NodeID, usize>;

pub struct ScriptCollection<R: RuntimeAPI + ?Sized> {
    scripts: Vec<Box<dyn ScriptObject<R>>>,
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
            scripts: Vec::new(),
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
            scripts: Vec::with_capacity(capacity),
            ids: Vec::with_capacity(capacity),
            index: AHashMap::with_capacity(capacity),
            update: Vec::with_capacity(capacity),
            fixed: Vec::with_capacity(capacity),
            update_pos: AHashMap::with_capacity(capacity),
            fixed_pos: AHashMap::with_capacity(capacity),
        }
    }

    pub fn get_script_mut(&mut self, id: NodeID) -> Option<&mut dyn ScriptObject<R>> {
        let &i = self.index.get(&id)?;
        Some(self.scripts[i].as_mut())
    }

    pub fn insert(&mut self, id: NodeID, mut obj: Box<dyn ScriptObject<R>>) {
        obj.set_id(id);
        let flags = obj.script_flags();

        if let Some(&i) = self.index.get(&id) {
            // replace in-place
            self.scripts[i] = obj;
            self.rebuild_schedules_for_id(id, i, flags);
            if flags.has_init() {
                // Need to pass api here - you'll need to change insert signature
                // Or defer init until next update_all
            }
            return;
        }

        let i = self.scripts.len();
        self.scripts.push(obj);
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
        // Defer init - can't call without API
    }

    pub fn remove(&mut self, id: NodeID) -> Option<Box<dyn ScriptObject<R>>> {
        let i = self.index.remove(&id)?;
        self.remove_from_schedules_by_index(i);

        let last = self.scripts.len() - 1;
        self.scripts.swap(i, last);
        self.ids.swap(i, last);

        let removed = self.scripts.pop().unwrap();
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

    fn rebuild_schedules_for_id(
        &mut self,
        id: NodeID,
        i: usize,
        flags: perro_scripting::ScriptFlags,
    ) {
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

        self.scripts[i].set_id(id);
    }
}

impl<R: RuntimeAPI + ?Sized> Default for ScriptCollection<R> {
    fn default() -> Self {
        Self::new()
    }
}
