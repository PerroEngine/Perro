use super::RuntimeResourceApi;
use perro_io::{load_asset, save_asset};
use perro_resource_api::{LoadError, LoadResult, sub_apis::SceneDocAPI};
use perro_scene::SceneDoc;

impl SceneDocAPI for RuntimeResourceApi {
    fn scene_load_doc(&self, path: &str) -> Result<SceneDoc, String> {
        self.scene_load_doc_typed(path)
            .map_err(|err| err.to_string())
    }

    fn scene_load_doc_typed(&self, path: &str) -> LoadResult<SceneDoc> {
        let bytes = load_asset(path).map_err(|err| LoadError::Read {
            path: path.to_string(),
            message: err.to_string(),
        })?;
        let source = std::str::from_utf8(&bytes).map_err(|err| LoadError::Utf8 {
            path: path.to_string(),
            message: err.to_string(),
        })?;
        Ok(SceneDoc::parse(source))
    }

    fn scene_save_doc(&self, path: &str, doc: &SceneDoc) -> Result<(), String> {
        self.scene_save_doc_typed(path, doc)
            .map_err(|err| err.to_string())
    }

    fn scene_save_doc_typed(&self, path: &str, doc: &SceneDoc) -> LoadResult<()> {
        let text = doc.to_text();
        save_asset(path, text.as_bytes()).map_err(|err| LoadError::Write {
            path: path.to_string(),
            message: err.to_string(),
        })
    }
}
