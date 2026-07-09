use crate::sub_apis::{
    AnimationAPI, AnimationModule, AnimationTreeAPI, AnimationTreeModule, AudioAPI, AudioModule,
    CsvAPI, CsvModule, Draw2DAPI, Draw2DModule, GlbModule, GltfAPI, IntoLocale, Locale,
    LocalizationAPI, LocalizationModule, MaterialAPI, MaterialModule, MeshAPI, MeshModule, MicAPI,
    MicModule, PostProcessingAPI, SceneDocAPI, SceneDocModule, SkeletonAPI, SkeletonModule,
    TextureAPI, TextureModule, VideoAPI, VideoModule, VisualAccessibilityAPI, WebcamAPI,
    WebcamModule,
};
use crate::{LoadResult, ResPathSource};
use perro_scene::{SceneDoc, SceneWrite};
use perro_structs::{ColorBlindFilter, PostProcessEffect, PostProcessSet, Vector2};

/// Full resource contract required by [`ResourceWindow`].
///
/// Resource stores implement this by implementing each resource sub-API plus
/// viewport access. The trait is `Send + Sync` because resource access is shared
/// with script contexts.
pub trait ResourceAPI:
    PostProcessingAPI
    + VisualAccessibilityAPI
    + AudioAPI
    + MicAPI
    + WebcamAPI
    + CsvAPI
    + TextureAPI
    + VideoAPI
    + MeshAPI
    + MaterialAPI
    + GltfAPI
    + SkeletonAPI
    + AnimationAPI
    + AnimationTreeAPI
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
        + MicAPI
        + WebcamAPI
        + CsvAPI
        + TextureAPI
        + VideoAPI
        + MeshAPI
        + MaterialAPI
        + GltfAPI
        + SkeletonAPI
        + AnimationAPI
        + AnimationTreeAPI
        + Draw2DAPI
        + LocalizationAPI
        + SceneDocAPI
        + ViewportAPI
        + Send
        + Sync
{
}

/// Viewport read access shared with resource scripts.
pub trait ViewportAPI {
    /// Return the active viewport size in pixels.
    fn viewport_size(&self) -> Vector2;
}

/// Script-facing resource facade.
///
/// `ResourceWindow` holds a shared resource API borrow for one script callback.
/// Domain accessors such as [`ResourceWindow::Textures`] and
/// [`ResourceWindow::Materials`] return lightweight wrappers over that borrow.
pub struct ResourceWindow<'res, R: ResourceAPI + ?Sized> {
    api: &'res R,
}

#[allow(non_snake_case)]
impl<'res, R: ResourceAPI + ?Sized> ResourceWindow<'res, R> {
    // ---- Construction ----

    /// Create a resource window around an existing resource API borrow.
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    // ---- Asset modules ----

