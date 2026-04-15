use perro_ids::NodeID;
use std::borrow::Cow;

pub type PreloadedSceneID = perro_ids::PreloadedSceneID;

pub trait IntoScenePath {
    fn into_scene_path(self) -> Cow<'static, str>;
}

impl IntoScenePath for &'static str {
    fn into_scene_path(self) -> Cow<'static, str> {
        Cow::Borrowed(self)
    }
}

impl IntoScenePath for String {
    fn into_scene_path(self) -> Cow<'static, str> {
        Cow::Owned(self)
    }
}

impl IntoScenePath for &String {
    fn into_scene_path(self) -> Cow<'static, str> {
        Cow::Owned(self.clone())
    }
}

impl IntoScenePath for Cow<'static, str> {
    fn into_scene_path(self) -> Cow<'static, str> {
        self
    }
}

impl IntoScenePath for &Cow<'static, str> {
    fn into_scene_path(self) -> Cow<'static, str> {
        self.clone()
    }
}

pub trait IntoPreloadedSceneID {
    fn into_preloaded_scene_id(self) -> PreloadedSceneID;
}

impl IntoPreloadedSceneID for PreloadedSceneID {
    fn into_preloaded_scene_id(self) -> PreloadedSceneID {
        self
    }
}

impl IntoPreloadedSceneID for &PreloadedSceneID {
    fn into_preloaded_scene_id(self) -> PreloadedSceneID {
        *self
    }
}

pub enum SceneLoadSource {
    Path(Cow<'static, str>),
    Preloaded(PreloadedSceneID),
}

pub trait IntoSceneLoadSource {
    fn into_scene_load_source(self) -> SceneLoadSource;
}

impl IntoSceneLoadSource for &'static str {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Path(Cow::Borrowed(self))
    }
}

impl IntoSceneLoadSource for String {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Path(Cow::Owned(self))
    }
}

impl IntoSceneLoadSource for &String {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Path(Cow::Owned(self.clone()))
    }
}

impl IntoSceneLoadSource for Cow<'static, str> {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Path(self)
    }
}

impl IntoSceneLoadSource for &Cow<'static, str> {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Path(self.clone())
    }
}

impl IntoSceneLoadSource for PreloadedSceneID {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Preloaded(self)
    }
}

impl IntoSceneLoadSource for &PreloadedSceneID {
    fn into_scene_load_source(self) -> SceneLoadSource {
        SceneLoadSource::Preloaded(*self)
    }
}

pub enum PreloadedSceneTarget {
    Id(PreloadedSceneID),
    Path(Cow<'static, str>),
}

pub trait IntoPreloadedSceneTarget {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget;
}

impl IntoPreloadedSceneTarget for PreloadedSceneID {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Id(self)
    }
}

impl IntoPreloadedSceneTarget for &PreloadedSceneID {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Id(*self)
    }
}

impl IntoPreloadedSceneTarget for &'static str {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Path(Cow::Borrowed(self))
    }
}

impl IntoPreloadedSceneTarget for String {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Path(Cow::Owned(self))
    }
}

impl IntoPreloadedSceneTarget for &String {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Path(Cow::Owned(self.clone()))
    }
}

impl IntoPreloadedSceneTarget for Cow<'static, str> {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Path(self)
    }
}

impl IntoPreloadedSceneTarget for &Cow<'static, str> {
    fn into_preloaded_scene_target(self) -> PreloadedSceneTarget {
        PreloadedSceneTarget::Path(self.clone())
    }
}

pub trait SceneAPI {
    fn scene_load(&mut self, path: &str) -> Result<NodeID, String>;
    fn scene_load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String> {
        let _ = path_hash;
        self.scene_load(path)
    }
    fn scene_preload(&mut self, _path: &str) -> Result<PreloadedSceneID, String> {
        Err("scene preload is not supported by this runtime".to_string())
    }
    fn scene_preload_hashed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> Result<PreloadedSceneID, String> {
        let _ = path_hash;
        self.scene_preload(path)
    }
    fn scene_load_preloaded(&mut self, _id: PreloadedSceneID) -> Result<NodeID, String> {
        Err("preloaded scene loading is not supported by this runtime".to_string())
    }
    fn scene_free_preloaded(&mut self, _id: PreloadedSceneID) -> bool {
        false
    }
    fn scene_free_preloaded_by_path(&mut self, _path: &str) -> bool {
        false
    }
    fn scene_free_preloaded_by_path_hash(&mut self, path_hash: u64, path: &str) -> bool {
        let _ = path_hash;
        self.scene_free_preloaded_by_path(path)
    }
}

pub struct SceneModule<'rt, R: SceneAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: SceneAPI + ?Sized> SceneModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn load<S: IntoSceneLoadSource>(&mut self, source: S) -> Result<NodeID, String> {
        match source.into_scene_load_source() {
            SceneLoadSource::Path(path) => self.rt.scene_load(path.as_ref()),
            SceneLoadSource::Preloaded(id) => self.rt.scene_load_preloaded(id),
        }
    }

    pub fn load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String> {
        self.rt.scene_load_hashed(path_hash, path)
    }

    pub fn preload<P: IntoScenePath>(&mut self, path: P) -> Result<PreloadedSceneID, String> {
        let path = path.into_scene_path();
        self.rt.scene_preload(path.as_ref())
    }

    pub fn preload_hashed(&mut self, path_hash: u64, path: &str) -> Result<PreloadedSceneID, String> {
        self.rt.scene_preload_hashed(path_hash, path)
    }

    pub fn load_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> Result<NodeID, String> {
        self.rt.scene_load_preloaded(id.into_preloaded_scene_id())
    }

    pub fn free_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> bool {
        self.rt.scene_free_preloaded(id.into_preloaded_scene_id())
    }

    pub fn drop_preloaded<T: IntoPreloadedSceneTarget>(&mut self, target: T) -> bool {
        match target.into_preloaded_scene_target() {
            PreloadedSceneTarget::Id(id) => self.rt.scene_free_preloaded(id),
            PreloadedSceneTarget::Path(path) => self.rt.scene_free_preloaded_by_path(path.as_ref()),
        }
    }

    pub fn drop_preloaded_hashed(&mut self, path_hash: u64, path: &str) -> bool {
        self.rt.scene_free_preloaded_by_path_hash(path_hash, path)
    }
}

#[macro_export]
macro_rules! scene_load {
    ($ctx:expr, $path:literal) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $ctx.Scene().load_hashed(__PATH_HASH, $path)
    }};
    ($ctx:expr, $path:expr) => {
        $ctx.Scene().load($path)
    };
}

#[macro_export]
macro_rules! scene_preload {
    ($ctx:expr, $path:literal) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $ctx.Scene().preload_hashed(__PATH_HASH, $path)
    }};
    ($ctx:expr, $path:expr) => {
        $ctx.Scene().preload($path)
    };
}

#[macro_export]
macro_rules! scene_free_preloaded {
    ($ctx:expr, $path:literal) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $ctx.Scene().drop_preloaded_hashed(__PATH_HASH, $path)
    }};
    ($ctx:expr, $target:expr) => {
        $ctx.Scene().drop_preloaded($target)
    };
}

#[macro_export]
macro_rules! scene_drop_preloaded {
    ($ctx:expr, $path:literal) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $ctx.Scene().drop_preloaded_hashed(__PATH_HASH, $path)
    }};
    ($ctx:expr, $target:expr) => {
        $ctx.Scene().drop_preloaded($target)
    };
}