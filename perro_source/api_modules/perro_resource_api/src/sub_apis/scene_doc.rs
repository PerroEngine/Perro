use crate::ResPathSource;
use perro_scene::{Scene, SceneDoc, SceneWrite};

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

    pub fn load<P: ResPathSource>(&self, path: P) -> Result<SceneDoc, String> {
        self.api.scene_load_doc(path.as_res_path_str())
    }

    pub fn load_hashed<P: ResPathSource>(
        &self,
        path_hash: u64,
        path: P,
    ) -> Result<SceneDoc, String> {
        self.api
            .scene_load_doc_hashed(path_hash, path.as_res_path_str())
    }

    pub fn save<P: ResPathSource, D: IntoSceneDoc>(&self, path: P, doc: D) -> Result<(), String> {
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api.scene_save_doc(path.as_res_path_str(), &doc)
    }

    pub fn save_hashed<P: ResPathSource, D: IntoSceneDoc>(
        &self,
        path_hash: u64,
        path: P,
        doc: D,
    ) -> Result<(), String> {
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api
            .scene_save_doc_hashed(path_hash, path.as_res_path_str(), &doc)
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
