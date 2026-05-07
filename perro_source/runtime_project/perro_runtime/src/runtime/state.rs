use crate::{
    cns::{ScriptCollection, SignalConnection, SignalRegistry},
    render_result::RuntimeRenderResult,
    runtime::{RuntimeScriptBehavior, RuntimeScriptCtor},
};
use ahash::{AHashMap, AHashSet};
use perro_ids::{MeshID, NodeID, TagID};
use perro_nodes::Spatial;
use perro_render_bridge::{
    AmbientLight3DState, Camera3DState, DenseInstancePose3D, Material3D, MeshSurfaceBinding3D,
    PointLight3DState, RayLight3DState, RenderCommand, RenderEvent, RenderRequestID,
    SkeletonPalette, Sky3DState, SpotLight3DState, Sprite2DCommand, UiCommand, UiRectState,
};
use perro_structs::{Transform2D, Transform3D, Vector2};
use perro_ui::{ComputedUiRect, UiSizeMode, UiVector2};
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, sync::Arc};

pub(crate) struct ScriptRuntimeState {
    pub(crate) active_script_stack: Vec<(usize, NodeID)>,
    pub(crate) last_node_lookup: Option<(NodeID, usize, u32)>,
    pub(crate) pending_start_scripts: Vec<NodeID>,
    pub(crate) pending_start_flags: Vec<Option<NodeID>>,
    pub(crate) script_libraries: Vec<libloading::Library>,
    pub(crate) base_scripts_loaded: bool,
    pub(crate) mounted_dlc_script_libs: AHashMap<String, PathBuf>,
    pub(crate) loaded_dlc_script_libs: AHashSet<String>,
    pub(crate) script_instance_dlc_mounts: AHashMap<NodeID, String>,
    pub(crate) dynamic_script_registry: AHashMap<u64, RuntimeScriptCtor>,
    pub(crate) script_behavior_cache: AHashMap<u64, Arc<RuntimeScriptBehavior>>,
}

impl ScriptRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            active_script_stack: Vec::new(),
            last_node_lookup: None,
            pending_start_scripts: Vec::new(),
            pending_start_flags: Vec::new(),
            script_libraries: Vec::new(),
            base_scripts_loaded: false,
            mounted_dlc_script_libs: AHashMap::default(),
            loaded_dlc_script_libs: AHashSet::default(),
            script_instance_dlc_mounts: AHashMap::default(),
            dynamic_script_registry: AHashMap::default(),
            script_behavior_cache: AHashMap::default(),
        }
    }
}

pub(crate) struct TransformRuntimeState {
    pub(crate) pending_transform_roots: Vec<NodeID>,
    pub(crate) traversal_stack: Vec<NodeID>,
    pub(crate) transform_visit_flags: Vec<u8>,
    pub(crate) transform_visit_indices: Vec<u32>,
    pub(crate) global_transform_2d: Vec<Transform2D>,
    pub(crate) global_transform_2d_generation: Vec<u32>,
    pub(crate) global_transform_3d: Vec<Transform3D>,
    pub(crate) global_transform_3d_generation: Vec<u32>,
    pub(crate) global_chain_scratch: Vec<NodeID>,
    pub(crate) dirty_indices_scratch: Vec<u32>,
}

impl TransformRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            pending_transform_roots: Vec::new(),
            traversal_stack: Vec::new(),
            transform_visit_flags: Vec::new(),
            transform_visit_indices: Vec::new(),
            global_transform_2d: Vec::new(),
            global_transform_2d_generation: Vec::new(),
            global_transform_3d: Vec::new(),
            global_transform_3d_generation: Vec::new(),
            global_chain_scratch: Vec::new(),
            dirty_indices_scratch: Vec::new(),
        }
    }
}

