#[cfg(target_arch = "wasm32")]
use super::*;

#[cfg(target_arch = "wasm32")]
impl PerroGraphics {
    pub(super) fn try_finish_gpu_init(&mut self) {
        let Some(slot) = self.pending_gpu.as_ref() else {
            return;
        };
        let Some(mut gpu) = slot.lock().ok().and_then(|mut guard| guard.take()) else {
            return;
        };
        gpu.set_virtual_size_2d(self.renderer_2d.virtual_viewport());
        gpu.resize(self.viewport.0.max(1), self.viewport.1.max(1));
        self.events
            .push(RenderEvent::HdrStatusChanged(gpu.hdr_status()));
        self.gpu = Some(gpu);
        self.pending_gpu = None;
        self.redraw_requested = true;
    }
}
