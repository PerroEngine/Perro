use perro_animation::AnimationClip;
use perro_ids::AnimationID;
use std::sync::Arc;

pub trait AnimationAPI {
    fn load_animation_source(&self, source: &str) -> AnimationID;
    fn reserve_animation_source(&self, source: &str) -> AnimationID;
    fn drop_animation_source(&self, source: &str) -> bool;
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
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> AnimationID {
        self.api.reserve_animation_source(source.as_ref())
    }

    #[inline]
    pub fn drop<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.drop_animation_source(source.as_ref())
    }

    #[inline]
    pub fn get(&self, id: AnimationID) -> Option<Arc<AnimationClip>> {
        self.api.get_animation(id)
    }
}

#[macro_export]
macro_rules! animation_load {
    ($res:expr, $source:expr) => {
        $res.Animations().load($source)
    };
}

#[macro_export]
macro_rules! animation_reserve {
    ($res:expr, $source:expr) => {
        $res.Animations().reserve($source)
    };
}

#[macro_export]
macro_rules! animation_drop {
    ($res:expr, $source:expr) => {
        $res.Animations().drop($source)
    };
}
