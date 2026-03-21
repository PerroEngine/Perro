use crate::{
    cns::{NodeArena, ScriptCollection, TerrainStore},
    render_result::RuntimeRenderResult,
    rs_ctx::RuntimeResourceApi,
    runtime_project::{ProviderMode, RuntimeProject},
};
use glam::{Mat3, Mat4};
use perro_ids::NodeID;
use perro_input::{GamepadAxis, GamepadButton, InputContext, InputSnapshot, KeyCode, MouseButton};
use perro_nodes::{
    InternalFixedUpdate, InternalUpdate, Node2D, Node3D, NodeType, SceneNodeData, Spatial,
};
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};
use perro_resource_context::ResourceContext;
use perro_runtime_context::RuntimeContext;
use perro_scripting::ScriptConstructor;
use perro_structs::{Transform2D, Transform3D};
use perro_terrain::{ChunkCoord, TerrainData};
use std::sync::{Arc, Mutex};

mod physics;
mod render_2d;
mod render_3d;
mod scene_loader;
mod state;
use state::{
    DirtyState, InternalUpdateState, NodeIndexState, Render2DState, Render3DState, RenderState,
    ScriptRuntimeState, ScriptSchedules, SignalRuntimeState, TransformRuntimeState,
};
pub(crate) use state::{TerrainChunkMeshKey, TerrainChunkMeshState, TerrainDebugState};

type RuntimeScriptCtor = ScriptConstructor<Runtime, RuntimeResourceApi, InputSnapshot>;
type StaticScriptRegistry = &'static [(&'static str, RuntimeScriptCtor)];

pub struct Runtime {
    pub time: Timing,
    provider_mode: ProviderMode,
    project: Option<Arc<RuntimeProject>>,

    // Core world state
    pub nodes: NodeArena,
    pub(crate) scripts: ScriptCollection<Self>,
    schedules: ScriptSchedules,
    pub(crate) script_runtime: ScriptRuntimeState,
    render: RenderState,
    dirty: DirtyState,
    transforms: TransformRuntimeState,
    internal_updates: InternalUpdateState,

    render_2d: Render2DState,
    render_3d: Render3DState,
    pub(crate) terrain_store: Arc<Mutex<TerrainStore>>,
    pub(crate) signal_runtime: SignalRuntimeState,
    pub(crate) node_index: NodeIndexState,
    pub(crate) resource_api: Arc<RuntimeResourceApi>,
    pub(crate) input: InputSnapshot,
    physics: physics::PhysicsState,
}

pub struct Timing {
    pub fixed_delta: f32,
    pub delta: f32,
    pub elapsed: f32,
}

impl Runtime {
    pub fn new() -> Self {
        let terrain_store = Arc::new(Mutex::new(TerrainStore::new()));
        Self {
            time: Timing {
                fixed_delta: 0.0,
                delta: 0.0,
                elapsed: 0.0,
            },
            provider_mode: ProviderMode::Dynamic,
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            schedules: ScriptSchedules::new(),
            script_runtime: ScriptRuntimeState::new(),
            project: None,
            render: RenderState::new(),
            dirty: DirtyState::new(),
            transforms: TransformRuntimeState::new(),
            internal_updates: InternalUpdateState::new(),
            render_2d: Render2DState::new(),
            render_3d: Render3DState::new(),
            terrain_store: terrain_store.clone(),
            signal_runtime: SignalRuntimeState::new(),
            node_index: NodeIndexState::new(),
            resource_api: RuntimeResourceApi::new(None, None, None, terrain_store),
            input: InputSnapshot::new(),
            physics: physics::PhysicsState::new(),
        }
    }

    pub fn from_project(project: RuntimeProject, provider_mode: ProviderMode) -> Self {
        Self::from_project_with_script_registry(project, provider_mode, None)
    }

