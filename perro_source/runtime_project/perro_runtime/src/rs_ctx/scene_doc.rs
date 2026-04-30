use super::RuntimeResourceApi;
use perro_io::{load_asset, save_asset};
use perro_resource_context::sub_apis::SceneDocAPI;
use perro_scene::SceneDoc;

impl SceneDocAPI for RuntimeResourceApi {
    fn scene_load_doc(&self, path: &str) -> Result<SceneDoc, String> {
        let bytes =
            load_asset(path).map_err(|err| format!("failed to load scene `{path}`: {err}"))?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|err| format!("scene `{path}` is not valid UTF-8: {err}"))?;
        Ok(SceneDoc::parse(source))
    }

    fn scene_save_doc(&self, path: &str, doc: &SceneDoc) -> Result<(), String> {
        let text = doc.to_text();
        save_asset(path, text.as_bytes())
            .map_err(|err| format!("failed to save scene `{path}`: {err}"))
    }
}
