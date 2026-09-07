//! Domain-owned state; runtime coordinates access at phase boundaries.
use super::*;

pub(crate) struct SceneRuntimeState {
    pub(crate) active_route_href: Option<String>,
    pub(crate) active_route_root: Option<NodeID>,
    pub(crate) scene_ownership_roots: AHashMap<NodeID, NodeID>,
    pub(crate) scene_cache: RefCell<ScenePathLruCache<Scene>>,
    pub(crate) prepared_scene_cache:
        RefCell<ScenePathLruCache<scene_loader::prepare::PreparedScene>>,
    pub(crate) preloaded_scenes: AHashMap<PreloadedSceneID, Arc<Scene>>,
    pub(crate) preloaded_prepared_scenes:
        AHashMap<PreloadedSceneID, Arc<scene_loader::prepare::PreparedScene>>,
    pub(crate) preloaded_scene_paths: AHashMap<u64, PreloadedSceneID>,
    pub(crate) preloaded_scene_reverse_paths: AHashMap<PreloadedSceneID, String>,
    pub(crate) next_preloaded_scene_id: u64,
    /// Handles whose load + prepare is running on a worker, by id and by path
    /// hash (the second one dedupes repeat requests for one path).
    pub(crate) pending_preloads: AHashMap<PreloadedSceneID, String>,
    pub(crate) pending_preload_paths: AHashMap<u64, PreloadedSceneID>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) scene_preload_tx:
        std::sync::mpsc::Sender<scene_loader::background::BackgroundPreloadResult>,
    pub(crate) scene_preload_rx:
        std::sync::mpsc::Receiver<scene_loader::background::BackgroundPreloadResult>,
}

impl SceneRuntimeState {
    pub(crate) fn new() -> Self {
        let (scene_preload_tx, scene_preload_rx) = std::sync::mpsc::channel();
        Self {
            active_route_href: None,
            active_route_root: None,
            scene_ownership_roots: AHashMap::new(),
            scene_cache: RefCell::new(ScenePathLruCache::default()),
            prepared_scene_cache: RefCell::new(ScenePathLruCache::default()),
            preloaded_scenes: AHashMap::new(),
            preloaded_prepared_scenes: AHashMap::new(),
            preloaded_scene_paths: AHashMap::new(),
            preloaded_scene_reverse_paths: AHashMap::new(),
            next_preloaded_scene_id: 1,
            pending_preloads: AHashMap::new(),
            pending_preload_paths: AHashMap::new(),
            scene_preload_tx,
            scene_preload_rx,
        }
    }
}
