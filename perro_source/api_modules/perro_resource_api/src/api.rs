use crate::sub_apis::{
    AnimationAPI, AnimationModule, AnimationTreeAPI, AnimationTreeModule, AudioAPI, AudioModule,
    CsvAPI, CsvModule, DisplayModule, Draw2DAPI, Draw2DModule, GlbModule, GltfAPI, IntoLocale,
    Locale, LocalizationAPI, LocalizationModule, MaterialAPI, MaterialModule, MeshAPI, MeshModule,
    MicAPI, MicModule, NavMeshAPI, NavMeshModule, PostProcessingAPI, SceneDocAPI, SceneDocModule,
    SkeletonAPI, SkeletonModule, TextureAPI, TextureModule, VideoAPI, VideoModule,
    VisualAccessibilityAPI, WebcamAPI, WebcamModule,
};
use crate::{LoadResult, ResPathSource};
use perro_render_bridge::{HdrMode, HdrStatus};
use perro_scene::{SceneDoc, SceneWrite};
use perro_structs::{ColorBlindFilter, PostProcessEffect, PostProcessSet, Vector2};

/// Full resource contract used by script contexts. Individual window accessors
/// require only their domain trait.
///
/// Resource stores implement this by implementing each resource sub-API plus
/// viewport access. Resource access stays on the runtime thread, so no
/// `Send`/`Sync` bound is required.
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
{
}

/// Viewport read access shared with resource scripts.
pub trait ViewportAPI {
    /// Return the active viewport size in pixels.
    fn viewport_size(&self) -> Vector2;

    /// Queue the fully composited display frame for image save.
    ///
    /// `true` means the request was queued. GPU readback and disk I/O finish
    /// asynchronously.
    fn save_display_image(&self, _path: &str) -> bool {
        false
    }

    fn set_hdr_mode(&self, _mode: HdrMode) {}

    fn hdr_status(&self) -> HdrStatus {
        HdrStatus::default()
    }
}

/// Script-facing resource facade.
///
/// `ResourceWindow` holds a shared resource API borrow for one script callback.
/// Domain accessors such as [`ResourceWindow::Textures`] and
/// [`ResourceWindow::Materials`] return lightweight wrappers over that borrow.
pub struct ResourceWindow<'res, R: ?Sized> {
    api: &'res R,
}

#[allow(non_snake_case)]
impl<'res, R: ?Sized> ResourceWindow<'res, R> {
    // ---- Construction ----

    /// Create a resource window around an existing resource API borrow.
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    // ---- Asset modules ----

    /// Access texture load, reserve, drop, and state queries.
    #[inline]
    pub fn Textures(&self) -> TextureModule<'_, R>
    where
        R: TextureAPI,
    {
        TextureModule::new(self.api)
    }

    /// Access video playback textures.
    #[inline]
    pub fn Videos(&self) -> VideoModule<'_, R>
    where
        R: VideoAPI,
    {
        VideoModule::new(self.api)
    }

    /// Access audio buffers, buses, MIDI, and playback helpers.
    #[inline]
    pub fn Audio(&self) -> AudioModule<'_, R>
    where
        R: AudioAPI,
    {
        AudioModule::new(self.api)
    }

    /// Access microphone capture, playback, save, and packed bytes.
    #[inline]
    pub fn Mic(&self) -> MicModule<'_, R>
    where
        R: MicAPI,
    {
        MicModule::new(self.api)
    }

    /// Access webcam capture and live texture helpers.
    #[inline]
    pub fn Webcams(&self) -> WebcamModule<'_, R>
    where
        R: WebcamAPI,
    {
        WebcamModule::new(self.api)
    }

    /// Access CSV load/save and query helpers.
    #[inline]
    pub fn Csv(&self) -> CsvModule<'_, R>
    where
        R: CsvAPI,
    {
        CsvModule::new(self.api)
    }

    /// Access mesh load, reserve, create, inspect, and write helpers.
    #[inline]
    pub fn Meshes(&self) -> MeshModule<'_, R>
    where
        R: MeshAPI,
    {
        MeshModule::new(self.api)
    }

    /// Access navigation mesh load, create, inspect, and write helpers.
    #[inline]
    pub fn NavMeshes(&self) -> NavMeshModule<'_, R>
    where
        R: NavMeshAPI,
    {
        NavMeshModule::new(self.api)
    }

    /// Access material load, reserve, create, inspect, and write helpers.
    #[inline]
    pub fn Materials(&self) -> MaterialModule<'_, R>
    where
        R: MaterialAPI,
    {
        MaterialModule::new(self.api)
    }

    /// Inspect GLB/GLTF files without loading them as scene resources.
    #[inline]
    pub fn Glbs(&self) -> GlbModule<'_, R>
    where
        R: GltfAPI,
    {
        GlbModule::new(self.api)
    }

    /// Access skeleton bone data loading.
    #[inline]
    pub fn Skeletons(&self) -> SkeletonModule<'_, R>
    where
        R: SkeletonAPI,
    {
        SkeletonModule::new(self.api)
    }

    /// Access animation load, reserve, drop, and state queries.
    #[inline]
    pub fn Animations(&self) -> AnimationModule<'_, R>
    where
        R: AnimationAPI,
    {
        AnimationModule::new(self.api)
    }

    /// Access animation tree load, drop, and state queries.
    #[inline]
    pub fn AnimationTrees(&self) -> AnimationTreeModule<'_, R>
    where
        R: AnimationTreeAPI,
    {
        AnimationTreeModule::new(self.api)
    }

    /// Access immediate 2D draw resource helpers.
    #[inline]
    pub fn Draw2D(&self) -> Draw2DModule<'_, R>
    where
        R: Draw2DAPI,
    {
        Draw2DModule::new(self.api)
    }

