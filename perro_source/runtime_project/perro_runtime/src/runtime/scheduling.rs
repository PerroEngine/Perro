use super::Runtime;

impl Runtime {
    pub(crate) fn run_update_schedule(&mut self) {
        let mut i = 0;
        while i < self.schedules.update_slots.len() {
            let (instance_index, id) = self.schedules.update_slots[i];
            self.call_update_script_scheduled(instance_index, id);
            i += 1;
        }
    }

    pub(crate) fn run_fixed_schedule(&mut self) {
        let mut i = 0;
        while i < self.schedules.fixed_slots.len() {
            let (instance_index, id) = self.schedules.fixed_slots[i];
            self.call_fixed_update_script_scheduled(instance_index, id);
            i += 1;
        }
    }

    pub(crate) fn run_start_schedule(&mut self) {
        let mut queued = std::mem::take(&mut self.script_runtime.pending_start_scripts);
        for id in queued.drain(..) {
            let slot = id.index() as usize;
            let still_pending = self.script_runtime.pending_start_flags.get(slot).copied().flatten() == Some(id);
            if !still_pending {
                continue;
            }
            self.script_runtime.pending_start_flags[slot] = None;
            self.call_start_script(id);
        }
        self.script_runtime.pending_start_scripts = queued;
    }
}

