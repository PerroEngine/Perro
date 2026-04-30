use perro_scene::{Scene, SceneDoc, SceneWrite};
use std::borrow::Cow;

pub trait IntoSceneDocPath {
    fn into_scene_doc_path(self) -> Cow<'static, str>;
}

impl IntoSceneDocPath for &'static str {
    fn into_scene_doc_path(self) -> Cow<'static, str> {
        Cow::Borrowed(self)
    }
}

impl IntoSceneDocPath for String {
    fn into_scene_doc_path(self) -> Cow<'static, str> {
        Cow::Owned(self)
    }
}

impl IntoSceneDocPath for &String {
    fn into_scene_doc_path(self) -> Cow<'static, str> {
        Cow::Owned(self.clone())
    }
}

impl IntoSceneDocPath for Cow<'static, str> {
    fn into_scene_doc_path(self) -> Cow<'static, str> {
        self
    }
}

impl IntoSceneDocPath for &Cow<'static, str> {
    fn into_scene_doc_path(self) -> Cow<'static, str> {
        self.clone()
    }
}

pub trait IntoSceneDoc {
    fn into_scene_doc(self) -> SceneDoc;
}

impl IntoSceneDoc for SceneDoc {
    fn into_scene_doc(self) -> SceneDoc {
        self
    }
}

impl IntoSceneDoc for &SceneDoc {
    fn into_scene_doc(self) -> SceneDoc {
        self.clone()
    }
}

impl IntoSceneDoc for Scene {
    fn into_scene_doc(self) -> SceneDoc {
        SceneDoc::from_scene(self)
    }
}

impl IntoSceneDoc for &Scene {
    fn into_scene_doc(self) -> SceneDoc {
        SceneDoc::from_scene(self.clone())
    }
}

pub trait SceneDocAPI {
    fn scene_load_doc(&self, path: &str) -> Result<SceneDoc, String>;
    fn scene_load_doc_hashed(&self, path_hash: u64, path: &str) -> Result<SceneDoc, String> {
        let _ = path_hash;
        self.scene_load_doc(path)
    }
    fn scene_save_doc(&self, path: &str, doc: &SceneDoc) -> Result<(), String>;
    fn scene_save_doc_hashed(
        &self,
        path_hash: u64,
        path: &str,
        doc: &SceneDoc,
    ) -> Result<(), String> {
        let _ = path_hash;
        self.scene_save_doc(path, doc)
    }
}

pub struct SceneDocModule<'res, R: SceneDocAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: SceneDocAPI + ?Sized> SceneDocModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    pub fn load<P: IntoSceneDocPath>(&self, path: P) -> Result<SceneDoc, String> {
        let path = path.into_scene_doc_path();
        self.api.scene_load_doc(path.as_ref())
    }

    pub fn load_hashed(&self, path_hash: u64, path: &str) -> Result<SceneDoc, String> {
        self.api.scene_load_doc_hashed(path_hash, path)
    }

    pub fn save<P: IntoSceneDocPath, D: IntoSceneDoc>(
        &self,
        path: P,
        doc: D,
    ) -> Result<(), String> {
        let path = path.into_scene_doc_path();
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api.scene_save_doc(path.as_ref(), &doc)
    }

    pub fn save_hashed<D: IntoSceneDoc>(
        &self,
        path_hash: u64,
        path: &str,
        doc: D,
    ) -> Result<(), String> {
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api.scene_save_doc_hashed(path_hash, path, &doc)
    }

    pub fn write<'a>(&self, doc: &'a SceneDoc) -> SceneWrite<'a> {
        SceneWrite::new(doc)
    }
}

#[macro_export]
macro_rules! scene_load_doc {
    ($res:expr, $path:literal) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $res.SceneDocs().load_hashed(__PATH_HASH, $path)
    }};
    ($res:expr, $path:expr) => {
        $res.SceneDocs().load($path)
    };
}

#[macro_export]
macro_rules! scene_save_doc {
    ($res:expr, $path:literal, $doc:expr) => {{
        const __PATH_HASH: u64 = $crate::__perro_string_to_u64($path);
        $res.SceneDocs().save_hashed(__PATH_HASH, $path, $doc)
    }};
    ($res:expr, $path:expr, $doc:expr) => {
        $res.SceneDocs().save($path, $doc)
    };
}