pub(crate) struct InternalUpdateState {
    pub(crate) internal_update_nodes: Vec<NodeID>,
    pub(crate) internal_fixed_update_nodes: Vec<NodeID>,
    pub(crate) internal_update_pos: Vec<u32>,
    pub(crate) internal_fixed_update_pos: Vec<u32>,
    pub(crate) physics_body_nodes_2d: Vec<NodeID>,
    pub(crate) physics_body_nodes_3d: Vec<NodeID>,
    pub(crate) physics_body_pos_2d: Vec<u32>,
    pub(crate) physics_body_pos_3d: Vec<u32>,
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

pub(crate) struct NodeApiScratchState {
    pub(crate) remove_stack: Vec<NodeID>,
    pub(crate) remove_postorder: Vec<NodeID>,
    pub(crate) remove_visited: AHashSet<NodeID>,
}

impl NodeApiScratchState {
    pub(crate) fn new() -> Self {
        Self {
            remove_stack: Vec::new(),
            remove_postorder: Vec::new(),
            remove_visited: AHashSet::default(),
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

    pub(crate) fn snapshot_update(&mut self, scripts: &ScriptCollection) {
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

    pub(crate) fn snapshot_fixed(&mut self, scripts: &ScriptCollection) {
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
    queued_resource_commands_scratch: Vec<RenderCommand>,
    resolved_requests: AHashMap<RenderRequestID, RuntimeRenderResult>,
    inflight_requests: AHashSet<RenderRequestID>,
}

impl RenderState {
    pub(crate) fn new() -> Self {
        Self {
            pending_commands: Vec::new(),
            queued_resource_commands_scratch: Vec::new(),
            resolved_requests: AHashMap::default(),
            inflight_requests: AHashSet::default(),
        }
    }

    pub(crate) fn queue_command(&mut self, command: RenderCommand) {
        self.pending_commands.push(command);
    }

    pub(crate) fn queue_commands(&mut self, commands: &mut Vec<RenderCommand>) {
        self.pending_commands.append(commands);
    }

    pub(crate) fn drain_commands(&mut self, out: &mut Vec<RenderCommand>) {
        if out.capacity() - out.len() < self.pending_commands.len() {
            out.reserve(self.pending_commands.len() - (out.capacity() - out.len()));
        }
        out.append(&mut self.pending_commands);
    }

    pub(crate) fn take_resource_queue_scratch(&mut self) -> Vec<RenderCommand> {
        std::mem::take(&mut self.queued_resource_commands_scratch)
    }

    pub(crate) fn restore_resource_queue_scratch(&mut self, mut scratch: Vec<RenderCommand>) {
        scratch.clear();
        self.queued_resource_commands_scratch = scratch;
    }

    pub(crate) fn apply_event(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::MeshCreated { request, id, .. } => {
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

    pub(crate) fn has_inflight_requests(&self) -> bool {
        !self.inflight_requests.is_empty()
    }

    pub(crate) fn has_resolved_requests(&self) -> bool {
        !self.resolved_requests.is_empty()
    }

    pub(crate) fn is_request_inflight(&self, request: RenderRequestID) -> bool {
        self.inflight_requests.contains(&request)
    }

    pub(crate) fn copy_inflight_requests(&self, out: &mut Vec<RenderRequestID>) {
        out.clear();
        out.extend(self.inflight_requests.iter().copied());
    }
}

/// Runtime-side dirty tracking for downstream systems (rendering, transform propagation).
pub(crate) struct DirtyState {
    node_flags: Vec<u16>,
    dirty_indices: Vec<u32>,
    pending_transform_roots: Vec<NodeID>,
    pending_transform_root_flags: Vec<u8>,
}

pub(crate) struct Render2DState {
    pub(crate) traversal_ids: Vec<NodeID>,
    pub(crate) visible_now: AHashSet<NodeID>,
    pub(crate) prev_visible: AHashSet<NodeID>,
    pub(crate) retained_sprites: AHashMap<NodeID, Sprite2DCommand>,
    pub(crate) texture_sources: AHashMap<NodeID, String>,
    pub(crate) last_camera: Option<perro_render_bridge::Camera2DState>,
    pub(crate) removed_nodes: Vec<NodeID>,
}

pub(crate) struct RenderUiState {
    pub(crate) traversal_ids: Vec<NodeID>,
    pub(crate) traversal_seen: AHashSet<NodeID>,
    pub(crate) command_ids: Vec<NodeID>,
    pub(crate) command_seen: AHashSet<NodeID>,
    pub(crate) visible_now: AHashSet<NodeID>,
    pub(crate) prev_visible: AHashSet<NodeID>,
    pub(crate) computed_rects: AHashMap<NodeID, ComputedUiRect>,
    pub(crate) size_clamp_baselines: RefCell<AHashMap<NodeID, UiSizeClampBaseline>>,
    pub(crate) computed_scales: AHashMap<NodeID, Vector2>,
    pub(crate) auto_layout_computed: AHashSet<NodeID>,
    pub(crate) retained_commands: AHashMap<NodeID, UiCommand>,
    pub(crate) retained_rects: AHashMap<NodeID, UiRectState>,
    pub(crate) button_states: AHashMap<NodeID, UiButtonVisualState>,
    pub(crate) hovered_text_edit: Option<NodeID>,
    pub(crate) focused_text_edit: Option<NodeID>,
    pub(crate) pressed_text_edit: Option<NodeID>,
    pub(crate) text_edit_repeat_key: Option<perro_input::KeyCode>,
    pub(crate) text_edit_repeat_timer: f32,
    pub(crate) last_ui_pointer: Option<(Vector2, bool)>,
    pub(crate) cursor_icon: perro_ui::CursorIcon,
    pub(crate) removed_nodes: Vec<NodeID>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum UiButtonVisualState {
    #[default]
    Neutral,
    Hover,
    Pressed,
}

#[derive(Clone, Copy)]
pub(crate) struct UiSizeClampBaseline {
    pub(crate) size: Vector2,
    pub(crate) size_def: UiVector2,
    pub(crate) h_mode: UiSizeMode,
    pub(crate) v_mode: UiSizeMode,
}

impl RenderUiState {
    pub(crate) fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            traversal_seen: AHashSet::default(),
            command_ids: Vec::new(),
            command_seen: AHashSet::default(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            computed_rects: AHashMap::default(),
            size_clamp_baselines: RefCell::new(AHashMap::default()),
            computed_scales: AHashMap::default(),
            auto_layout_computed: AHashSet::default(),
            retained_commands: AHashMap::default(),
            retained_rects: AHashMap::default(),
            button_states: AHashMap::default(),
            hovered_text_edit: None,
            focused_text_edit: None,
            pressed_text_edit: None,
            text_edit_repeat_key: None,
            text_edit_repeat_timer: 0.0,
            last_ui_pointer: None,
            cursor_icon: perro_ui::CursorIcon::Default,
            removed_nodes: Vec::new(),
        }
    }
}

impl Render2DState {
    pub(crate) fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            retained_sprites: AHashMap::default(),
            texture_sources: AHashMap::default(),
            last_camera: None,
            removed_nodes: Vec::new(),
        }
    }
}

pub(crate) struct Render3DState {
    pub(crate) traversal_ids: Vec<NodeID>,
    pub(crate) visible_now: AHashSet<NodeID>,
    pub(crate) prev_visible: AHashSet<NodeID>,
    pub(crate) mesh_sources: AHashMap<NodeID, String>,
    pub(crate) material_surface_sources: AHashMap<NodeID, Vec<Option<String>>>,
    pub(crate) material_surface_overrides: AHashMap<NodeID, Vec<Option<Material3D>>>,
    pub(crate) collision_debug_state: AHashMap<NodeID, CollisionDebugState>,
    pub(crate) particle_path_cache: AHashMap<String, perro_render_bridge::ParticleProfile3D>,
    pub(crate) particle_path_cache_order: VecDeque<String>,
    pub(crate) last_camera: Option<Camera3DState>,
    pub(crate) retained_ambient_lights: AHashMap<NodeID, AmbientLight3DState>,
    pub(crate) retained_skies: AHashMap<NodeID, Sky3DState>,
    pub(crate) retained_ray_lights: AHashMap<NodeID, RayLight3DState>,
    pub(crate) retained_point_lights: AHashMap<NodeID, PointLight3DState>,
    pub(crate) retained_spot_lights: AHashMap<NodeID, SpotLight3DState>,
    pub(crate) retained_mesh_draws: AHashMap<NodeID, RetainedMeshDrawState>,
    pub(crate) skeleton_cache_scratch: AHashMap<NodeID, SkeletonPalette>,
    pub(crate) removed_nodes: Vec<NodeID>,
    pub(crate) force_full_scan_once: bool,
}

impl Render3DState {
    pub(crate) fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            mesh_sources: AHashMap::default(),
            material_surface_sources: AHashMap::default(),
            material_surface_overrides: AHashMap::default(),
            collision_debug_state: AHashMap::default(),
            particle_path_cache: AHashMap::default(),
            particle_path_cache_order: VecDeque::new(),
            last_camera: None,
            retained_ambient_lights: AHashMap::default(),
            retained_skies: AHashMap::default(),
            retained_ray_lights: AHashMap::default(),
            retained_point_lights: AHashMap::default(),
            retained_spot_lights: AHashMap::default(),
            retained_mesh_draws: AHashMap::default(),
            skeleton_cache_scratch: AHashMap::default(),
            removed_nodes: Vec::new(),
            force_full_scan_once: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RetainedMeshDrawState {
    pub(crate) mesh: MeshID,
    pub(crate) surfaces: std::sync::Arc<[MeshSurfaceBinding3D]>,
    pub(crate) instances: RetainedMeshInstanceState,
    pub(crate) skeleton: Option<SkeletonPalette>,
    pub(crate) meshlet_override: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RetainedMeshInstanceState {
    Matrices(std::sync::Arc<[[[f32; 4]; 4]]>),
    Dense {
        node_model: [[f32; 4]; 4],
        instance_scale: f32,
        poses: std::sync::Arc<[DenseInstancePose3D]>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CollisionDebugState {
    pub(crate) signature: u64,
    pub(crate) edge_count: u32,
}

impl DirtyState {
    pub(crate) const FLAG_RERENDER: u16 = 1 << 0;
    pub(crate) const FLAG_DIRTY_2D_TRANSFORM: u16 = 1 << 1;
    pub(crate) const FLAG_DIRTY_3D_TRANSFORM: u16 = 1 << 2;
    pub(crate) const DIRTY_TRANSFORM: u16 = 1 << 3;
    pub(crate) const DIRTY_LAYOUT_SELF: u16 = 1 << 4;
    pub(crate) const DIRTY_LAYOUT_PARENT: u16 = 1 << 5;
    pub(crate) const DIRTY_COMMANDS: u16 = 1 << 6;
    pub(crate) const DIRTY_TEXT: u16 = 1 << 7;
    pub(crate) const UI_DIRTY_MASK: u16 = Self::DIRTY_TRANSFORM
        | Self::DIRTY_LAYOUT_SELF
        | Self::DIRTY_LAYOUT_PARENT
        | Self::DIRTY_COMMANDS
        | Self::DIRTY_TEXT;
    pub(crate) const UI_LAYOUT_MASK: u16 = Self::DIRTY_TRANSFORM
        | Self::DIRTY_LAYOUT_SELF
        | Self::DIRTY_LAYOUT_PARENT
        | Self::DIRTY_TEXT;

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

    pub(crate) fn mark_ui(&mut self, id: NodeID, flags: u16) {
        self.mark(id, Self::FLAG_RERENDER | (flags & Self::UI_DIRTY_MASK));
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
    pub(crate) fn transform_mask(spatial: Spatial) -> u16 {
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
    pub(crate) fn clear_transform_dirty_at_index(&mut self, index: usize, mask: u16) {
        if let Some(flags) = self.node_flags.get_mut(index) {
            *flags &= !mask;
        }
    }

    #[inline]
    pub(crate) fn dirty_indices(&self) -> &[u32] {
        &self.dirty_indices
    }

    #[inline]
    pub(crate) fn transform_flags_at(&self, index: usize) -> u16 {
        self.node_flags.get(index).copied().unwrap_or(0)
            & (Self::FLAG_DIRTY_2D_TRANSFORM | Self::FLAG_DIRTY_3D_TRANSFORM)
    }

    #[inline]
    pub(crate) fn ui_flags_at(&self, index: usize) -> u16 {
        self.node_flags.get(index).copied().unwrap_or(0) & Self::UI_DIRTY_MASK
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
        if out.capacity() < self.pending_transform_roots.len() {
            out.reserve(self.pending_transform_roots.len() - out.capacity());
        }
        out.append(&mut self.pending_transform_roots);
        for id in out.iter().copied() {
            let index = id.index() as usize;
            if index < self.pending_transform_root_flags.len() {
                self.pending_transform_root_flags[index] = 0;
            }
        }
    }

    pub(crate) fn has_any_dirty(&self) -> bool {
        !self.dirty_indices.is_empty()
    }

    pub(crate) fn has_pending_transform_roots(&self) -> bool {
        !self.pending_transform_roots.is_empty()
    }

    pub(crate) fn dirty_count(&self) -> usize {
        self.dirty_indices.len()
    }

    #[inline]
    fn mark(&mut self, id: NodeID, flag: u16) {
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

    pub(crate) fn clear_keep_ui_dirty(&mut self) {
        let mut write = 0usize;
        let dirty_len = self.dirty_indices.len();
        for read in 0..dirty_len {
            let index = self.dirty_indices[read];
            let i = index as usize;
            if i >= self.node_flags.len() {
                continue;
            }
            let preserved = self.node_flags[i] & Self::UI_DIRTY_MASK;
            self.node_flags[i] = preserved;
            if preserved != 0 {
                self.dirty_indices[write] = index;
                write += 1;
            }
        }
        self.dirty_indices.truncate(write);

        for id in self.pending_transform_roots.drain(..) {
            let index = id.index() as usize;
            if index < self.pending_transform_root_flags.len() {
                self.pending_transform_root_flags[index] = 0;
            }
        }
    }
}
