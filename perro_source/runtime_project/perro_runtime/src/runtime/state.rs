use crate::{
    cns::{ScriptCollection, SignalConnection, SignalRegistry},
    rs_ctx::RuntimeResourceApi,
    runtime::{RuntimeScriptBehavior, RuntimeScriptCtor},
};
use ahash::{AHashMap, AHashSet};
use perro_ids::{NodeID, SignalID, TagID};
use perro_input_api::InputSnapshot;
use perro_nodes::Spatial;
use perro_structs::{Transform2D, Transform3D};
use std::{path::PathBuf, sync::Arc};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
type DynamicScriptLibrary = libloading::Library;
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
type DynamicScriptLibrary = ();

pub(crate) struct ScriptRuntimeState {
    pub(crate) active_script_stack: Vec<(usize, NodeID)>,
    pub(crate) active_callback_context: Option<ScriptCallbackContext>,
    pub(crate) last_node_lookup: Option<(NodeID, usize, u32)>,
    pub(crate) pending_start_scripts: Vec<NodeID>,
    pub(crate) pending_start_flags: Vec<Option<NodeID>>,
    pub(crate) script_libraries: Vec<DynamicScriptLibrary>,
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
            active_callback_context: None,
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
    pub(crate) global_transform_2d_valid: Vec<u8>,
    pub(crate) global_transform_2d_generation: Vec<u32>,
    pub(crate) global_transform_3d: Vec<Transform3D>,
    pub(crate) global_transform_3d_valid: Vec<u8>,
    pub(crate) global_transform_3d_generation: Vec<u32>,
    pub(crate) global_chain_scratch: Vec<NodeID>,
    pub(crate) dirty_indices_scratch: Vec<u32>,
    pub(crate) physics_pose_2d: Vec<PhysicsPose2D>,
    pub(crate) physics_pose_3d: Vec<PhysicsPose3D>,
    pub(crate) physics_pose_ids_2d: Vec<NodeID>,
    pub(crate) physics_pose_ids_3d: Vec<NodeID>,
    pub(crate) physics_pose_id_flags_2d: Vec<u8>,
    pub(crate) physics_pose_id_flags_3d: Vec<u8>,
    pub(crate) render_alpha: f32,
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
            dirty_indices_scratch: Vec::new(),
            physics_pose_2d: Vec::new(),
            physics_pose_3d: Vec::new(),
            physics_pose_ids_2d: Vec::new(),
            physics_pose_ids_3d: Vec::new(),
            physics_pose_id_flags_2d: Vec::new(),
            physics_pose_id_flags_3d: Vec::new(),
            render_alpha: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PhysicsPose2D {
    pub(crate) prev: Transform2D,
    pub(crate) curr: Transform2D,
    pub(crate) parent: NodeID,
    pub(crate) generation: u32,
    pub(crate) valid: bool,
}

impl Default for PhysicsPose2D {
    fn default() -> Self {
        Self {
            prev: Transform2D::IDENTITY,
            curr: Transform2D::IDENTITY,
            parent: NodeID::nil(),
            generation: 0,
            valid: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PhysicsPose3D {
    pub(crate) prev: Transform3D,
    pub(crate) curr: Transform3D,
    pub(crate) parent: NodeID,
    pub(crate) generation: u32,
    pub(crate) valid: bool,
}

impl Default for PhysicsPose3D {
    fn default() -> Self {
        Self {
            prev: Transform3D::IDENTITY,
            curr: Transform3D::IDENTITY,
            parent: NodeID::nil(),
            generation: 0,
            valid: false,
        }
    }
}

pub(crate) struct InternalUpdateState {
    pub(crate) internal_update_nodes: Vec<NodeID>,
    pub(crate) internal_fixed_update_nodes: Vec<NodeID>,
    pub(crate) internal_fixed_dispatch_nodes: Vec<NodeID>,
    pub(crate) internal_update_pos: Vec<u32>,
    pub(crate) internal_fixed_update_pos: Vec<u32>,
    pub(crate) physics_body_nodes_2d: Vec<NodeID>,
    pub(crate) physics_body_nodes_3d: Vec<NodeID>,
    pub(crate) physics_joint_nodes_2d: Vec<NodeID>,
    pub(crate) physics_joint_nodes_3d: Vec<NodeID>,
    pub(crate) physics_body_pos_2d: Vec<u32>,
    pub(crate) physics_body_pos_3d: Vec<u32>,
    pub(crate) button_nodes_2d: Vec<NodeID>,
    pub(crate) button_pos_2d: Vec<u32>,
}

impl InternalUpdateState {
    pub(crate) fn new() -> Self {
        Self {
            internal_update_nodes: Vec::new(),
            internal_fixed_update_nodes: Vec::new(),
            internal_fixed_dispatch_nodes: Vec::new(),
            internal_update_pos: Vec::new(),
            internal_fixed_update_pos: Vec::new(),
            physics_body_nodes_2d: Vec::new(),
            physics_body_nodes_3d: Vec::new(),
            physics_joint_nodes_2d: Vec::new(),
            physics_joint_nodes_3d: Vec::new(),
            physics_body_pos_2d: Vec::new(),
            physics_body_pos_3d: Vec::new(),
            button_nodes_2d: Vec::new(),
            button_pos_2d: Vec::new(),
        }
    }
}

pub(crate) struct SignalRuntimeState {
    pub(crate) registry: SignalRegistry,
    pub(crate) emit_scratch: Vec<SignalConnection>,
    pub(crate) param_scratch: Vec<perro_variant::Variant>,
    pub(crate) queued_ui_signals: Vec<(SignalID, Arc<[perro_variant::Variant]>)>,
}

impl SignalRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            registry: SignalRegistry::new(),
            emit_scratch: Vec::new(),
            param_scratch: Vec::new(),
            queued_ui_signals: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ScriptCallbackContext {
    pub(crate) resource_api: *const RuntimeResourceApi,
    pub(crate) input: *const InputSnapshot,
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

    #[inline]
    pub(crate) fn fixed_slots_empty(&self) -> bool {
        self.fixed_slots.is_empty()
    }
}

/// Runtime-side dirty tracking for downstream systems (rendering, transform propagation).
pub(crate) struct DirtyState {
    node_flags: Vec<u16>,
    dirty_indices: Vec<u32>,
    pending_transform_roots: Vec<NodeID>,
    pending_transform_root_flags: Vec<u8>,
}

pub(crate) use perro_runtime_render::{
    CollisionDebugState, DenseInstancePoseCache, LocaleTextBinding, LocaleTextField,
    LocaleTextState, Render2DState, Render3DState, RenderState, RenderUiState,
    RetainedMeshDrawState, RetainedMeshInstanceState, UiButtonVisualState, UiSizeClampBaseline,
};
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
        out.reserve(self.pending_transform_roots.len());
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

    pub(crate) fn has_transform_dirty_any(&self) -> bool {
        self.dirty_indices.iter().any(|&index| {
            self.transform_flags_at(index as usize)
                & (Self::FLAG_DIRTY_2D_TRANSFORM | Self::FLAG_DIRTY_3D_TRANSFORM)
                != 0
        }) || self.has_pending_transform_roots()
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
