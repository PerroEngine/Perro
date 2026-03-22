use crate::{
    cns::{ScriptCollection, SignalConnection, SignalRegistry},
    render_result::RuntimeRenderResult,
    runtime::RuntimeScriptCtor,
};
use ahash::{AHashMap, AHashSet};
use perro_ids::{MaterialID, MeshID, NodeID, TagID, TextureID};
use perro_nodes::Spatial;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, Material3D, PointLight3DState, RayLight3DState,
    RenderCommand, RenderEvent, RenderRequestID, SpotLight3DState,
};
use perro_structs::{Transform2D, Transform3D};
use perro_terrain::ChunkCoord;

pub(crate) struct ScriptRuntimeState {
    pub(crate) active_script_stack: Vec<(usize, NodeID)>,
    pub(crate) last_node_lookup: Option<(NodeID, usize, u32)>,
    pub(crate) pending_start_scripts: Vec<NodeID>,
    pub(crate) pending_start_flags: Vec<Option<NodeID>>,
    pub(crate) script_library: Option<libloading::Library>,
    pub(crate) dynamic_script_registry: AHashMap<String, RuntimeScriptCtor>,
}

impl ScriptRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            active_script_stack: Vec::new(),
            last_node_lookup: None,
            pending_start_scripts: Vec::new(),
            pending_start_flags: Vec::new(),
            script_library: None,
            dynamic_script_registry: AHashMap::default(),
        }
    }
}

pub(crate) struct TransformRuntimeState {
    pub(crate) pending_transform_roots: Vec<NodeID>,
    pub(crate) traversal_stack: Vec<NodeID>,
    pub(crate) transform_visit_flags: Vec<u8>,
    pub(crate) transform_visit_indices: Vec<u32>,
    pub(crate) global_transform_2d: Vec<Transform2D>,
    pub(crate) global_transform_2d_valid: Vec<u8>,
    pub(crate) global_transform_2d_generation: Vec<u32>,
    pub(crate) global_transform_3d: Vec<Transform3D>,
    pub(crate) global_transform_3d_valid: Vec<u8>,
    pub(crate) global_transform_3d_generation: Vec<u32>,
    pub(crate) global_chain_scratch: Vec<NodeID>,
}

impl TransformRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            pending_transform_roots: Vec::new(),
            traversal_stack: Vec::new(),
            transform_visit_flags: Vec::new(),
            transform_visit_indices: Vec::new(),
            global_transform_2d: Vec::new(),
            global_transform_2d_valid: Vec::new(),
            global_transform_2d_generation: Vec::new(),
            global_transform_3d: Vec::new(),
            global_transform_3d_valid: Vec::new(),
            global_transform_3d_generation: Vec::new(),
            global_chain_scratch: Vec::new(),
        }
    }
}

pub(crate) struct InternalUpdateState {
    pub(crate) internal_update_nodes: Vec<NodeID>,
    pub(crate) internal_fixed_update_nodes: Vec<NodeID>,
    pub(crate) internal_update_pos: Vec<Option<usize>>,
    pub(crate) internal_fixed_update_pos: Vec<Option<usize>>,
    pub(crate) physics_body_nodes_2d: Vec<NodeID>,
    pub(crate) physics_body_nodes_3d: Vec<NodeID>,
    pub(crate) physics_body_pos_2d: Vec<Option<usize>>,
    pub(crate) physics_body_pos_3d: Vec<Option<usize>>,
}

impl InternalUpdateState {
    pub(crate) fn new() -> Self {
        Self {
            internal_update_nodes: Vec::new(),
            internal_fixed_update_nodes: Vec::new(),
            internal_update_pos: Vec::new(),
            internal_fixed_update_pos: Vec::new(),
            physics_body_nodes_2d: Vec::new(),
            physics_body_nodes_3d: Vec::new(),
            physics_body_pos_2d: Vec::new(),
            physics_body_pos_3d: Vec::new(),
        }
    }
}