    pub fn from_project_with_script_registry(
        project: RuntimeProject,
        provider_mode: ProviderMode,
        script_registry: Option<StaticScriptRegistry>,
    ) -> Self {
        let mut runtime = Self::new();
        let static_material_lookup = project.static_material_lookup;
        let static_audio_lookup = project.static_audio_lookup;
        let static_skeleton_lookup = project.static_skeleton_lookup;
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = provider_mode;
        runtime.resource_api = RuntimeResourceApi::new(
            static_material_lookup,
            static_audio_lookup,
            static_skeleton_lookup,
            runtime.terrain_store.clone(),
        );
        if let Some(entries) = script_registry {
            for (path, ctor) in entries {
                runtime
                    .script_runtime.dynamic_script_registry
                    .insert((*path).to_string(), *ctor);
            }
        }
        if let Err(err) = runtime.load_boot_scene() {
            panic!("failed to load boot scene: {err}");
        }
        runtime
    }

    pub fn project(&self) -> Option<&RuntimeProject> {
        self.project.as_deref()
    }

    pub fn provider_mode(&self) -> ProviderMode {
        self.provider_mode
    }

    #[inline]
    pub fn update(&mut self, delta_time: f32) {
        self.time.delta = delta_time;
        self.run_start_schedule();
        self.schedules.snapshot_update(&self.scripts);
        self.run_update_schedule();
        self.run_internal_update_schedule();
    }

    #[inline]
    pub fn fixed_update(&mut self, fixed_delta_time: f32) {
        self.time.fixed_delta = fixed_delta_time;
        self.schedules.snapshot_fixed(&self.scripts);
        self.run_fixed_schedule();
        self.physics_fixed_step();
        self.run_internal_fixed_update_schedule();
    }

    #[inline]
    pub fn begin_input_frame(&mut self) {
        self.input.apply_queued_commands();
        self.input.begin_frame();
    }

    #[inline]
    pub fn set_key_state(&mut self, key: KeyCode, is_down: bool) {
        self.input.set_key_state(key, is_down);
    }

    #[inline]
    pub fn set_mouse_button_state(&mut self, button: MouseButton, is_down: bool) {
        self.input.set_mouse_button_state(button, is_down);
    }

