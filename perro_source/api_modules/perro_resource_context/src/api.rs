use crate::sub_apis::{
    AnimationAPI, AnimationModule, AudioAPI, AudioModule, Draw2DAPI, Draw2DModule, Locale,
    LocalizationAPI, LocalizationModule, MaterialAPI, MaterialModule, MeshAPI, MeshModule,
    PostProcessingAPI, SceneDocAPI, SceneDocModule, SkeletonAPI, SkeletonModule, TextureAPI,
    TextureModule, VisualAccessibilityAPI,
};
use perro_scene::{SceneDoc, SceneWrite};
use perro_structs::{ColorBlindFilter, PostProcessEffect, PostProcessSet, Vector2};

pub trait ResourceAPI:
    PostProcessingAPI
    + VisualAccessibilityAPI
    + AudioAPI
    + TextureAPI
    + MeshAPI
    + MaterialAPI
    + SkeletonAPI
    + AnimationAPI
    + Draw2DAPI
    + LocalizationAPI
    + SceneDocAPI
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
        + AnimationAPI
        + Draw2DAPI
        + LocalizationAPI
        + SceneDocAPI
        + ViewportAPI
        + Send
        + Sync
{
}

pub trait ViewportAPI {
    fn viewport_size(&self) -> Vector2;
}

/// ResourceWindow is a wrapper around the ResourceAPI that provides access to various resource-related sub-APIs. It is designed to be passed to scripts as part of the ScriptContext, allowing them to interact with resources in a structured way.
pub struct ResourceWindow<'res, R: ResourceAPI + ?Sized> {
    api: &'res R,
}

#[allow(non_snake_case)]
impl<'res, R: ResourceAPI + ?Sized> ResourceWindow<'res, R> {
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
    pub fn Animations(&self) -> AnimationModule<'_, R> {
        AnimationModule::new(self.api)
    }

    #[inline]
    pub fn Draw2D(&self) -> Draw2DModule<'_, R> {
        Draw2DModule::new(self.api)
    }

    #[inline]
    pub fn Localization(&self) -> LocalizationModule<'_, R> {
        LocalizationModule::new(self.api)
    }

    #[inline]
    pub fn SceneDocs(&self) -> SceneDocModule<'_, R> {
        SceneDocModule::new(self.api)
    }

    #[inline]
    pub fn scene_load_doc(&self, path: &str) -> Result<SceneDoc, String> {
        self.api.scene_load_doc(path)
    }

    #[inline]
    pub fn scene_save_doc(&self, path: &str, doc: &SceneDoc) -> Result<(), String> {
        self.api.scene_save_doc(path, doc)
    }

    #[inline]
    pub fn scene_write<'a>(&self, doc: &'a SceneDoc) -> SceneWrite<'a> {
        SceneWrite::new(doc)
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

    #[inline]
    pub fn set_locale(&self, locale: Locale) -> bool {
        self.api.localization_set_locale(locale)
    }

    #[inline]
    pub fn locale_current(&self) -> Locale {
        self.api.localization_get_locale()
    }

    #[inline]
    pub fn locale<S: AsRef<str>>(&self, key: S) -> Option<&'static str> {
        self.api.localization_get(key.as_ref())
    }
}

#[macro_export]
macro_rules! get_viewport_size {
    ($res:expr) => {
        $res.viewport_size()
    };
}

