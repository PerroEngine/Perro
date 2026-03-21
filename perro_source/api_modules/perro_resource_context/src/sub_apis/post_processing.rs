use perro_structs::{PostProcessEffect, PostProcessSet};
use std::borrow::Cow;

pub trait PostProcessingAPI {
    fn set_global_post_processing(&self, set: PostProcessSet);
    fn add_global_post_processing_named(&self, name: Cow<'static, str>, effect: PostProcessEffect);
    fn add_global_post_processing(&self, effect: PostProcessEffect);
    fn remove_global_post_processing_by_name(&self, name: &str) -> bool;
    fn remove_global_post_processing_by_index(&self, index: usize) -> bool;
    fn clear_global_post_processing(&self);
}

#[macro_export]
macro_rules! post_processing_set {
    ($res:expr, $set:expr) => {
        $res.set_global_post_processing($set)
    };
}

#[macro_export]
macro_rules! post_processing_add {
    ($res:expr, $effect:expr) => {
        $res.add_global_post_processing($effect)
    };
    ($res:expr, $name:expr, $effect:expr) => {
        $res.add_global_post_processing_named($name, $effect)
    };
}

#[macro_export]
macro_rules! post_processing_remove {
    ($res:expr, name = $name:expr) => {
        $res.remove_global_post_processing_by_name($name)
    };
    ($res:expr, index = $index:expr) => {
        $res.remove_global_post_processing_by_index($index)
    };
}

#[macro_export]
macro_rules! post_processing_clear {
    ($res:expr) => {
        $res.clear_global_post_processing()
    };
}