    #[inline]
    pub fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.input.add_mouse_delta(dx, dy);
    }

    #[inline]
    pub fn add_mouse_wheel(&mut self, dx: f32, dy: f32) {
        self.input.add_mouse_wheel(dx, dy);
    }

    #[inline]
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.input.set_mouse_position(x, y);
    }

    #[inline]
    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.input.set_viewport_size(width, height);
    }

    #[inline]
    pub fn set_gamepad_button_state(&mut self, index: usize, button: GamepadButton, is_down: bool) {
        self.input.set_gamepad_button_state(index, button, is_down);
    }

    #[inline]
    pub fn set_gamepad_axis(&mut self, index: usize, axis: GamepadAxis, value: f32) {
        self.input.set_gamepad_axis(index, axis, value);
    }

    #[inline]
    pub fn set_gamepad_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_gamepad_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_gamepad_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_gamepad_accel(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_button_state(
        &mut self,
        index: usize,
        button: perro_input::JoyConButton,
        is_down: bool,
    ) {
        self.input.set_joycon_button_state(index, button, is_down);
    }

    #[inline]
    pub fn set_joycon_stick(&mut self, index: usize, x: f32, y: f32) {
        self.input.set_joycon_stick(index, x, y);
    }

    #[inline]
    pub fn set_joycon_gyro(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_gyro(index, x, y, z);
    }

    #[inline]
    pub fn set_joycon_accel(&mut self, index: usize, x: f32, y: f32, z: f32) {
        self.input.set_joycon_accel(index, x, y, z);
    }

    pub fn queue_render_command(&mut self, command: RenderCommand) {
        self.render.queue_command(command);
    }

    pub fn drain_render_commands(&mut self, out: &mut Vec<RenderCommand>) {
        let mut queued_resource_commands = Vec::new();
        self.resource_api
            .drain_commands(&mut queued_resource_commands);
        for command in queued_resource_commands {
            self.render.queue_command(command);
        }
        self.render.drain_commands(out);
    }

    pub fn apply_render_event(&mut self, event: RenderEvent) {
        self.resource_api.apply_render_event(&event);
        self.render.apply_event(event);
    }

    pub fn apply_render_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = RenderEvent>,
    {
        for event in events {
            self.apply_render_event(event);
        }
    }

    pub fn take_render_result(&mut self, request: RenderRequestID) -> Option<RuntimeRenderResult> {
        self.render.take_result(request)
    }

    pub fn mark_needs_rerender(&mut self, id: NodeID) {
        self.dirty.mark_rerender(id);
    }

    pub fn mark_transform_dirty_recursive(&mut self, root: NodeID) {
        self.dirty.mark_transform_root(root);
    }

    pub(crate) fn propagate_pending_transform_dirty(&mut self) {
        let mut roots = std::mem::take(&mut self.transforms.pending_transform_roots);
        self.dirty.take_pending_transform_roots(&mut roots);
        if roots.is_empty() {
            self.transforms.pending_transform_roots = roots;
            return;
        }

        let mut stack = std::mem::take(&mut self.transforms.traversal_stack);
        stack.clear();

        for root in roots.iter().copied() {
            if self.nodes.get(root).is_none() {
                continue;
            }
            stack.push(root);
            while let Some(id) = stack.pop() {
                let index = id.index() as usize;
                if self.transforms.transform_visit_flags.len() <= index {
                    self.transforms.transform_visit_flags.resize(index + 1, 0);
                }
                if self.transforms.transform_visit_flags[index] != 0 {
                    continue;
                }
                self.transforms.transform_visit_flags[index] = 1;
                self.transforms.transform_visit_indices.push(index as u32);

                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                self.dirty.mark_transform(id, node.spatial());
                stack.extend(node.children_slice().iter().copied());
            }
        }

        for &index in &self.transforms.transform_visit_indices {
            let i = index as usize;
            if i < self.transforms.transform_visit_flags.len() {
                self.transforms.transform_visit_flags[i] = 0;
            }
        }
        self.transforms.transform_visit_indices.clear();

        stack.clear();
        self.transforms.traversal_stack = stack;
        roots.clear();
        self.transforms.pending_transform_roots = roots;
    }

    #[inline]
    fn ensure_global_2d_capacity(&mut self, index: usize) {
        if self.transforms.global_transform_2d.len() <= index {
            self.transforms.global_transform_2d
                .resize(index + 1, Transform2D::IDENTITY);
        }
        if self.transforms.global_transform_2d_valid.len() <= index {
            self.transforms.global_transform_2d_valid.resize(index + 1, 0);
        }
        if self.transforms.global_transform_2d_generation.len() <= index {
            self.transforms.global_transform_2d_generation.resize(index + 1, 0);
        }
    }

    #[inline]
    fn ensure_global_3d_capacity(&mut self, index: usize) {
        if self.transforms.global_transform_3d.len() <= index {
            self.transforms.global_transform_3d
                .resize(index + 1, Transform3D::IDENTITY);
        }
        if self.transforms.global_transform_3d_valid.len() <= index {
            self.transforms.global_transform_3d_valid.resize(index + 1, 0);
        }
        if self.transforms.global_transform_3d_generation.len() <= index {
            self.transforms.global_transform_3d_generation.resize(index + 1, 0);
        }
    }

    fn is_global_2d_cached_clean(&self, id: NodeID) -> bool {
        let index = id.index() as usize;
        if self
            .transforms.global_transform_2d_valid
            .get(index)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return false;
        }
        if self
            .transforms.global_transform_2d_generation
            .get(index)
            .copied()
            .unwrap_or(u32::MAX)
            != id.generation()
        {
            return false;
        }
        !self.dirty.has_transform_dirty(id, Spatial::TwoD)
    }

    fn is_global_3d_cached_clean(&self, id: NodeID) -> bool {
        let index = id.index() as usize;
        if self
            .transforms.global_transform_3d_valid
            .get(index)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return false;
        }
        if self
            .transforms.global_transform_3d_generation
            .get(index)
            .copied()
            .unwrap_or(u32::MAX)
            != id.generation()
        {
            return false;
        }
        !self.dirty.has_transform_dirty(id, Spatial::ThreeD)
    }

    pub(crate) fn get_global_transform_2d(&mut self, id: NodeID) -> Option<Transform2D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
        }
        let start_index = id.index() as usize;
        self.ensure_global_2d_capacity(start_index);
        if self.is_global_2d_cached_clean(id) {
            return self.transforms.global_transform_2d.get(start_index).copied();
        }

        let mut chain = std::mem::take(&mut self.transforms.global_chain_scratch);
        chain.clear();

        let mut cursor = id;
        let mut parent_world = Mat3::IDENTITY;
        let max_hops = self.nodes.len().saturating_add(1);
        let mut hops = 0usize;

        while hops < max_hops {
            let Some((parent, _local)) = self.nodes.get(cursor).and_then(|node| {
                node.with_base_ref::<Node2D, _>(|base| (node.parent, base.transform))
            }) else {
                break;
            };
            let index = cursor.index() as usize;
            self.ensure_global_2d_capacity(index);
            let dirty = self.dirty.has_transform_dirty(cursor, Spatial::TwoD);
            let cached_clean = self.is_global_2d_cached_clean(cursor);
            if cached_clean && !dirty {
                parent_world = self.transforms.global_transform_2d[index].to_mat3();
                break;
            }
            chain.push(cursor);

            if parent.is_nil() {
                break;
            }
            if self.nodes.get(parent).is_none() {
                break;
            }
            cursor = parent;
            hops += 1;
        }

        for chain_id in chain.iter().rev().copied() {
            let Some((local, parent)) = self.nodes.get(chain_id).and_then(|node| {
                node.with_base_ref::<Node2D, _>(|base| (base.transform, node.parent))
            }) else {
                continue;
            };
            let parent_is_2d = !parent.is_nil()
                && self
                    .nodes
                    .get(parent)
                    .and_then(|node| node.with_base_ref::<Node2D, _>(|_| ()))
                    .is_some();
            let (global, world) = if parent_is_2d {
                let world = parent_world * local.to_mat3();
                (Transform2D::from_mat3(world), world)
            } else {
                (local, local.to_mat3())
            };
            let index = chain_id.index() as usize;
            self.ensure_global_2d_capacity(index);
            self.transforms.global_transform_2d[index] = global;
            self.transforms.global_transform_2d_valid[index] = 1;
            self.transforms.global_transform_2d_generation[index] = chain_id.generation();
            self.dirty.clear_transform_dirty(chain_id, Spatial::TwoD);
            parent_world = world;
        }

        let result = self.transforms.global_transform_2d.get(start_index).copied();
        chain.clear();
        self.transforms.global_chain_scratch = chain;
        result
    }

    pub(crate) fn get_global_transform_3d(&mut self, id: NodeID) -> Option<Transform3D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
        }
        let start_index = id.index() as usize;
        self.ensure_global_3d_capacity(start_index);
        if self.is_global_3d_cached_clean(id) {
            return self.transforms.global_transform_3d.get(start_index).copied();
        }

        let mut chain = std::mem::take(&mut self.transforms.global_chain_scratch);
        chain.clear();

        let mut cursor = id;
        let mut parent_world = Mat4::IDENTITY;
        let max_hops = self.nodes.len().saturating_add(1);
        let mut hops = 0usize;

        while hops < max_hops {
            let Some((parent, _local)) = self.nodes.get(cursor).and_then(|node| {
                node.with_base_ref::<Node3D, _>(|base| (node.parent, base.transform))
            }) else {
                break;
            };
            let index = cursor.index() as usize;
            self.ensure_global_3d_capacity(index);
            let dirty = self.dirty.has_transform_dirty(cursor, Spatial::ThreeD);
            let cached_clean = self.is_global_3d_cached_clean(cursor);
            if cached_clean && !dirty {
                parent_world = self.transforms.global_transform_3d[index].to_mat4();
                break;
            }
            chain.push(cursor);

            if parent.is_nil() {
                break;
            }
            if self.nodes.get(parent).is_none() {
                break;
            }
            cursor = parent;
            hops += 1;
        }

        for chain_id in chain.iter().rev().copied() {
            let Some((local, parent)) = self.nodes.get(chain_id).and_then(|node| {
                node.with_base_ref::<Node3D, _>(|base| (base.transform, node.parent))
            }) else {
                continue;
            };
            let parent_is_3d = !parent.is_nil()
                && self
                    .nodes
                    .get(parent)
                    .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
                    .is_some();
            let (global, world) = if parent_is_3d {
                let world = parent_world * local.to_mat4();
                (Transform3D::from_mat4(world), world)
            } else {
                (local, local.to_mat4())
            };
            let index = chain_id.index() as usize;
            self.ensure_global_3d_capacity(index);
            self.transforms.global_transform_3d[index] = global;
            self.transforms.global_transform_3d_valid[index] = 1;
            self.transforms.global_transform_3d_generation[index] = chain_id.generation();
            self.dirty.clear_transform_dirty(chain_id, Spatial::ThreeD);
            parent_world = world;
        }

        let result = self.transforms.global_transform_3d.get(start_index).copied();
        chain.clear();
        self.transforms.global_chain_scratch = chain;
        result
    }

    pub(crate) fn refresh_dirty_global_transforms(&mut self) {
        let dirty_indices = self.dirty.dirty_indices().to_vec();
        for raw_index in dirty_indices {
            let index = raw_index as usize;
            let flags = self.dirty.transform_flags_at(index);
            if flags == 0 {
                continue;
            }
            let Some((id, _)) = self.nodes.slot_get(index) else {
                self.dirty.clear_transform_dirty_at_index(
                    index,
                    DirtyState::FLAG_DIRTY_2D_TRANSFORM | DirtyState::FLAG_DIRTY_3D_TRANSFORM,
                );
                continue;
            };

            if (flags & DirtyState::FLAG_DIRTY_2D_TRANSFORM) != 0 {
                let _ = self.get_global_transform_2d(id);
            }
            if (flags & DirtyState::FLAG_DIRTY_3D_TRANSFORM) != 0 {
                let _ = self.get_global_transform_3d(id);
            }
        }
    }

    fn run_update_schedule(&mut self) {
        let mut i = 0;
        while i < self.schedules.update_slots.len() {
            let (instance_index, id) = self.schedules.update_slots[i];
            self.call_update_script_scheduled(instance_index, id);
            i += 1;
        }
    }

    fn run_fixed_schedule(&mut self) {
        let mut i = 0;
        while i < self.schedules.fixed_slots.len() {
            let (instance_index, id) = self.schedules.fixed_slots[i];
            self.call_fixed_update_script_scheduled(instance_index, id);
            i += 1;
        }
    }

    fn run_start_schedule(&mut self) {
        let mut queued = std::mem::take(&mut self.script_runtime.pending_start_scripts);
        for id in queued.drain(..) {
            let slot = id.index() as usize;
            let still_pending = self.script_runtime.pending_start_flags.get(slot).copied().flatten() == Some(id);
            if !still_pending {
                continue;
            }
            self.script_runtime.pending_start_flags[slot] = None;
            self.call_start_script(id);
        }
        self.script_runtime.pending_start_scripts = queued;
    }

    pub(crate) fn rebuild_internal_node_schedules(&mut self) {
        self.internal_updates.internal_update_nodes.clear();
        self.internal_updates.internal_fixed_update_nodes.clear();
        self.internal_updates.internal_update_pos.clear();
        self.internal_updates.internal_fixed_update_pos.clear();
        let mut pairs = Vec::new();
        for (id, node) in self.nodes.iter() {
            pairs.push((id, node.node_type()));
        }
        for (id, ty) in pairs {
            self.register_internal_node_schedules(id, ty);
        }
    }

    pub(crate) fn register_internal_node_schedules(&mut self, id: NodeID, ty: NodeType) {
        self.register_physics_body(id, ty);
        if matches!(ty.get_internal_update(), InternalUpdate::True) {
            let slot = id.index() as usize;
            if self.internal_updates.internal_update_pos.len() <= slot {
                self.internal_updates.internal_update_pos.resize(slot + 1, None);
            }
            if self.internal_updates.internal_update_pos[slot].is_none() {
                let pos = self.internal_updates.internal_update_nodes.len();
                self.internal_updates.internal_update_nodes.push(id);
                self.internal_updates.internal_update_pos[slot] = Some(pos);
            }
        }
        if matches!(ty.get_internal_fixed_update(), InternalFixedUpdate::True) {
            let slot = id.index() as usize;
            if self.internal_updates.internal_fixed_update_pos.len() <= slot {
                self.internal_updates.internal_fixed_update_pos.resize(slot + 1, None);
            }
            if self.internal_updates.internal_fixed_update_pos[slot].is_none() {
                let pos = self.internal_updates.internal_fixed_update_nodes.len();
                self.internal_updates.internal_fixed_update_nodes.push(id);
                self.internal_updates.internal_fixed_update_pos[slot] = Some(pos);
            }
        }
    }

    pub(crate) fn unregister_internal_node_schedules(&mut self, id: NodeID) {
        self.unregister_physics_body(id);
        let slot = id.index() as usize;

        if let Some(Some(pos)) = self.internal_updates.internal_update_pos.get(slot).copied() {
            let last_pos = self.internal_updates.internal_update_nodes.len().saturating_sub(1);
            self.internal_updates.internal_update_nodes.swap_remove(pos);
            self.internal_updates.internal_update_pos[slot] = None;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self.internal_updates.internal_update_nodes.get(pos).copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.internal_update_pos.len() <= moved_slot {
                    self.internal_updates.internal_update_pos.resize(moved_slot + 1, None);
                }
                self.internal_updates.internal_update_pos[moved_slot] = Some(pos);
            }
        }

        if let Some(Some(pos)) = self.internal_updates.internal_fixed_update_pos.get(slot).copied() {
            let last_pos = self.internal_updates.internal_fixed_update_nodes.len().saturating_sub(1);
            self.internal_updates.internal_fixed_update_nodes.swap_remove(pos);
            self.internal_updates.internal_fixed_update_pos[slot] = None;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self.internal_updates.internal_fixed_update_nodes.get(pos).copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.internal_fixed_update_pos.len() <= moved_slot {
                    self.internal_updates.internal_fixed_update_pos.resize(moved_slot + 1, None);
                }
                self.internal_updates.internal_fixed_update_pos[moved_slot] = Some(pos);
            }
        }
    }

    pub(crate) fn clear_internal_node_schedules(&mut self) {
        self.internal_updates.internal_update_nodes.clear();
        self.internal_updates.internal_fixed_update_nodes.clear();
        self.internal_updates.internal_update_pos.clear();
        self.internal_updates.internal_fixed_update_pos.clear();
        self.internal_updates.physics_body_nodes_2d.clear();
        self.internal_updates.physics_body_nodes_3d.clear();
        self.internal_updates.physics_body_pos_2d.clear();
        self.internal_updates.physics_body_pos_3d.clear();
    }

    fn register_physics_body(&mut self, id: NodeID, ty: NodeType) {
        match ty {
            NodeType::StaticBody2D | NodeType::Area2D | NodeType::RigidBody2D => {
                let slot = id.index() as usize;
                if self.internal_updates.physics_body_pos_2d.len() <= slot {
                    self.internal_updates.physics_body_pos_2d.resize(slot + 1, None);
                }
                if self.internal_updates.physics_body_pos_2d[slot].is_none() {
                    let pos = self.internal_updates.physics_body_nodes_2d.len();
                    self.internal_updates.physics_body_nodes_2d.push(id);
                    self.internal_updates.physics_body_pos_2d[slot] = Some(pos);
                }
            }
            NodeType::StaticBody3D | NodeType::Area3D | NodeType::RigidBody3D => {
                let slot = id.index() as usize;
                if self.internal_updates.physics_body_pos_3d.len() <= slot {
                    self.internal_updates.physics_body_pos_3d.resize(slot + 1, None);
                }
                if self.internal_updates.physics_body_pos_3d[slot].is_none() {
                    let pos = self.internal_updates.physics_body_nodes_3d.len();
                    self.internal_updates.physics_body_nodes_3d.push(id);
                    self.internal_updates.physics_body_pos_3d[slot] = Some(pos);
                }
            }
            _ => {}
        }
    }

    fn unregister_physics_body(&mut self, id: NodeID) {
        let slot = id.index() as usize;

        if let Some(Some(pos)) = self.internal_updates.physics_body_pos_2d.get(slot).copied() {
            let last_pos = self.internal_updates.physics_body_nodes_2d.len().saturating_sub(1);
            self.internal_updates.physics_body_nodes_2d.swap_remove(pos);
            self.internal_updates.physics_body_pos_2d[slot] = None;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self.internal_updates.physics_body_nodes_2d.get(pos).copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.physics_body_pos_2d.len() <= moved_slot {
                    self.internal_updates.physics_body_pos_2d.resize(moved_slot + 1, None);
                }
                self.internal_updates.physics_body_pos_2d[moved_slot] = Some(pos);
            }
        }

        if let Some(Some(pos)) = self.internal_updates.physics_body_pos_3d.get(slot).copied() {
            let last_pos = self.internal_updates.physics_body_nodes_3d.len().saturating_sub(1);
            self.internal_updates.physics_body_nodes_3d.swap_remove(pos);
            self.internal_updates.physics_body_pos_3d[slot] = None;
            if pos <= last_pos.saturating_sub(1)
                && let Some(moved) = self.internal_updates.physics_body_nodes_3d.get(pos).copied()
            {
                let moved_slot = moved.index() as usize;
                if self.internal_updates.physics_body_pos_3d.len() <= moved_slot {
                    self.internal_updates.physics_body_pos_3d.resize(moved_slot + 1, None);
                }
                self.internal_updates.physics_body_pos_3d[moved_slot] = Some(pos);
            }
        }
    }

    pub(crate) fn rebuild_node_tag_index(&mut self) {
        self.node_index.node_tag_index.clear();
        for (id, node) in self.nodes.iter() {
            for &tag in node.tags_slice() {
                self.node_index.node_tag_index.entry(tag).or_default().insert(id);
            }
        }
    }

    fn run_internal_update_schedule(&mut self) {
        let schedule = std::mem::take(&mut self.internal_updates.internal_update_nodes);
        for id in schedule.iter().copied() {
            if self.nodes.get(id).is_none() {
                continue;
            }
            self.call_internal_update_node(id);
        }
        self.internal_updates.internal_update_nodes = schedule;
    }

    fn run_internal_fixed_update_schedule(&mut self) {
        let schedule = std::mem::take(&mut self.internal_updates.internal_fixed_update_nodes);
        for id in schedule.iter().copied() {
            if self.nodes.get(id).is_none() {
                continue;
            }
            self.call_internal_fixed_update_node(id);
        }
        self.internal_updates.internal_fixed_update_nodes = schedule;
    }

    fn call_internal_update_node(&mut self, id: NodeID) {
        if self.nodes.get(id).is_none() {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res: ResourceContext<'_, crate::RuntimeResourceApi> =
            ResourceContext::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputContext<'_, perro_input::InputSnapshot> =
            unsafe { InputContext::new(&*input_ptr) };
        let mut ctx = RuntimeContext::new(self);
        perro_internal_updates::internal_update_node(&mut ctx, &res, &ipt, id);
    }

    fn call_internal_fixed_update_node(&mut self, id: NodeID) {
        if self.nodes.get(id).is_none() {
            return;
        }
        let resource_api = self.resource_api.clone();
        let res: ResourceContext<'_, crate::RuntimeResourceApi> =
            ResourceContext::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: During callback dispatch, input is treated as immutable runtime state.
        // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
        let ipt: InputContext<'_, perro_input::InputSnapshot> =
            unsafe { InputContext::new(&*input_ptr) };
        let mut ctx = RuntimeContext::new(self);
        perro_internal_updates::internal_fixed_update_node(&mut ctx, &res, &ipt, id);
    }

    pub fn clear_dirty_flags(&mut self) {
        self.dirty.clear();
    }

    pub(crate) fn node_local_visible(data: &SceneNodeData) -> bool {
        match data {
            SceneNodeData::Node => true,
            SceneNodeData::Node2D(node) => node.visible,
            SceneNodeData::Sprite2D(node) => node.visible,
            SceneNodeData::Camera2D(node) => node.visible,
            SceneNodeData::CollisionShape2D(node) => node.visible,
            SceneNodeData::StaticBody2D(node) => node.visible,
            SceneNodeData::Area2D(node) => node.visible,
            SceneNodeData::RigidBody2D(node) => node.visible,
            SceneNodeData::Node3D(node) => node.visible,
            SceneNodeData::MeshInstance3D(node) => node.visible,
            SceneNodeData::CollisionShape3D(node) => node.visible,
            SceneNodeData::StaticBody3D(node) => node.visible,
            SceneNodeData::Area3D(node) => node.visible,
            SceneNodeData::RigidBody3D(node) => node.visible,
            SceneNodeData::TerrainInstance3D(node) => node.visible,
            SceneNodeData::Camera3D(node) => node.visible,
            SceneNodeData::AmbientLight3D(node) => node.visible,
            SceneNodeData::RayLight3D(node) => node.visible,
            SceneNodeData::PointLight3D(node) => node.visible,
            SceneNodeData::SpotLight3D(node) => node.visible,
            SceneNodeData::ParticleEmitter3D(node) => node.visible,
            SceneNodeData::Skeleton3D(node) => node.visible,
        }
    }

    pub(crate) fn is_effectively_visible(&self, node: NodeID) -> bool {
        if node.is_nil() {
            return false;
        }
        let mut current = node;
        let mut hops = 0usize;
        let max_hops = self.nodes.len().saturating_add(1);
        while hops < max_hops {
            let Some(scene_node) = self.nodes.get(current) else {
                return false;
            };
            if !Self::node_local_visible(&scene_node.data) {
                return false;
            }
            if scene_node.parent.is_nil() {
                return true;
            }
            current = scene_node.parent;
            hops += 1;
        }
        false
    }

    pub(crate) fn default_terrain_data() -> TerrainData {
        let mut terrain = TerrainData::new(64.0);
        let _ = terrain.ensure_chunk(ChunkCoord::new(0, 0));
        terrain
    }

    pub(crate) fn ensure_terrain_instance_data(&mut self, node: NodeID) -> bool {
        let Some(current_id) = self
            .nodes
            .get(node)
            .and_then(|scene_node| match &scene_node.data {
                SceneNodeData::TerrainInstance3D(terrain) => Some(terrain.terrain),
                _ => None,
            })
        else {
            return false;
        };

        if !current_id.is_nil() {
            let store = self
                .terrain_store
                .lock()
                .expect("terrain store mutex poisoned");
            if store.get(current_id).is_some() {
                return true;
            }
        }

        let id = self
            .terrain_store
            .lock()
            .expect("terrain store mutex poisoned")
            .insert(Self::default_terrain_data());
        if let Some(scene_node) = self.nodes.get_mut(node)
            && let SceneNodeData::TerrainInstance3D(terrain) = &mut scene_node.data
        {
            terrain.terrain = id;
            return true;
        }

        false
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_hotpath_tests.rs"]
mod runtime_hotpath_tests;



