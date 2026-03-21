use super::core::RuntimeResourceApi;
use perro_render_bridge::{PostProcessingCommand, RenderCommand};
use perro_resource_context::sub_apis::PostProcessingAPI;
use perro_structs::{PostProcessEffect, PostProcessSet};
use std::borrow::Cow;

impl PostProcessingAPI for RuntimeResourceApi {
    fn set_global_post_processing(&self, set: PostProcessSet) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::PostProcessing(
            PostProcessingCommand::SetGlobal(set),
        ));
    }

    fn add_global_post_processing_named(&self, name: Cow<'static, str>, effect: PostProcessEffect) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::PostProcessing(
            PostProcessingCommand::AddGlobalNamed { name, effect },
        ));
    }

    fn add_global_post_processing(&self, effect: PostProcessEffect) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::PostProcessing(
            PostProcessingCommand::AddGlobalUnnamed(effect),
        ));
    }

    fn remove_global_post_processing_by_name(&self, name: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::PostProcessing(
            PostProcessingCommand::RemoveGlobalByName(Cow::Owned(name.to_string())),
        ));
        true
    }

    fn remove_global_post_processing_by_index(&self, index: usize) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::PostProcessing(
            PostProcessingCommand::RemoveGlobalByIndex(index),
        ));
        true
    }

    fn clear_global_post_processing(&self) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.queued_commands.push(RenderCommand::PostProcessing(
            PostProcessingCommand::ClearGlobal,
        ));
    }
}
