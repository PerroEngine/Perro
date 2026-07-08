use crate::Runtime;
use perro_ids::NodeID;
use perro_resource_api::{LoadError, LoadResult};
use perro_runtime_api::sub_apis::{PreloadedSceneID, SceneAPI};
use perro_scene::Scene;

impl SceneAPI for Runtime {
    fn scene_load(&mut self, path: &str) -> Result<NodeID, String> {
        self.scene_load_typed(path).map_err(|err| err.to_string())
    }

    fn scene_load_typed(&mut self, path: &str) -> LoadResult<NodeID> {
        self.load_scene_at_runtime(path).map_err(LoadError::Legacy)
    }

    fn scene_load_doc(&mut self, scene: Scene) -> Result<NodeID, String> {
        self.scene_load_doc_typed(scene)
            .map_err(|err| err.to_string())
    }

    fn scene_load_doc_typed(&mut self, scene: Scene) -> LoadResult<NodeID> {
        self.load_scene_doc_at_runtime(scene)
            .map_err(|message| LoadError::Prepare { message })
    }

    fn scene_load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String> {
        self.scene_load_hashed_typed(path_hash, path)
            .map_err(|err| err.to_string())
    }

    fn scene_load_hashed_typed(&mut self, path_hash: u64, path: &str) -> LoadResult<NodeID> {
        self.load_scene_at_runtime_hashed(path_hash, path)
            .map_err(LoadError::Legacy)
    }

    fn scene_preload(&mut self, path: &str) -> Result<PreloadedSceneID, String> {
        self.scene_preload_typed(path)
            .map_err(|err| err.to_string())
    }

    fn scene_preload_typed(&mut self, path: &str) -> LoadResult<PreloadedSceneID> {
        self.preload_scene_at_runtime(path)
            .map_err(LoadError::Legacy)
    }

    fn scene_preload_hashed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> Result<PreloadedSceneID, String> {
        self.scene_preload_hashed_typed(path_hash, path)
            .map_err(|err| err.to_string())
    }

    fn scene_preload_hashed_typed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> LoadResult<PreloadedSceneID> {
        self.preload_scene_at_runtime_hashed(path_hash, path)
            .map_err(LoadError::Legacy)
    }

    fn scene_load_preloaded(&mut self, id: PreloadedSceneID) -> Result<NodeID, String> {
        self.scene_load_preloaded_typed(id)
            .map_err(|err| err.to_string())
    }

    fn scene_load_preloaded_typed(&mut self, id: PreloadedSceneID) -> LoadResult<NodeID> {
        if !self.preloaded_scenes.contains_key(&id) {
            return Err(LoadError::InvalidHandle {
                kind: "preloaded scene",
                id: id.as_u64(),
            });
        }
        self.load_preloaded_scene_at_runtime(id)
            .map_err(LoadError::Legacy)
    }

    fn scene_free_preloaded(&mut self, id: PreloadedSceneID) -> bool {
        self.free_preloaded_scene_at_runtime(id)
    }

    fn scene_free_preloaded_by_path(&mut self, path: &str) -> bool {
        self.free_preloaded_scene_by_path_at_runtime(path)
    }

    fn scene_free_preloaded_by_path_hash(&mut self, path_hash: u64, path: &str) -> bool {
        self.free_preloaded_scene_by_path_at_runtime_hashed(path_hash, path)
    }
}
