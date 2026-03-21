use super::core::RuntimeResourceApi;
use perro_render_bridge::{AccessibilityCommand, RenderCommand};
use perro_resource_context::sub_apis::AccessibilityAPI;
use perro_structs::ColorBlindFilter;

impl AccessibilityAPI for RuntimeResourceApi {
    fn enable_color_blind_filter(&self, mode: ColorBlindFilter, strength: f32) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::Accessibility(
            AccessibilityCommand::EnableColorBlind { mode, strength },
        ));
    }

    fn disable_color_blind_filter(&self) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::Accessibility(
            AccessibilityCommand::DisableColorBlind,
        ));
    }
}
