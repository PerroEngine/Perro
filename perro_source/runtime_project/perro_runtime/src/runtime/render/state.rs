//! Domain-owned state; runtime coordinates access at phase boundaries.
use super::*;

pub(crate) struct ExtractionState {
    pub(crate) scene_texture_refs_cache: AHashMap<TextureID, Vec<NodeID>>,
    pub(crate) scene_mesh_refs_cache: AHashMap<MeshID, Vec<NodeID>>,
    pub(crate) scene_material_refs_cache: AHashMap<MaterialID, Vec<NodeID>>,
    /// last arena mutation_revision seen by resource-ref scan. gate re-scan.
    pub(crate) scene_resource_refs_scanned_version: u64,
    /// force resource-ref re-scan next drain. set on resource render events
    /// (pending resolve / retained invalidation) that arena version misses.
    pub(crate) scene_resource_refs_dirty: bool,
    /// reusable scratch maps 4 resource-ref scan; avoid per-frame alloc + clone.
    pub(crate) scene_resource_refs_scratch: SceneResourceRefsScratch,
    /// resource event seen since last flush; batch SubView dirty-mark scan
    /// once per apply batch instead of once per event (load storm = N events).
    pub(crate) resource_event_scan_pending: bool,
    /// materials loaded since last flush; batched retained-draw invalidation
    /// does 1 node pass 4 all of them instead of 1 pass per material.
    pub(crate) pending_material_invalidations: Vec<MaterialID>,
    /// shared member list 4 camera-stream collectors; refcount view of the
    /// world-membership cache, reassigned once per stream rebuild.
    pub(crate) camera_stream_node_scratch: Arc<[NodeID]>,
    /// reusable set: worlds holding >=1 dirty node this pass (+ sub-view owner
    /// chain). gates stream/sub-view state rebuild to changed worlds only.
    pub(crate) dirty_world_scratch: AHashSet<NodeID>,
    /// reusable stream-node candidate list per extraction pass.
    pub(crate) stream_node_scratch: Vec<NodeID>,
    /// per-camera flattened post-processing cache: (source set, effects Arc).
    /// unchanged sets hand out refcount clones instead of re-flattening +
    /// re-allocating the effects slice every extraction pass.
    pub(crate) camera_postfx_cache: AHashMap<
        NodeID,
        (
            perro_structs::PostProcessSet,
            Arc<[perro_structs::PostProcessEffect]>,
        ),
    >,
    /// stream/sub-view nodes with a live gpu-side CameraStream upsert. gates
    /// redundant RemoveNode traffic (each command wakes a full gpu frame).
    pub(crate) camera_stream_active: AHashSet<NodeID>,
    /// cross-refresh Arc retention 4 stream/sub-view lanes + whole states +
    /// skinning palettes; see [`world_state::StreamRetention`].
    pub(crate) stream_retention: world_state::StreamRetention,
    /// last built (output_texture, resolution, ui rect size) per ui stream /
    /// sub-view node. lets input-refresh command visits reuse the previous
    /// output w/o re-collecting the watched world; rect-size change (auto
    /// resolution) forces rebuild.
    pub(crate) ui_stream_render_info: AHashMap<NodeID, (TextureID, [u32; 2], [f32; 2])>,
    pub(crate) pending_camera_capture_removals: Vec<(NodeID, u8)>,
}

impl ExtractionState {
    pub(crate) fn new() -> Self {
        Self {
            scene_texture_refs_cache: AHashMap::new(),
            scene_mesh_refs_cache: AHashMap::new(),
            scene_material_refs_cache: AHashMap::new(),
            scene_resource_refs_scanned_version: u64::MAX,
            scene_resource_refs_dirty: true,
            scene_resource_refs_scratch: SceneResourceRefsScratch::default(),
            resource_event_scan_pending: false,
            pending_material_invalidations: Vec::new(),
            camera_stream_node_scratch: perro_render_bridge::empty_arc_slice(),
            dirty_world_scratch: AHashSet::new(),
            stream_node_scratch: Vec::new(),
            camera_postfx_cache: AHashMap::new(),
            camera_stream_active: AHashSet::new(),
            stream_retention: world_state::StreamRetention::default(),
            ui_stream_render_info: AHashMap::new(),
            pending_camera_capture_removals: Vec::new(),
        }
    }
}
