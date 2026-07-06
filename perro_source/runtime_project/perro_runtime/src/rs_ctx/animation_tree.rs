use super::core::RuntimeResourceApi;
use perro_animation::AnimationTreeAsset;
use perro_ids::AnimationTreeID;
use perro_resource_api::sub_apis::AnimationTreeAPI;
use std::sync::Arc;

impl AnimationTreeAPI for RuntimeResourceApi {
    fn load_animation_tree_source(&self, source: &str) -> AnimationTreeID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.load_animation_tree_source_hashed(hash, None)
        } else {
            self.load_animation_tree_source_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn load_animation_tree_source_hashed(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> AnimationTreeID {
        if source.is_some_and(|v| v.trim().is_empty()) {
            return AnimationTreeID::nil();
        }

        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.animation_tree_by_source.get(&source_hash).copied() {
            if state.has_animation_tree_id(id) {
                return id;
            }
            state.animation_tree_by_source.remove(&source_hash);
            state.animation_tree_data_by_id.remove(&id);
            state.animation_tree_loaded_by_id.remove(&id);
        }

        let tree = self
            .static_animation_tree_lookup
            .map(|lookup| Arc::new(lookup(source_hash).clone()))
            .unwrap_or_else(|| Arc::new(AnimationTreeAsset::default()));
        let id = state.allocate_animation_tree_id();
        state.animation_tree_by_source.insert(source_hash, id);
        state.animation_tree_data_by_id.insert(id, tree);
        if self.static_animation_tree_lookup.is_some() {
            state.animation_tree_loaded_by_id.insert(id);
        }
        if self.static_animation_tree_lookup.is_none()
            && let Some(source) = source.map(str::trim).filter(|v| !v.is_empty())
        {
            let source = source.to_string();
            drop(state);
            self.queue_animation_tree_source_load(id, source);
        }
        id
    }

    fn get_animation_tree(&self, id: AnimationTreeID) -> Option<Arc<AnimationTreeAsset>> {
        if id.is_nil() {
            return None;
        }
        self.poll_async_animation_tree_loads();
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_tree_data_by_id.get(&id).cloned()
    }

    fn create_animation_tree_from_bytes(&self, bytes: &[u8]) -> AnimationTreeID {
        let Ok(text) = std::str::from_utf8(bytes) else {
            return AnimationTreeID::nil();
        };
        let Ok(tree) = perro_animation::parse_panimtree(text) else {
            return AnimationTreeID::nil();
        };
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let id = state.allocate_animation_tree_id();
        state.animation_tree_data_by_id.insert(id, Arc::new(tree));
        state.animation_tree_loaded_by_id.insert(id);
        id
    }

    fn drop_animation_tree_source(&self, id: AnimationTreeID) -> bool {
        if id.is_nil() {
            return false;
        }

        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state
            .animation_tree_by_source
            .retain(|_, existing| *existing != id);
        state.animation_tree_data_by_id.remove(&id);
        state.animation_tree_loaded_by_id.remove(&id);
        state.free_animation_tree_id(id)
    }

    fn is_animation_tree_loaded(&self, id: AnimationTreeID) -> bool {
        if id.is_nil() {
            return false;
        }
        self.poll_async_animation_tree_loads();
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_tree_loaded_by_id.contains(&id)
    }
}

impl RuntimeResourceApi {
    #[allow(dead_code)]
    pub(crate) fn is_animation_tree_id_pending(&self, tree: AnimationTreeID) -> bool {
        if tree.is_nil() {
            return false;
        }
        self.poll_async_animation_tree_loads();
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_tree_data_by_id.contains_key(&tree)
            && !state.animation_tree_loaded_by_id.contains(&tree)
    }
}
