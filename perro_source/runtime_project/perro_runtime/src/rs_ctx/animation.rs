use super::core::RuntimeResourceApi;
use perro_animation::AnimationClip;
use perro_ids::AnimationID;
use perro_resource_context::sub_apis::AnimationAPI;
use std::borrow::Cow;
use std::sync::Arc;

impl AnimationAPI for RuntimeResourceApi {
    fn load_animation_source(&self, source: &str) -> AnimationID {
        let source = source.trim();
        if source.is_empty() {
            return AnimationID::nil();
        }

        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.animation_by_source.get(source).copied() {
            return id;
        }

        let clip = self
            .load_animation_source_data(source)
            .unwrap_or_else(|| Arc::new(AnimationClip::default()));
        let id = state.allocate_animation_id();
        state.animation_by_source.insert(source.to_string(), id);
        state.animation_data_by_id.insert(id, clip);
        id
    }

    fn reserve_animation_source(&self, source: &str) -> AnimationID {
        self.load_animation_source(source)
    }

    fn drop_animation_source(&self, source: &str) -> bool {
        let source = source.trim();
        if source.is_empty() {
            return false;
        }

        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let Some(id) = state.animation_by_source.remove(source) else {
            return false;
        };
        state.animation_data_by_id.remove(&id);
        let _ = state.free_animation_id(id);
        true
    }

    fn get_animation(&self, id: AnimationID) -> Option<Arc<AnimationClip>> {
        if id.is_nil() {
            return None;
        }

        let state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_data_by_id.get(&id).cloned()
    }
}

impl RuntimeResourceApi {
    fn load_animation_source_data(&self, source: &str) -> Option<Arc<AnimationClip>> {
        if let Some(lookup) = self.static_animation_lookup {
            return lookup(source).map(|clip| Arc::new(clip.clone()));
        }

        // First pass: dynamic disk parsing is not implemented yet.
        // The runtime still caches a default clip so IDs remain stable.
        let _ = source;
        Some(Arc::new(AnimationClip {
            name: Cow::Borrowed("Animation"),
            fps: 60.0,
            frame_count: 1,
            duration: 0.0,
            tracks: Cow::Borrowed(&[]),
        }))
    }
}
