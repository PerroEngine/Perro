use super::core::RuntimeResourceApi;
use perro_animation::AnimationTreeAsset;
use perro_ids::AnimationTreeID;
use perro_resource_api::sub_apis::AnimationTreeAPI;
use std::borrow::Cow;
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
            return id;
        }

        let tree = self
            .load_animation_tree_source_data(source_hash, source.map(str::trim))
            .unwrap_or_else(|| Arc::new(AnimationTreeAsset::default()));
        let id = state.allocate_animation_tree_id();
        state.animation_tree_by_source.insert(source_hash, id);
        state.animation_tree_data_by_id.insert(id, tree);
        id
    }

    fn get_animation_tree(&self, id: AnimationTreeID) -> Option<Arc<AnimationTreeAsset>> {
        if id.is_nil() {
            return None;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_tree_data_by_id.get(&id).cloned()
    }
}

impl RuntimeResourceApi {
    fn load_animation_tree_source_data(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> Option<Arc<AnimationTreeAsset>> {
        if let Some(lookup) = self.static_animation_tree_lookup {
            return Some(Arc::new(lookup(source_hash).clone()));
        }

        let source = source?;
        if source.ends_with(".panimtree")
            && let Ok(bytes) = perro_io::load_asset(source)
            && let Ok(text) = std::str::from_utf8(&bytes)
            && let Ok(tree) = perro_animation::parse_panimtree(text)
        {
            return Some(Arc::new(tree));
        }

        Some(Arc::new(AnimationTreeAsset {
            name: Cow::Borrowed("AnimationTree"),
            slots: Cow::Borrowed(&[]),
            nodes: Cow::Borrowed(&[]),
            output: Cow::Borrowed(""),
        }))
    }
}
