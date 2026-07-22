use super::RuntimeResourceApi;
use perro_render_bridge::{DisplayCommand, HdrMode, HdrStatus, RenderCommand};
use perro_resource_api::api::ViewportAPI;
use perro_structs::Vector2;

impl ViewportAPI for RuntimeResourceApi {
    #[inline]
    fn viewport_size(&self) -> Vector2 {
        let (width, height) = RuntimeResourceApi::viewport_size(self);
        Vector2::new(width as f32, height as f32)
    }

    fn set_hdr_mode(&self, mode: HdrMode) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.hdr_status.requested = mode;
        state
            .queued_commands
            .push(RenderCommand::Display(DisplayCommand::SetHdrMode(mode)));
    }

    fn hdr_status(&self) -> HdrStatus {
        self.state
            .lock()
            .expect("resource api mutex poisoned")
            .hdr_status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_render_bridge::{HdrColorSpace, HdrFallback};
    use perro_resource_api::ResourceWindow;

    #[test]
    fn hdr_macros_queue_mode_and_read_backend_status() {
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);
        let res = ResourceWindow::new(api.as_ref());
        perro_resource_api::hdr_set!(res, HdrMode::On);

        let mut commands = Vec::new();
        api.drain_commands(&mut commands);
        assert!(matches!(
            commands.as_slice(),
            [RenderCommand::Display(DisplayCommand::SetHdrMode(
                HdrMode::On
            ))]
        ));

        let status = HdrStatus {
            requested: HdrMode::On,
            supported: true,
            active: true,
            scene_hdr: true,
            color_space: HdrColorSpace::ExtendedSrgbLinear,
            headroom: 4.0,
            peak_nits: Some(800.0),
            fallback: None,
        };
        api.apply_render_event(&perro_render_bridge::RenderEvent::HdrStatusChanged(status));
        assert!(perro_resource_api::hdr_active!(res));
        assert!(perro_resource_api::hdr_supported!(res));
        assert_eq!(perro_resource_api::hdr_status!(res), status);
        assert_ne!(status.fallback, Some(HdrFallback::Disabled));
    }
}
