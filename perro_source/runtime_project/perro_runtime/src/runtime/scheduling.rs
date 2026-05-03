use super::{Runtime, UpdateScheduleTiming};
use std::time::Instant;

impl Runtime {
    pub(crate) fn run_update_schedule(&mut self) {
        let resource_api = self.resource_api.clone();
        let res = perro_resource_context::ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { perro_input::InputWindow::new(&*input_ptr) };
        let mut i = 0;
        while i < self.schedules.update_slots.len() {
            let (instance_index, id) = self.schedules.update_slots[i];
            self.call_update_script_scheduled_with_context(instance_index, id, &res, &ipt);
            i += 1;
        }
    }

    pub(crate) fn run_fixed_schedule(&mut self) {
        let resource_api = self.resource_api.clone();
        let res = perro_resource_context::ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { perro_input::InputWindow::new(&*input_ptr) };
        let mut i = 0;
        while i < self.schedules.fixed_slots.len() {
            let (instance_index, id) = self.schedules.fixed_slots[i];
            self.call_fixed_update_script_scheduled_with_context(instance_index, id, &res, &ipt);
            i += 1;
        }
    }

    pub(crate) fn run_start_schedule(&mut self) {
        let mut queued = std::mem::take(&mut self.script_runtime.pending_start_scripts);
        let mut ran_start = false;
        for id in queued.drain(..) {
            let slot = id.index() as usize;
            let still_pending = self
                .script_runtime
                .pending_start_flags
                .get(slot)
                .copied()
                .flatten()
                == Some(id);
            if !still_pending {
                continue;
            }
            self.script_runtime.pending_start_flags[slot] = None;
            self.call_start_script(id);
            ran_start = true;
        }
        self.script_runtime.pending_start_scripts = queued;
        if ran_start {
            self.mark_ui_viewport_dirty();
        }
    }

    pub(crate) fn run_update_schedule_timed(&mut self) -> UpdateScheduleTiming {
        let schedule_start = Instant::now();
        let mut scripts_total = std::time::Duration::ZERO;
        let mut script_count = 0u32;
        let mut slowest_script = std::time::Duration::ZERO;
        let mut slowest_script_id = None;
        let resource_api = self.resource_api.clone();
        let res = perro_resource_context::ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt = unsafe { perro_input::InputWindow::new(&*input_ptr) };

        let mut i = 0;
        while i < self.schedules.update_slots.len() {
            let (instance_index, id) = self.schedules.update_slots[i];
            let script_start = Instant::now();
            self.call_update_script_scheduled_with_context(instance_index, id, &res, &ipt);
            let script_duration = script_start.elapsed();
            scripts_total += script_duration;
            script_count = script_count.saturating_add(1);
            if script_duration > slowest_script {
                slowest_script = script_duration;
                slowest_script_id = Some(id);
            }
            i += 1;
        }

        UpdateScheduleTiming {
            total: schedule_start.elapsed(),
            scripts_total,
            script_count,
            slowest_script_id,
            slowest_script,
        }
    }
}