pub(crate) struct SignalRuntimeState {
    pub(crate) registry: SignalRegistry,
    pub(crate) emit_scratch: Vec<SignalConnection>,
}

impl SignalRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            registry: SignalRegistry::new(),
            emit_scratch: Vec::new(),
        }
    }
}

pub(crate) struct NodeIndexState {
    pub(crate) node_tag_index: AHashMap<TagID, AHashSet<NodeID>>,
}

impl NodeIndexState {
    pub(crate) fn new() -> Self {
        Self {
            node_tag_index: AHashMap::default(),
        }
    }
}

/// Scratch buffers used to snapshot script update/fixed schedules without allocating each frame.
pub(crate) struct ScriptSchedules {
    pub(crate) update_slots: Vec<(usize, NodeID)>,
    pub(crate) fixed_slots: Vec<(usize, NodeID)>,
    update_epoch: u64,
    fixed_epoch: u64,
}

impl ScriptSchedules {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            update_slots: Vec::new(),
            fixed_slots: Vec::new(),
            update_epoch: u64::MAX,
            fixed_epoch: u64::MAX,
        }
    }

    pub(crate) fn snapshot_update<R: perro_runtime_context::api::RuntimeAPI + ?Sized>(
        &mut self,
        scripts: &ScriptCollection<R>,
    ) {
        let epoch = scripts.schedule_epoch();
        if self.update_epoch == epoch {
            return;
        }

        let needed = scripts.update_schedule_len();
        if self.update_slots.capacity() < needed {
            self.update_slots
                .reserve(needed - self.update_slots.capacity());
        }
        self.update_slots.clear();
        scripts.append_update_slots(&mut self.update_slots);
        self.update_epoch = epoch;
    }

    pub(crate) fn snapshot_fixed<R: perro_runtime_context::api::RuntimeAPI + ?Sized>(
        &mut self,
        scripts: &ScriptCollection<R>,
    ) {
        let epoch = scripts.schedule_epoch();
        if self.fixed_epoch == epoch {
            return;
        }

        let needed = scripts.fixed_schedule_len();
        if self.fixed_slots.capacity() < needed {
            self.fixed_slots
                .reserve(needed - self.fixed_slots.capacity());
        }
        self.fixed_slots.clear();
        scripts.append_fixed_update_slots(&mut self.fixed_slots);
        self.fixed_epoch = epoch;
    }
}

/// Runtime-side render exchange state:
/// queued outgoing commands and resolved incoming request results.
pub(crate) struct RenderState {
    pending_commands: Vec<RenderCommand>,
    resolved_requests: AHashMap<RenderRequestID, RuntimeRenderResult>,
    inflight_requests: AHashSet<RenderRequestID>,
}

impl RenderState {
    pub(crate) fn new() -> Self {
        Self {
            pending_commands: Vec::new(),
            resolved_requests: AHashMap::default(),
            inflight_requests: AHashSet::default(),
        }
    }

    pub(crate) fn queue_command(&mut self, command: RenderCommand) {
        self.pending_commands.push(command);
    }

    pub(crate) fn drain_commands(&mut self, out: &mut Vec<RenderCommand>) {
        out.append(&mut self.pending_commands);
    }

    pub(crate) fn apply_event(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::MeshCreated { request, id } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Mesh(id));
            }
            RenderEvent::TextureCreated { request, id } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Texture(id));
            }
            RenderEvent::MaterialCreated { request, id } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Material(id));
            }
            RenderEvent::Failed { request, reason } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Failed(reason));
            }
        }
    }

    pub(crate) fn take_result(&mut self, request: RenderRequestID) -> Option<RuntimeRenderResult> {
        self.resolved_requests.remove(&request)
    }

    pub(crate) fn is_inflight(&self, request: RenderRequestID) -> bool {
        self.inflight_requests.contains(&request)
    }

    pub(crate) fn mark_inflight(&mut self, request: RenderRequestID) {
        self.inflight_requests.insert(request);
    }
}