    /// Access display HDR state + control.
    #[inline]
    pub fn Display(&self) -> DisplayModule<'_, R>
    where
        R: ViewportAPI,
    {
        DisplayModule::new(self.api)
    }

    #[inline]
    pub fn set_hdr_mode(&self, mode: HdrMode)
    where
        R: ViewportAPI,
    {
        self.api.set_hdr_mode(mode);
    }

    #[inline]
    pub fn hdr_status(&self) -> HdrStatus
    where
        R: ViewportAPI,
    {
        self.api.hdr_status()
    }

    /// Access locale selection and localized string lookup.
    #[inline]
    pub fn Localization(&self) -> LocalizationModule<'_, R>
    where
        R: LocalizationAPI,
    {
        LocalizationModule::new(self.api)
    }

    /// Load, save, and write scene documents.
    #[inline]
    pub fn SceneDocs(&self) -> SceneDocModule<'_, R>
    where
        R: SceneDocAPI,
    {
        SceneDocModule::new(self.api)
    }

    // ---- Direct scene document helpers ----

    /// Load a scene document from a resource path.
    #[inline]
    pub fn scene_load_doc<P: ResPathSource>(&self, path: P) -> Result<SceneDoc, String>
    where
        R: SceneDocAPI,
    {
        self.api.scene_load_doc(path.as_res_path_str())
    }

    /// Load a scene document from a resource path with typed errors.
    #[inline]
    pub fn scene_load_doc_typed<P: ResPathSource>(&self, path: P) -> LoadResult<SceneDoc>
    where
        R: SceneDocAPI,
    {
        self.api.scene_load_doc_typed(path.as_res_path_str())
    }

    /// Save a scene document to a resource path.
    #[inline]
    pub fn scene_save_doc<P: ResPathSource>(&self, path: P, doc: &SceneDoc) -> Result<(), String>
    where
        R: SceneDocAPI,
    {
        self.api.scene_save_doc(path.as_res_path_str(), doc)
    }

    /// Save a scene document to a resource path with typed errors.
    #[inline]
    pub fn scene_save_doc_typed<P: ResPathSource>(&self, path: P, doc: &SceneDoc) -> LoadResult<()>
    where
        R: SceneDocAPI,
    {
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
    pub fn enable_colorblind_filter(&self, mode: ColorBlindFilter, strength: f32)
    where
        R: VisualAccessibilityAPI,
    {
        self.api.enable_color_blind_filter(mode, strength);
    }

    /// Disable the global colorblind filter.
    #[inline]
    pub fn disable_colorblind_filter(&self)
    where
        R: VisualAccessibilityAPI,
    {
        self.api.disable_color_blind_filter();
    }

    /// Replace the full global post-processing set.
    #[inline]
    pub fn set_global_post_processing(&self, set: PostProcessSet)
    where
        R: PostProcessingAPI,
    {
        self.api.set_global_post_processing(set);
    }

    /// Add a named global post-processing effect.
    #[inline]
    pub fn add_global_post_processing_named(
        &self,
        name: impl Into<std::borrow::Cow<'static, str>>,
        effect: PostProcessEffect,
    ) where
        R: PostProcessingAPI,
    {
        self.api
            .add_global_post_processing_named(name.into(), effect);
    }

    /// Add an unnamed global post-processing effect.
    #[inline]
    pub fn add_global_post_processing(&self, effect: PostProcessEffect)
    where
        R: PostProcessingAPI,
    {
        self.api.add_global_post_processing(effect);
    }

    /// Remove the first named global post-processing effect.
    #[inline]
    pub fn remove_global_post_processing_by_name(&self, name: &str) -> bool
    where
        R: PostProcessingAPI,
    {
        self.api.remove_global_post_processing_by_name(name)
    }

    /// Remove a global post-processing effect by index.
    #[inline]
    pub fn remove_global_post_processing_by_index(&self, index: usize) -> bool
    where
        R: PostProcessingAPI,
    {
        self.api.remove_global_post_processing_by_index(index)
    }

    /// Clear all global post-processing effects.
    #[inline]
    pub fn clear_global_post_processing(&self)
    where
        R: PostProcessingAPI,
    {
        self.api.clear_global_post_processing();
    }

    // ---- Viewport and localization shortcuts ----

    /// Return the active viewport size in pixels.
    #[inline]
    pub fn viewport_size(&self) -> Vector2
    where
        R: ViewportAPI,
    {
        self.api.viewport_size()
    }

    /// Set the active locale. Returns `true` when the locale exists.
    #[inline]
    pub fn set_locale<L: IntoLocale>(&self, locale: L) -> bool
    where
        R: LocalizationAPI,
    {
        self.api.localization_set_locale(locale.into_locale())
    }

    /// Return the active locale.
    #[inline]
    pub fn locale_current(&self) -> Locale
    where
        R: LocalizationAPI,
    {
        self.api.localization_get_locale()
    }

    /// Look up a localized string in the active locale.
    #[inline]
    pub fn locale<S: AsRef<str>>(&self, key: S) -> Option<&'static str>
    where
        R: LocalizationAPI,
    {
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

#[cfg(test)]
mod narrow_window_tests {
    use super::*;
    struct ViewportOnly;
    impl ViewportAPI for ViewportOnly {
        fn viewport_size(&self) -> Vector2 {
            Vector2::new(640.0, 480.0)
        }
    }
    #[test]
    fn viewport_window_does_not_require_asset_stores() {
        let viewport = ViewportOnly;
        let window = ResourceWindow::new(&viewport);
        assert_eq!(window.viewport_size(), Vector2::new(640.0, 480.0));
    }
}
