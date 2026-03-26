use perro_ids::NodeID;
use std::borrow::Cow;

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

pub trait SceneAPI {
    fn scene_load(&mut self, path: &str) -> Result<NodeID, String>;
}

pub struct SceneModule<'rt, R: SceneAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: SceneAPI + ?Sized> SceneModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn load<P: IntoScenePath>(&mut self, path: P) -> Result<NodeID, String> {
        let path = path.into_scene_path();
        self.rt.scene_load(path.as_ref())
    }
}

/// Scene loading macros.
///
/// Loads a scene by path and returns the loaded scene root `NodeID`.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeContext<_>`
/// - `path`: scene path (`&str`, `String`, `Cow<'static, str>`, and references)
#[macro_export]
macro_rules! scene_load {
    ($ctx:expr, $path:expr) => {
        $ctx.Scene().load($path)
    };
}