/// Runtime-side dirty tracking for downstream systems (rendering, transform propagation).
pub(crate) struct DirtyState {
    node_flags: Vec<u8>,
    dirty_indices: Vec<u32>,
    pending_transform_roots: Vec<NodeID>,
    pending_transform_root_flags: Vec<u8>,
}

pub(crate) struct Render2DState {
    pub(crate) traversal_ids: Vec<NodeID>,
    pub(crate) visible_now: AHashSet<NodeID>,
    pub(crate) prev_visible: AHashSet<NodeID>,
    pub(crate) retained_sprite_textures: AHashMap<NodeID, TextureID>,
    pub(crate) texture_sources: AHashMap<NodeID, String>,
    pub(crate) removed_nodes: Vec<NodeID>,
}

impl Render2DState {
    pub(crate) fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            retained_sprite_textures: AHashMap::default(),
            texture_sources: AHashMap::default(),
            removed_nodes: Vec::new(),
        }
    }
}

pub(crate) struct Render3DState {
    pub(crate) traversal_ids: Vec<NodeID>,
    pub(crate) visible_now: AHashSet<NodeID>,
    pub(crate) prev_visible: AHashSet<NodeID>,
    pub(crate) mesh_sources: AHashMap<NodeID, String>,
    pub(crate) material_sources: AHashMap<NodeID, String>,
    pub(crate) material_overrides: AHashMap<NodeID, Material3D>,
    pub(crate) terrain_material: MaterialID,
    pub(crate) terrain_chunk_meshes: AHashMap<TerrainChunkMeshKey, TerrainChunkMeshState>,
    pub(crate) terrain_debug_state: AHashMap<NodeID, TerrainDebugState>,
    pub(crate) particle_path_cache: AHashMap<String, perro_render_bridge::ParticleProfile3D>,
    pub(crate) last_camera: Option<Camera3DState>,
    pub(crate) retained_ambient_lights: AHashMap<NodeID, AmbientLight3DState>,
    pub(crate) retained_ray_lights: AHashMap<NodeID, RayLight3DState>,
    pub(crate) retained_point_lights: AHashMap<NodeID, PointLight3DState>,
    pub(crate) retained_spot_lights: AHashMap<NodeID, SpotLight3DState>,
    pub(crate) removed_nodes: Vec<NodeID>,
}

impl Render3DState {
    pub(crate) fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            mesh_sources: AHashMap::default(),
            material_sources: AHashMap::default(),
            material_overrides: AHashMap::default(),
            terrain_material: MaterialID::nil(),
            terrain_chunk_meshes: AHashMap::default(),
            terrain_debug_state: AHashMap::default(),
            particle_path_cache: AHashMap::default(),
            last_camera: None,
            retained_ambient_lights: AHashMap::default(),
            retained_ray_lights: AHashMap::default(),
            retained_point_lights: AHashMap::default(),
            retained_spot_lights: AHashMap::default(),
            removed_nodes: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TerrainChunkMeshKey {
    pub(crate) node: NodeID,
    pub(crate) coord: ChunkCoord,
}

#[derive(Clone, Debug)]
pub(crate) struct TerrainChunkMeshState {
    pub(crate) source: String,
    pub(crate) hash: u64,
    pub(crate) mesh: MeshID,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TerrainDebugState {
    pub(crate) signature: u64,
    pub(crate) point_count: u32,
    pub(crate) edge_count: u32,
}

impl DirtyState {
    pub(crate) const FLAG_RERENDER: u8 = 1 << 0;
    pub(crate) const FLAG_DIRTY_2D_TRANSFORM: u8 = 1 << 1;
    pub(crate) const FLAG_DIRTY_3D_TRANSFORM: u8 = 1 << 2;

    pub(crate) fn new() -> Self {
        Self {
            node_flags: Vec::new(),
            dirty_indices: Vec::new(),
            pending_transform_roots: Vec::new(),
            pending_transform_root_flags: Vec::new(),
        }
    }