    /// Access texture load, reserve, drop, and state queries.
    #[inline]
    pub fn Textures(&self) -> TextureModule<'_, R> {
        TextureModule::new(self.api)
    }

    /// Access video playback textures.
    #[inline]
    pub fn Videos(&self) -> VideoModule<'_, R> {
        VideoModule::new(self.api)
    }

    /// Access audio buffers, buses, MIDI, and playback helpers.
    #[inline]
    pub fn Audio(&self) -> AudioModule<'_, R> {
        AudioModule::new(self.api)
    }

    /// Access microphone capture, playback, save, and packed bytes.
    #[inline]
    pub fn Mic(&self) -> MicModule<'_, R> {
        MicModule::new(self.api)
    }

    /// Access webcam capture and live texture helpers.
    #[inline]
    pub fn Webcams(&self) -> WebcamModule<'_, R> {
        WebcamModule::new(self.api)
    }

    /// Access CSV load/save and query helpers.
    #[inline]
    pub fn Csv(&self) -> CsvModule<'_, R> {
        CsvModule::new(self.api)
    }

    /// Access mesh load, reserve, create, inspect, and write helpers.
    #[inline]
    pub fn Meshes(&self) -> MeshModule<'_, R> {
        MeshModule::new(self.api)
    }

    /// Access material load, reserve, create, inspect, and write helpers.
    #[inline]
    pub fn Materials(&self) -> MaterialModule<'_, R> {
        MaterialModule::new(self.api)
    }

    /// Inspect GLB/GLTF files without loading them as scene resources.
    #[inline]
    pub fn Glbs(&self) -> GlbModule<'_, R> {
        GlbModule::new(self.api)
    }

    /// Access skeleton bone data loading.
    #[inline]
    pub fn Skeletons(&self) -> SkeletonModule<'_, R> {
        SkeletonModule::new(self.api)
    }

    /// Access animation load, reserve, drop, and state queries.
    #[inline]
    pub fn Animations(&self) -> AnimationModule<'_, R> {
        AnimationModule::new(self.api)
    }

    /// Access animation tree load, drop, and state queries.
    #[inline]
    pub fn AnimationTrees(&self) -> AnimationTreeModule<'_, R> {
        AnimationTreeModule::new(self.api)
    }

    /// Access immediate 2D draw resource helpers.
    #[inline]
    pub fn Draw2D(&self) -> Draw2DModule<'_, R> {
        Draw2DModule::new(self.api)
    }

    /// Access locale selection and localized string lookup.
    #[inline]
    pub fn Localization(&self) -> LocalizationModule<'_, R> {
        LocalizationModule::new(self.api)
    }

    /// Load, save, and write scene documents.
    #[inline]
    pub fn SceneDocs(&self) -> SceneDocModule<'_, R> {
        SceneDocModule::new(self.api)
    }

    // ---- Direct scene document helpers ----

    /// Load a scene document from a resource path.
    #[inline]
    pub fn scene_load_doc<P: ResPathSource>(&self, path: P) -> Result<SceneDoc, String> {
        self.api.scene_load_doc(path.as_res_path_str())
    }

    /// Load a scene document from a resource path with typed errors.
    #[inline]
    pub fn scene_load_doc_typed<P: ResPathSource>(&self, path: P) -> LoadResult<SceneDoc> {
        self.api.scene_load_doc_typed(path.as_res_path_str())
    }

    /// Save a scene document to a resource path.
    #[inline]
    pub fn scene_save_doc<P: ResPathSource>(&self, path: P, doc: &SceneDoc) -> Result<(), String> {
        self.api.scene_save_doc(path.as_res_path_str(), doc)
    }

    /// Save a scene document to a resource path with typed errors.
    #[inline]
    pub fn scene_save_doc_typed<P: ResPathSource>(
        &self,
        path: P,
        doc: &SceneDoc,
    ) -> LoadResult<()> {
        self.api.scene_save_doc_typed(path.as_res_path_str(), doc)
    }

    /// Create a read-only writer helper for an existing scene document.
    #[inline]
    pub fn scene_write<'a>(&self, doc: &'a SceneDoc) -> SceneWrite<'a> {
        SceneWrite::new(doc)
    }

    // ---- Global visual state ----

    /// Enable a global colorblind simulation/filter pass.
    #[inline]
    pub fn enable_colorblind_filter(&self, mode: ColorBlindFilter, strength: f32) {
        self.api.enable_color_blind_filter(mode, strength);
    }

    /// Disable the global colorblind filter.
    #[inline]
    pub fn disable_colorblind_filter(&self) {
        self.api.disable_color_blind_filter();
    }

    /// Replace the full global post-processing set.
    #[inline]
    pub fn set_global_post_processing(&self, set: PostProcessSet) {
        self.api.set_global_post_processing(set);
    }

    /// Add a named global post-processing effect.
    #[inline]
    pub fn add_global_post_processing_named(
        &self,
        name: impl Into<std::borrow::Cow<'static, str>>,
        effect: PostProcessEffect,
    ) {
        self.api
            .add_global_post_processing_named(name.into(), effect);
    }

    /// Add an unnamed global post-processing effect.
    #[inline]
    pub fn add_global_post_processing(&self, effect: PostProcessEffect) {
        self.api.add_global_post_processing(effect);
    }

    /// Remove the first named global post-processing effect.
    #[inline]
    pub fn remove_global_post_processing_by_name(&self, name: &str) -> bool {
        self.api.remove_global_post_processing_by_name(name)
    }

    /// Remove a global post-processing effect by index.
    #[inline]
    pub fn remove_global_post_processing_by_index(&self, index: usize) -> bool {
        self.api.remove_global_post_processing_by_index(index)
    }

    /// Clear all global post-processing effects.
    #[inline]
    pub fn clear_global_post_processing(&self) {
        self.api.clear_global_post_processing();
    }

    // ---- Viewport and localization shortcuts ----

    /// Return the active viewport size in pixels.
    #[inline]
    pub fn viewport_size(&self) -> Vector2 {
        self.api.viewport_size()
    }

    /// Set the active locale. Returns `true` when the locale exists.
    #[inline]
    pub fn set_locale<L: IntoLocale>(&self, locale: L) -> bool {
        self.api.localization_set_locale(locale.into_locale())
    }

    /// Return the active locale.
    #[inline]
    pub fn locale_current(&self) -> Locale {
        self.api.localization_get_locale()
    }

    /// Look up a localized string in the active locale.
    #[inline]
    pub fn locale<S: AsRef<str>>(&self, key: S) -> Option<&'static str> {
        self.api.localization_get(key.as_ref())
    }
}

/// Return the active viewport size from a [`ResourceWindow`].
#[macro_export]
macro_rules! get_viewport_size {
    ($res:expr) => {
        $res.viewport_size()
    };
}
