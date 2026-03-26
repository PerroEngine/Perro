use crate::sub_apis::{
    AnimationAPI, AnimationModule, AudioAPI, AudioModule, MaterialAPI, MaterialModule, MeshAPI,
    MeshModule, PostProcessingAPI, SkeletonAPI, SkeletonModule, TerrainAPI, TerrainModule,
    TextureAPI, TextureModule, VisualAccessibilityAPI,
};
use perro_structs::{ColorBlindFilter, PostProcessEffect, PostProcessSet, Vector2};

pub trait ResourceAPI:
    PostProcessingAPI
    + VisualAccessibilityAPI
    + AudioAPI
    + TextureAPI
    + MeshAPI
    + MaterialAPI
    + SkeletonAPI
    + TerrainAPI
    + AnimationAPI
    + ViewportAPI
    + Send
    + Sync
{
}
impl<T> ResourceAPI for T where
    T: PostProcessingAPI
        + VisualAccessibilityAPI
        + AudioAPI
        + TextureAPI
        + MeshAPI
        + MaterialAPI
        + SkeletonAPI
        + TerrainAPI
        + AnimationAPI
        + ViewportAPI
        + Send
        + Sync
{
}

pub trait ViewportAPI {
    fn viewport_size(&self) -> Vector2;
}

pub struct ResourceContext<'res, R: ResourceAPI + ?Sized> {
    api: &'res R,
}

#[allow(non_snake_case)]
impl<'res, R: ResourceAPI + ?Sized> ResourceContext<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn Textures(&self) -> TextureModule<'_, R> {
        TextureModule::new(self.api)
    }

    #[inline]
    pub fn Audio(&self) -> AudioModule<'_, R> {
        AudioModule::new(self.api)
    }

    #[inline]
    pub fn Meshes(&self) -> MeshModule<'_, R> {
        MeshModule::new(self.api)
    }

    #[inline]
    pub fn Materials(&self) -> MaterialModule<'_, R> {
        MaterialModule::new(self.api)
    }

    #[inline]
    pub fn Skeletons(&self) -> SkeletonModule<'_, R> {
        SkeletonModule::new(self.api)
    }

    #[inline]
    pub fn Terrain(&self) -> TerrainModule<'_, R> {
        TerrainModule::new(self.api)
    }

    #[inline]
    pub fn Animations(&self) -> AnimationModule<'_, R> {
        AnimationModule::new(self.api)
    }

    #[inline]
    pub fn enable_colorblind_filter(&self, mode: ColorBlindFilter, strength: f32) {
        self.api.enable_color_blind_filter(mode, strength);
    }

    #[inline]
    pub fn disable_colorblind_filter(&self) {
        self.api.disable_color_blind_filter();
    }

    #[inline]
    pub fn set_global_post_processing(&self, set: PostProcessSet) {
        self.api.set_global_post_processing(set);
    }

    #[inline]
    pub fn add_global_post_processing_named(
        &self,
        name: impl Into<std::borrow::Cow<'static, str>>,
        effect: PostProcessEffect,
    ) {
        self.api
            .add_global_post_processing_named(name.into(), effect);
    }

    #[inline]
    pub fn add_global_post_processing(&self, effect: PostProcessEffect) {
        self.api.add_global_post_processing(effect);
    }

    #[inline]
    pub fn remove_global_post_processing_by_name(&self, name: &str) -> bool {
        self.api.remove_global_post_processing_by_name(name)
    }

    #[inline]
    pub fn remove_global_post_processing_by_index(&self, index: usize) -> bool {
        self.api.remove_global_post_processing_by_index(index)
    }

    #[inline]
    pub fn clear_global_post_processing(&self) {
        self.api.clear_global_post_processing();
    }

    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.api.viewport_size()
    }
}

#[macro_export]
macro_rules! get_viewport_size {
    ($res:expr) => {
        $res.viewport_size()
    };
}