    pub(crate) fn mark_rerender(&mut self, id: NodeID) {
        self.mark(id, Self::FLAG_RERENDER);
    }

    pub(crate) fn mark_transform(&mut self, id: NodeID, spatial: Spatial) {
        match spatial {
            Spatial::TwoD => {
                self.mark(id, Self::FLAG_DIRTY_2D_TRANSFORM);
            }
            Spatial::ThreeD => {
                self.mark(id, Self::FLAG_DIRTY_3D_TRANSFORM);
            }
            Spatial::None => {}
        }
    }

    #[inline]
    pub(crate) fn transform_mask(spatial: Spatial) -> u8 {
        match spatial {
            Spatial::TwoD => Self::FLAG_DIRTY_2D_TRANSFORM,
            Spatial::ThreeD => Self::FLAG_DIRTY_3D_TRANSFORM,
            Spatial::None => 0,
        }
    }

    #[inline]
    pub(crate) fn has_transform_dirty(&self, id: NodeID, spatial: Spatial) -> bool {
        let index = id.index() as usize;
        let mask = Self::transform_mask(spatial);
        if mask == 0 {
            return false;
        }
        self.node_flags
            .get(index)
            .copied()
            .is_some_and(|flags| (flags & mask) != 0)
    }

    #[inline]
    pub(crate) fn clear_transform_dirty(&mut self, id: NodeID, spatial: Spatial) {
        let index = id.index() as usize;
        let mask = Self::transform_mask(spatial);
        if mask == 0 {
            return;
        }
        if let Some(flags) = self.node_flags.get_mut(index) {
            *flags &= !mask;
        }
    }

    #[inline]
    pub(crate) fn clear_transform_dirty_at_index(&mut self, index: usize, mask: u8) {
        if let Some(flags) = self.node_flags.get_mut(index) {
            *flags &= !mask;
        }
    }

    #[inline]
    pub(crate) fn dirty_indices(&self) -> &[u32] {
        &self.dirty_indices
    }

    #[inline]
    pub(crate) fn transform_flags_at(&self, index: usize) -> u8 {
        self.node_flags.get(index).copied().unwrap_or(0)
            & (Self::FLAG_DIRTY_2D_TRANSFORM | Self::FLAG_DIRTY_3D_TRANSFORM)
    }

    pub(crate) fn mark_transform_root(&mut self, id: NodeID) {
        let index = id.index() as usize;
        if self.pending_transform_root_flags.len() <= index {
            self.pending_transform_root_flags.resize(index + 1, 0);
        }
        if self.pending_transform_root_flags[index] == 0 {
            self.pending_transform_root_flags[index] = 1;
            self.pending_transform_roots.push(id);
        }
    }

    pub(crate) fn take_pending_transform_roots(&mut self, out: &mut Vec<NodeID>) {
        out.clear();
        out.append(&mut self.pending_transform_roots);
        for id in out.iter().copied() {
            let index = id.index() as usize;
            if index < self.pending_transform_root_flags.len() {
                self.pending_transform_root_flags[index] = 0;
            }
        }
    }

    #[inline]
    fn mark(&mut self, id: NodeID, flag: u8) {
        let index = id.index() as usize;
        if self.node_flags.len() <= index {
            self.node_flags.resize(index + 1, 0);
        }
        let entry = &mut self.node_flags[index];
        if *entry == 0 {
            self.dirty_indices.push(index as u32);
        }
        *entry |= flag;
    }

    pub(crate) fn clear(&mut self) {
        for &index in &self.dirty_indices {
            let i = index as usize;
            if i < self.node_flags.len() {
                self.node_flags[i] = 0;
            }
        }
        self.dirty_indices.clear();

        for id in self.pending_transform_roots.drain(..) {
            let index = id.index() as usize;
            if index < self.pending_transform_root_flags.len() {
                self.pending_transform_root_flags[index] = 0;
            }
        }
    }
}
