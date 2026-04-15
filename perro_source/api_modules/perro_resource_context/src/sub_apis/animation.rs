use perro_animation::AnimationClip;
use perro_ids::AnimationID;
use std::sync::Arc;

pub trait AnimationAPI {
    fn load_animation_source_hashed(&self, source_hash: u64, source: Option<&str>) -> AnimationID;
    fn reserve_animation_source_hashed(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> AnimationID;
    fn load_animation_source(&self, source: &str) -> AnimationID {
        self.load_animation_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_animation_source(&self, source: &str) -> AnimationID {
        self.reserve_animation_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_animation_source(&self, id: AnimationID) -> bool;
    fn get_animation(&self, id: AnimationID) -> Option<Arc<AnimationClip>>;
}

pub struct AnimationModule<'res, R: AnimationAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AnimationAPI + ?Sized> AnimationModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: AsRef<str>>(&self, source: S) -> AnimationID {
        self.api.load_animation_source(source.as_ref())
    }

    #[inline]
    pub fn load_hashed(&self, source_hash: u64) -> AnimationID {
        self.api.load_animation_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source(&self, source_hash: u64, source: &str) -> AnimationID {
        self.api
            .load_animation_source_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> AnimationID {
        self.api.reserve_animation_source(source.as_ref())
    }

    #[inline]
    pub fn reserve_hashed(&self, source_hash: u64) -> AnimationID {
        self.api.reserve_animation_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn reserve_hashed_with_source(&self, source_hash: u64, source: &str) -> AnimationID {
        self.api
            .reserve_animation_source_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn drop(&self, id: AnimationID) -> bool {
        self.api.drop_animation_source(id)
    }

    #[inline]
    pub fn get(&self, id: AnimationID) -> Option<Arc<AnimationClip>> {
        self.api.get_animation(id)
    }
}

#[macro_export]
macro_rules! animation_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Animations().load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Animations().load($source)
    };
}

#[macro_export]
macro_rules! animation_reserve {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Animations().reserve_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Animations().reserve($source)
    };
}

#[macro_export]
macro_rules! animation_drop {
    ($res:expr, $id:expr) => {
        $res.Animations().drop($id)
    };
}
