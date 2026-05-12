use crate::ResPathSource;
use perro_animation::AnimationTreeAsset;
use perro_ids::AnimationTreeID;
use std::sync::Arc;

pub trait AnimationTreeAPI {
    fn load_animation_tree_source_hashed(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> AnimationTreeID;
    fn load_animation_tree_source(&self, source: &str) -> AnimationTreeID {
        self.load_animation_tree_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn get_animation_tree(&self, id: AnimationTreeID) -> Option<Arc<AnimationTreeAsset>>;
}

pub struct AnimationTreeModule<'res, R: AnimationTreeAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AnimationTreeAPI + ?Sized> AnimationTreeModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    pub fn load<S: ResPathSource>(&self, source: S) -> AnimationTreeID {
        self.api
            .load_animation_tree_source(source.as_res_path_str())
    }

    pub fn load_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> AnimationTreeID {
        self.api
            .load_animation_tree_source_hashed(source_hash, Some(source.as_res_path_str()))
    }

    pub fn get(&self, id: AnimationTreeID) -> Option<Arc<AnimationTreeAsset>> {
        self.api.get_animation_tree(id)
    }
}

#[macro_export]
macro_rules! animation_tree_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.AnimationTrees()
            .load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.AnimationTrees().load($source)
    };
}
