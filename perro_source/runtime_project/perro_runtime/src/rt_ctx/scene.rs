use crate::Runtime;
use perro_ids::NodeID;
use perro_runtime_context::sub_apis::{PreloadedSceneID, SceneAPI};

impl SceneAPI for Runtime {
    fn scene_load(&mut self, path: &str) -> Result<NodeID, String> {
        self.load_scene_at_runtime(path)
    }

    fn scene_load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String> {
        self.load_scene_at_runtime_hashed(path_hash, path)
    }

    fn scene_preload(&mut self, path: &str) -> Result<PreloadedSceneID, String> {
        self.preload_scene_at_runtime(path)
    }

    fn scene_preload_hashed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> Result<PreloadedSceneID, String> {
        self.preload_scene_at_runtime_hashed(path_hash, path)
    }

    fn scene_load_preloaded(&mut self, id: PreloadedSceneID) -> Result<NodeID, String> {
        self.load_preloaded_scene_at_runtime(id)
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
