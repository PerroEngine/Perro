//! Scene document resource API.
//!
//! Loads and saves editable scene documents.

use crate::{LoadError, LoadResult, ResPathSource};
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
    fn scene_load_doc_typed(&self, path: &str) -> LoadResult<SceneDoc> {
        self.scene_load_doc(path).map_err(LoadError::Legacy)
    }
    fn scene_load_doc_hashed(&self, path_hash: u64, path: &str) -> Result<SceneDoc, String> {
        let _ = path_hash;
        self.scene_load_doc(path)
    }
    fn scene_load_doc_hashed_typed(&self, path_hash: u64, path: &str) -> LoadResult<SceneDoc> {
        let _ = path_hash;
        self.scene_load_doc_typed(path)
    }
    fn scene_save_doc(&self, path: &str, doc: &SceneDoc) -> Result<(), String>;
    fn scene_save_doc_typed(&self, path: &str, doc: &SceneDoc) -> LoadResult<()> {
        self.scene_save_doc(path, doc).map_err(LoadError::Legacy)
    }
    fn scene_save_doc_hashed(
        &self,
        path_hash: u64,
        path: &str,
        doc: &SceneDoc,
    ) -> Result<(), String> {
        let _ = path_hash;
        self.scene_save_doc(path, doc)
    }
    fn scene_save_doc_hashed_typed(
        &self,
        path_hash: u64,
        path: &str,
        doc: &SceneDoc,
    ) -> LoadResult<()> {
        let _ = path_hash;
        self.scene_save_doc_typed(path, doc)
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

    pub fn load_typed<P: ResPathSource>(&self, path: P) -> LoadResult<SceneDoc> {
        self.api.scene_load_doc_typed(path.as_res_path_str())
    }

    pub fn load_hashed<P: ResPathSource>(
        &self,
        path_hash: u64,
        path: P,
    ) -> Result<SceneDoc, String> {
        self.api
            .scene_load_doc_hashed(path_hash, path.as_res_path_str())
    }

    pub fn load_hashed_typed<P: ResPathSource>(
        &self,
        path_hash: u64,
        path: P,
    ) -> LoadResult<SceneDoc> {
        self.api
            .scene_load_doc_hashed_typed(path_hash, path.as_res_path_str())
    }

    pub fn save<P: ResPathSource, D: IntoSceneDoc>(&self, path: P, doc: D) -> Result<(), String> {
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api.scene_save_doc(path.as_res_path_str(), &doc)
    }

    pub fn save_typed<P: ResPathSource, D: IntoSceneDoc>(&self, path: P, doc: D) -> LoadResult<()> {
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api.scene_save_doc_typed(path.as_res_path_str(), &doc)
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

    pub fn save_hashed_typed<P: ResPathSource, D: IntoSceneDoc>(
        &self,
        path_hash: u64,
        path: P,
        doc: D,
    ) -> LoadResult<()> {
        let mut doc = doc.into_scene_doc();
        doc.normalize_links();
        self.api
            .scene_save_doc_hashed_typed(path_hash, path.as_res_path_str(), &doc)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    struct LegacySceneDocApi;

    impl SceneDocAPI for LegacySceneDocApi {
        fn scene_load_doc(&self, path: &str) -> Result<SceneDoc, String> {
            Err(format!("load {path}"))
        }

        fn scene_save_doc(&self, path: &str, _doc: &SceneDoc) -> Result<(), String> {
            Err(format!("save {path}"))
        }
    }

    #[test]
    fn typed_scene_doc_defaults_wrap_legacy_errors() {
        let api = LegacySceneDocApi;
        let module = SceneDocModule::new(&api);

        assert_eq!(
            api.scene_load_doc_typed("res://missing.scn").unwrap_err(),
            LoadError::Legacy("load res://missing.scn".to_owned())
        );
        assert_eq!(
            module.load_typed("res://missing.scn").unwrap_err(),
            LoadError::Legacy("load res://missing.scn".to_owned())
        );

        let doc = SceneDoc {
            vars: Cow::Borrowed(&[]),
            scene: Scene {
                nodes: Cow::Borrowed(&[]),
                root: None,
                key_names: Cow::Borrowed(&[]),
            },
        };

        assert_eq!(
            api.scene_save_doc_typed("user://slot.scn", &doc)
                .unwrap_err(),
            LoadError::Legacy("save user://slot.scn".to_owned())
        );
        assert_eq!(
            module.save_typed("user://slot.scn", &doc).unwrap_err(),
            LoadError::Legacy("save user://slot.scn".to_owned())
        );
    }
}
