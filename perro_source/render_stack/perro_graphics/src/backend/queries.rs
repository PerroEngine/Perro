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
        // (0,0) = no resize landed yet; the attach-time configure already used
        // the real window size, so a 1x1 rebuild here is pure waste.
        if self.viewport.0 > 0 && self.viewport.1 > 0 {
            gpu.resize(self.viewport.0, self.viewport.1);
        }
        self.events
            .push(RenderEvent::HdrStatusChanged(gpu.hdr_status()));
        self.gpu = Some(gpu);
        self.pending_gpu = None;
        self.redraw_requested = true;
    }
}
