use crate::Runtime;
use perro_ids::NodeID;
use perro_runtime_context::sub_apis::SceneAPI;

impl SceneAPI for Runtime {
    fn scene_load(&mut self, path: &str) -> Result<NodeID, String> {
        self.load_scene_at_runtime(path)
    }
}
