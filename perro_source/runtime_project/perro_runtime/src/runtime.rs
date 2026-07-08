use crate::{
    cns::{NodeArena, ScriptCollection},
    rs_ctx::RuntimeResourceApi,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::AHashMap;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_input_api::InputSnapshot;
use perro_runtime_api::sub_apis::{PreloadedSceneID, WindowRequest};
use perro_scene::Scene;
use perro_scripting::{ScriptAPI, ScriptBehavior, ScriptConstructor};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use std::{cell::RefCell, sync::Arc};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

const STARTUP_INPUT_CLEAR_FRAMES: u32 = 100;

// Runtime subsystem leaves. Public API glue stays here; heavy behavior lives in folders.
mod audio;
mod input_bridge;
mod internal_updates;
mod mesh_query;
mod physics;
#[path = "runtime/render/two_d.rs"]
mod render_2d;
#[path = "runtime/render/three_d.rs"]
mod render_3d;
#[path = "runtime/render/bridge.rs"]
mod render_bridge;
#[path = "runtime/render/ui.rs"]
mod render_ui;
mod scene_loader;
mod scheduling;
pub(crate) mod state;
mod transforms;
mod world_state;

use audio::AudioPropagationState;
pub(crate) use scene_loader::PendingScriptAttach;
#[cfg(feature = "bench")]
pub use scene_loader::{
    bench_prepare_and_merge_scene, bench_prepare_merge_extract_scene, bench_prepare_scene,
};
pub(crate) use state::CollisionDebugState;
pub(crate) use state::ScriptCallbackContext;
use state::{
    DirtyState, InternalUpdateState, NodeApiScratchState, NodeIndexState, Render2DState,
    Render3DState, RenderState, RenderUiState, ScriptRuntimeState, ScriptSchedules,
    SignalRuntimeState, TransformRuntimeState,
};

pub struct RuntimeScriptApi;
impl ScriptAPI for RuntimeScriptApi {
    type RT = Runtime;
    type RS = RuntimeResourceApi;
    type IP = InputSnapshot;
}
type RuntimeScriptCtor = ScriptConstructor<RuntimeScriptApi>;
type RuntimeScriptBehavior = dyn ScriptBehavior<RuntimeScriptApi>;
type StaticScriptRegistry = &'static [(u64, RuntimeScriptCtor)];

#[derive(Clone, Copy, Debug)]
pub(crate) struct ForceWaterImpact2D {
    pub(crate) position: perro_structs::Vector2,
    pub(crate) force: perro_structs::Vector2,
    pub(crate) strength: f32,
    pub(crate) radius: f32,
    pub(crate) cavitation: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ForceWaterImpact3D {
    pub(crate) position: perro_structs::Vector3,
    pub(crate) force: perro_structs::Vector3,
    pub(crate) strength: f32,
    pub(crate) radius: f32,
    pub(crate) cavitation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WaterBodySampleKey {
    pub(crate) water: NodeID,
    pub(crate) body: NodeID,
    pub(crate) point: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WaterBodySampleCache {
    pub(crate) local: perro_structs::Vector2,
    pub(crate) height: f32,
    pub(crate) velocity: perro_structs::Vector2,
    pub(crate) foam: f32,
    pub(crate) sample_time: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PendingWaterQuery {
    pub(crate) body: NodeID,
    pub(crate) point: u8,
    pub(crate) local: perro_structs::Vector2,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WaterBodyContact2D {
    pub(crate) position: perro_structs::Vector2,
    pub(crate) velocity: perro_structs::Vector2,
    pub(crate) radius: f32,
    pub(crate) foam_amount: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WaterBodyContact3D {
    pub(crate) position: perro_structs::Vector3,
    pub(crate) velocity: perro_structs::Vector3,
    pub(crate) radius: f32,
    pub(crate) foam_amount: f32,
}

/// Reusable scratch maps 4 scene resource-ref scan.
///
/// Kept on runtime so the per-frame scan reuse allocs instead of building +
/// cloning fresh maps each drain.
#[derive(Default)]
pub(crate) struct SceneResourceRefsScratch {
    pub(crate) textures: AHashMap<TextureID, Vec<NodeID>>,
    pub(crate) meshes: AHashMap<MeshID, Vec<NodeID>>,
    pub(crate) materials: AHashMap<MaterialID, Vec<NodeID>>,
}

/// Live game runtime state.
///
/// Keeps scene nodes, script schedules, resource APIs, input snapshots,
/// physics state, audio propagation, and retained render state in one owner.
pub struct Runtime {
    pub time: Timing,
    provider_mode: ProviderMode,
    project: Option<Arc<RuntimeProject>>,
    pub(crate) active_route_href: Option<String>,
    pub(crate) active_route_root: Option<NodeID>,
    pub(crate) scene_cache: RefCell<AHashMap<String, Arc<Scene>>>,
    pub(crate) preloaded_scenes: AHashMap<PreloadedSceneID, Arc<Scene>>,
    pub(crate) preloaded_scene_paths: AHashMap<u64, PreloadedSceneID>,
    pub(crate) preloaded_scene_reverse_paths: AHashMap<PreloadedSceneID, String>,
    pub(crate) next_preloaded_scene_id: u64,

    pub nodes: NodeArena,
    pub(crate) scripts: ScriptCollection,
    schedules: ScriptSchedules,
    pub(crate) script_runtime: ScriptRuntimeState,
    render: RenderState,
    scene_texture_refs_cache: AHashMap<TextureID, Vec<NodeID>>,
    scene_mesh_refs_cache: AHashMap<MeshID, Vec<NodeID>>,
    scene_material_refs_cache: AHashMap<MaterialID, Vec<NodeID>>,
    /// last arena mutation_revision seen by resource-ref scan. gate re-scan.
    scene_resource_refs_scanned_version: u64,
    /// force resource-ref re-scan next drain. set on resource render events
    /// (pending resolve / retained invalidation) that arena version misses.
    scene_resource_refs_dirty: bool,
    /// reusable scratch maps 4 resource-ref scan; avoid per-frame alloc + clone.
    scene_resource_refs_scratch: SceneResourceRefsScratch,
    /// reusable node-id list 4 camera-stream collectors; refill once per drain.
    camera_stream_node_scratch: Vec<NodeID>,
    pub(crate) dirty: DirtyState,
    pub(crate) transforms: TransformRuntimeState,
    internal_updates: InternalUpdateState,

    render_2d: Render2DState,
    render_3d: Render3DState,
    render_ui: RenderUiState,
    locale_text: state::LocaleTextState,
    pub(crate) signal_runtime: SignalRuntimeState,
    pub(crate) node_index: NodeIndexState,
    pub(crate) node_api_scratch: NodeApiScratchState,
    pub(crate) resource_api: Arc<RuntimeResourceApi>,
    pub(crate) input: InputSnapshot,
    startup_input_clear_frames_left: u32,
    cursor_icon_request: Option<perro_ui::CursorIcon>,
    pub(crate) window_requests: Vec<WindowRequest>,
    pub(crate) active_refresh_rate: Option<f32>,
    pub(crate) physics_gravity_override: Option<f32>,
    pub(crate) physics_coef_override: Option<f32>,
    physics: physics::PhysicsState,
    /// arena mutation revision @ last node->world sync; match + no dirty => skip re-sync
    physics_synced_node_revision_2d: Option<u64>,
    physics_synced_node_revision_3d: Option<u64>,
    physics_body_descs_2d: Vec<perro_physics::BodyDesc2D>,
    physics_body_descs_3d: Vec<perro_physics::BodyDesc3D>,
    physics_joint_descs_2d: Vec<perro_physics::JointDesc2D>,
    physics_joint_descs_3d: Vec<perro_physics::JointDesc3D>,
    /// internal fall speed per char body 4 script-invoked apply_gravity;
    /// ! exposed on node (char body has no velocity state)
    character_fall_speed_2d: AHashMap<NodeID, f32>,
    character_fall_speed_3d: AHashMap<NodeID, f32>,
    /// last sweep hit per char body (node, point, normal); merged -> contacts_*
    /// cuz kinematic-vs-fixed pairs never activate in solver narrow phase
    character_sweep_hit_2d:
        AHashMap<NodeID, (NodeID, perro_structs::Vector2, perro_structs::Vector2)>,
    character_sweep_hit_3d:
        AHashMap<NodeID, (NodeID, perro_structs::Vector3, perro_structs::Vector3)>,
    water_samples: AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_sample_times: AHashMap<NodeID, f32>,
    water_body_samples: AHashMap<WaterBodySampleKey, WaterBodySampleCache>,
    pending_water_queries_2d: AHashMap<NodeID, Vec<PendingWaterQuery>>,
    pending_water_queries_3d: AHashMap<NodeID, Vec<PendingWaterQuery>>,
    water_contacts_2d: AHashMap<NodeID, Vec<WaterBodyContact2D>>,
    water_contacts_3d: AHashMap<NodeID, Vec<WaterBodyContact3D>>,
    water_rigid_body_ids_2d_cache: Vec<NodeID>,
    water_rigid_body_ids_3d_cache: Vec<NodeID>,
    water_ids_2d_cache: Vec<NodeID>,
    water_ids_3d_cache: Vec<NodeID>,
    /// `nodes.physics_revision()` snapshot @ last fill of each cache above.
    /// `None` means unfilled. Lets empty-result scenes cache too (`is_empty`
    /// used 2 be the unfilled sentinel, so 0-water scenes rescanned forever).
    water_rigid_body_ids_2d_cache_version: Option<u64>,
    water_rigid_body_ids_3d_cache_version: Option<u64>,
    water_ids_2d_cache_version: Option<u64>,
    water_ids_3d_cache_version: Option<u64>,
    pending_skeleton_sources_2d: AHashMap<NodeID, String>,
    pending_skeleton_sources_3d: AHashMap<NodeID, String>,
    pub(crate) force_water_impacts_2d: Vec<ForceWaterImpact2D>,
    pub(crate) force_water_impacts_3d: Vec<ForceWaterImpact3D>,
    pub(crate) pending_force_emitters_2d: Vec<perro_nodes::PhysicsForceEmitter2D>,
    pub(crate) pending_force_emitters_3d: Vec<perro_nodes::PhysicsForceEmitter3D>,
    /// reusable body-handle update buf 4 sync_world_2d/3d; avoid per-frame alloc.
    physics_handle_updates_scratch_2d: Vec<(NodeID, Option<u64>)>,
    physics_handle_updates_scratch_3d: Vec<(NodeID, Option<u64>)>,
    /// reusable staged-pose buf 4 sync_world_to_nodes_2d/3d writeback.
    physics_writeback_scratch_2d: Vec<physics::StagedBodyPose2D>,
    physics_writeback_scratch_3d: Vec<physics::StagedBodyPose3D>,
    /// reusable force-emitter collect buf 4 queue_physics_force_emitters_2d/3d.
    physics_force_emitters_scratch_2d:
        Vec<(perro_structs::Vector2, perro_nodes::PhysicsForceEmitter2D)>,
    physics_force_emitters_scratch_3d:
        Vec<(perro_structs::Vector3, perro_nodes::PhysicsForceEmitter3D)>,
    /// reusable emitter-id scan buf 4 queue_physics_force_emitters_2d/3d;
    /// avoid per-step alloc on the type-lane scan result.
    physics_force_emitter_ids_scratch_2d: Vec<NodeID>,
    physics_force_emitter_ids_scratch_3d: Vec<NodeID>,
    /// reusable water-index input buf 4 queue_water_forces_2d/3d.
    physics_waters_scratch_2d: Vec<physics::RuntimeWater2D>,
    physics_waters_scratch_3d: Vec<physics::RuntimeWater3D>,
    /// reusable subtree-walk stack 4 force_rerender; avoid per-node
    /// children_slice().to_vec() alloc on every visited node.
    force_rerender_stack_scratch: Vec<NodeID>,
    pub(crate) audio: AudioPropagationState,
    /// Per-node cache 4 mesh point/ray/region queries; avoids re-cloning
    /// surfaces + rebuilding per-instance Mat4s (MultiMeshInstance3D) on
    /// every query. Keyed by NodeID (generation-safe on slot reuse) +
    /// validated against `nodes.mutation_revision()` @ build time.
    mesh_query_node_cache: mesh_query::QueryNodeDataCache,
    /// test/bench probe: # of QueryNodeData rebuilds (cache misses). proves
    /// repeated queries on an unchanged node hit the cache.
    #[cfg(any(test, feature = "bench"))]
    pub(crate) mesh_query_node_rebuilds: std::cell::Cell<u64>,
    /// test/bench probe: # of collect_body_descs_2d/3d calls. proves the
    /// physics-scoped dirty gate skip collect 4 non-physics node moves.
    #[cfg(any(test, feature = "bench"))]
    pub(crate) physics_collect_calls_2d: std::cell::Cell<u64>,
    #[cfg(any(test, feature = "bench"))]
    pub(crate) physics_collect_calls_3d: std::cell::Cell<u64>,
}

pub struct Timing {
    /// Fixed-step delta passed to physics and fixed scripts.
    pub fixed_delta: f32,
    /// Variable-step delta passed to frame scripts.
    pub delta: f32,
    /// Accumulated runtime time in seconds.
    pub elapsed: f32,
    /// Last measured simulation time.
    pub simulation: Duration,
    /// Last measured graphics time.
    pub graphics: Duration,
    /// Last measured frame time.
    pub frame: Duration,
    /// Frames per second averaged over the runner's fps window (~0.5s), not a
    /// single-frame reciprocal.
    pub fps: f32,
    /// Last measured 3D gpu prepare total.
    pub draw_gpu_prepare_3d: Duration,
    /// Last measured 3D frustum prepare.
    pub draw_gpu_prepare_3d_frustum: Duration,
    /// Last measured 3D hiz prepare.
    pub draw_gpu_prepare_3d_hiz: Duration,
    /// Last measured 3D indirect prepare.
    pub draw_gpu_prepare_3d_indirect: Duration,
    /// Last measured 3D cull input prepare.
    pub draw_gpu_prepare_3d_cull_inputs: Duration,
    /// Last measured 2D draw calls.
    pub draw_calls_2d: u32,
    /// Last measured 3D draw calls.
    pub draw_calls_3d: u32,
    /// Last measured total draw calls.
    pub draw_calls_total: u32,
    /// Last measured 2D sprite batches.
    pub sprite_batches_2d: u32,
    /// Last measured 2D sprite texture bind switches.
    pub sprite_bind_group_switches_2d: u32,
    /// Last measured 3D draw batches.
    pub draw_batches_3d: u32,
    /// Last measured 3D pipeline switches.
    pub pipeline_switches_3d: u32,
    /// Last measured 3D material texture bind switches.
    pub texture_bind_group_switches_3d: u32,
    /// Last measured 3D instances.
    pub draw_instances_3d: u32,
    /// Last measured 3D material refs.
    pub draw_material_refs_3d: u32,
    /// Last measured 3D prepare skips.
    pub skip_prepare_3d: u32,
    /// Last measured frustum prepare skips.
    pub skip_prepare_3d_frustum: u32,
    /// Last measured hiz prepare skips.
    pub skip_prepare_3d_hiz: u32,
    /// Last measured indirect prepare skips.
    pub skip_prepare_3d_indirect: u32,
    /// Last measured cull input prepare skips.
    pub skip_prepare_3d_cull_inputs: u32,
}

/// Timing breakdown for variable-step script schedules.
#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateScheduleTiming {
    pub total: Duration,
    pub scripts_total: Duration,
    pub script_count: u32,
    pub slowest_script_id: Option<NodeID>,
    pub slowest_script: Duration,
}

/// Timing breakdown for one variable runtime update.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeUpdateTiming {
    pub start_schedule: Duration,
    pub snapshot_update: Duration,
    pub update_schedule: UpdateScheduleTiming,
    pub internal_update: Duration,
    pub total: Duration,
}

/// Timing breakdown for one fixed runtime update.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeFixedUpdateTiming {
    pub snapshot_update: Duration,
    pub script_fixed_update: Duration,
    pub physics: Duration,
    pub physics_pre_transforms: Duration,
    pub physics_collect: Duration,
    pub physics_sync_world: Duration,
    pub physics_apply_forces_impulses: Duration,
    pub physics_step: Duration,
    pub physics_sync_nodes: Duration,
    pub physics_post_transforms: Duration,
    pub physics_signals: Duration,
    pub internal_fixed_update: Duration,
    pub total: Duration,
}

/// Timing breakdown for retained UI extraction.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeUiTiming {
    pub layout: Duration,
    pub commands: Duration,
    pub total: Duration,
    pub dirty_nodes: u32,
    pub affected_nodes: u32,
    pub recalculated_rects: u32,
    pub cached_rects: u32,
    pub auto_layout_batches: u32,
    pub command_nodes: u32,
    pub command_emitted: u32,
    pub command_skipped: u32,
    pub removed_nodes: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuntimePhysicsStepTiming {
    pub pre_transforms: Duration,
    pub collect: Duration,
    pub sync_world: Duration,
    pub apply_forces_impulses: Duration,
    pub step: Duration,
    pub sync_nodes: Duration,
    pub post_transforms: Duration,
    pub signals: Duration,
    pub total: Duration,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            time: Timing {
                fixed_delta: 0.0,
                delta: 0.0,
                elapsed: 0.0,
                simulation: Duration::ZERO,
                graphics: Duration::ZERO,
                frame: Duration::ZERO,
                fps: 0.0,
                draw_gpu_prepare_3d: Duration::ZERO,
                draw_gpu_prepare_3d_frustum: Duration::ZERO,
                draw_gpu_prepare_3d_hiz: Duration::ZERO,
                draw_gpu_prepare_3d_indirect: Duration::ZERO,
                draw_gpu_prepare_3d_cull_inputs: Duration::ZERO,
                draw_calls_2d: 0,
                draw_calls_3d: 0,
                draw_calls_total: 0,
                sprite_batches_2d: 0,
                sprite_bind_group_switches_2d: 0,
                draw_batches_3d: 0,
                pipeline_switches_3d: 0,
                texture_bind_group_switches_3d: 0,
                draw_instances_3d: 0,
                draw_material_refs_3d: 0,
                skip_prepare_3d: 0,
                skip_prepare_3d_frustum: 0,
                skip_prepare_3d_hiz: 0,
                skip_prepare_3d_indirect: 0,
                skip_prepare_3d_cull_inputs: 0,
            },
            provider_mode: ProviderMode::Dynamic,
            active_route_href: None,
            active_route_root: None,
            scene_cache: RefCell::new(AHashMap::new()),
            preloaded_scenes: AHashMap::new(),
            preloaded_scene_paths: AHashMap::new(),
            preloaded_scene_reverse_paths: AHashMap::new(),
            next_preloaded_scene_id: 1,
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            schedules: ScriptSchedules::new(),
            script_runtime: ScriptRuntimeState::new(),
            project: None,
            render: RenderState::new(),
            scene_texture_refs_cache: AHashMap::new(),
            scene_mesh_refs_cache: AHashMap::new(),
            scene_material_refs_cache: AHashMap::new(),
            scene_resource_refs_scanned_version: u64::MAX,
            scene_resource_refs_dirty: true,
            scene_resource_refs_scratch: SceneResourceRefsScratch::default(),
            camera_stream_node_scratch: Vec::new(),
            dirty: DirtyState::new(),
            transforms: TransformRuntimeState::new(),
            internal_updates: InternalUpdateState::new(),
            render_2d: Render2DState::new(),
            render_3d: Render3DState::new(),
            render_ui: RenderUiState::new(),
            locale_text: state::LocaleTextState::new(),
            signal_runtime: SignalRuntimeState::new(),
            node_index: NodeIndexState::new(),
            node_api_scratch: NodeApiScratchState::new(),
            resource_api: RuntimeResourceApi::new(None, None, None, None, None, None, None, None),
            input: InputSnapshot::new(),
            startup_input_clear_frames_left: 0,
            cursor_icon_request: None,
            window_requests: Vec::new(),
            active_refresh_rate: None,
            physics_gravity_override: None,
            physics_coef_override: None,
            physics: physics::PhysicsState::new(),
            physics_synced_node_revision_2d: None,
            physics_synced_node_revision_3d: None,
            physics_body_descs_2d: Vec::new(),
            physics_body_descs_3d: Vec::new(),
            physics_joint_descs_2d: Vec::new(),
            physics_joint_descs_3d: Vec::new(),
            character_fall_speed_2d: AHashMap::new(),
            character_fall_speed_3d: AHashMap::new(),
            character_sweep_hit_2d: AHashMap::new(),
            character_sweep_hit_3d: AHashMap::new(),
            water_samples: AHashMap::new(),
            water_sample_times: AHashMap::new(),
            water_body_samples: AHashMap::new(),
            pending_water_queries_2d: AHashMap::new(),
            pending_water_queries_3d: AHashMap::new(),
            water_contacts_2d: AHashMap::new(),
            water_contacts_3d: AHashMap::new(),
            water_rigid_body_ids_2d_cache: Vec::new(),
            water_rigid_body_ids_3d_cache: Vec::new(),
            water_ids_2d_cache: Vec::new(),
            water_ids_3d_cache: Vec::new(),
            water_rigid_body_ids_2d_cache_version: None,
            water_rigid_body_ids_3d_cache_version: None,
            water_ids_2d_cache_version: None,
            water_ids_3d_cache_version: None,
            pending_skeleton_sources_2d: AHashMap::new(),
            pending_skeleton_sources_3d: AHashMap::new(),
            force_water_impacts_2d: Vec::new(),
            force_water_impacts_3d: Vec::new(),
            pending_force_emitters_2d: Vec::new(),
            pending_force_emitters_3d: Vec::new(),
            physics_handle_updates_scratch_2d: Vec::new(),
            physics_handle_updates_scratch_3d: Vec::new(),
            physics_writeback_scratch_2d: Vec::new(),
            physics_writeback_scratch_3d: Vec::new(),
            physics_force_emitters_scratch_2d: Vec::new(),
            physics_force_emitters_scratch_3d: Vec::new(),
            physics_force_emitter_ids_scratch_2d: Vec::new(),
            physics_force_emitter_ids_scratch_3d: Vec::new(),
            physics_waters_scratch_2d: Vec::new(),
            physics_waters_scratch_3d: Vec::new(),
            force_rerender_stack_scratch: Vec::new(),
            audio: AudioPropagationState::new(),
            mesh_query_node_cache: AHashMap::default(),
            #[cfg(any(test, feature = "bench"))]
            mesh_query_node_rebuilds: std::cell::Cell::new(0),
            #[cfg(any(test, feature = "bench"))]
            physics_collect_calls_2d: std::cell::Cell::new(0),
            #[cfg(any(test, feature = "bench"))]
            physics_collect_calls_3d: std::cell::Cell::new(0),
        }
    }

    #[cfg(feature = "bench")]
    pub fn bench_create_mesh_data(&self, data: perro_render_bridge::Mesh3D) -> perro_ids::MeshID {
        use perro_resource_api::sub_apis::MeshAPI;

        self.resource_api.create_mesh_data(data)
    }

    /// Wires a node's mesh-query source path (normally set by the scene
    /// loader on mesh assignment). Lets benches exercise
    /// `mesh_instance_surface_*` node queries without a full scene load.
    #[cfg(feature = "bench")]
    pub fn bench_set_mesh_source(&mut self, node_id: NodeID, source: &str) {
        self.render_3d
            .mesh_sources
            .insert(node_id, source.to_string());
    }

    #[cfg(feature = "bench")]
    pub fn bench_with_script_context<R>(
        &mut self,
        id: NodeID,
        f: impl FnOnce(&mut perro_scripting::ScriptContext<'_, RuntimeScriptApi>) -> R,
    ) -> R {
        let resource_api = self.resource_api.clone();
        let res = perro_resource_api::ResourceWindow::new(resource_api.as_ref());
        let input_ptr = std::ptr::addr_of!(self.input);
        // SAFETY: Bench callback mirrors runtime script dispatch. Input stays immutable for call.
        let ipt = unsafe { perro_input_api::InputWindow::new(&*input_ptr) };
        let mut run = perro_runtime_api::RuntimeWindow::new(self);
        let mut ctx = perro_scripting::ScriptContext {
            run: &mut run,
            res: &res,
            ipt: &ipt,
            id,
        };
        f(&mut ctx)
    }

    pub fn from_project(project: RuntimeProject, provider_mode: ProviderMode) -> Self {
        Self::from_project_with_script_registry(project, provider_mode, None)
    }

    #[inline]
    pub(crate) fn reset_water_scan_cache_2d(&mut self) {
        self.water_rigid_body_ids_2d_cache.clear();
        self.water_ids_2d_cache.clear();
        self.water_rigid_body_ids_2d_cache_version = None;
        self.water_ids_2d_cache_version = None;
    }

    #[inline]
    pub(crate) fn reset_water_scan_cache_3d(&mut self) {
        self.water_rigid_body_ids_3d_cache.clear();
        self.water_ids_3d_cache.clear();
        self.water_rigid_body_ids_3d_cache_version = None;
        self.water_ids_3d_cache_version = None;
    }

    pub(crate) fn cached_rigid_body_ids_2d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.water_rigid_body_ids_2d_cache_version != Some(version) {
            self.water_rigid_body_ids_2d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::RigidBody2D,
                |node| matches!(&node.data, perro_nodes::SceneNodeData::RigidBody2D(body) if body.enabled),
                &mut self.water_rigid_body_ids_2d_cache,
            );
            self.water_rigid_body_ids_2d_cache_version = Some(version);
        }
        &self.water_rigid_body_ids_2d_cache
    }

    pub(crate) fn cached_rigid_body_ids_3d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.water_rigid_body_ids_3d_cache_version != Some(version) {
            self.water_rigid_body_ids_3d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::RigidBody3D,
                |node| matches!(&node.data, perro_nodes::SceneNodeData::RigidBody3D(body) if body.enabled),
                &mut self.water_rigid_body_ids_3d_cache,
            );
            self.water_rigid_body_ids_3d_cache_version = Some(version);
        }
        &self.water_rigid_body_ids_3d_cache
    }

    pub(crate) fn cached_water_ids_2d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.water_ids_2d_cache_version != Some(version) {
            self.water_ids_2d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::WaterBody2D,
                |node| matches!(node.data, perro_nodes::SceneNodeData::WaterBody2D(_)),
                &mut self.water_ids_2d_cache,
            );
            self.water_ids_2d_cache_version = Some(version);
        }
        &self.water_ids_2d_cache
    }

    pub(crate) fn cached_water_ids_3d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.water_ids_3d_cache_version != Some(version) {
            self.water_ids_3d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::WaterBody3D,
                |node| matches!(node.data, perro_nodes::SceneNodeData::WaterBody3D(_)),
                &mut self.water_ids_3d_cache,
            );
            self.water_ids_3d_cache_version = Some(version);
        }
        &self.water_ids_3d_cache
    }

    pub(crate) fn apply_loaded_skeleton_bones(&mut self) {
        self.resource_api.poll_skeleton_bone_loads();
        let mut changed_2d = Vec::new();
        for (node, source) in &self.pending_skeleton_sources_2d {
            if let Some(bones) = self.resource_api.cached_bones_2d(source)
                && let Some(scene_node) = self.nodes.get_mut(*node)
                && let perro_nodes::SceneNodeData::Skeleton2D(skeleton) = &mut scene_node.data
            {
                skeleton.bones = bones;
                changed_2d.push(*node);
            }
        }
        for node in &changed_2d {
            self.pending_skeleton_sources_2d.remove(node);
            self.mark_transform_dirty_recursive(*node);
        }

        let mut changed_3d = Vec::new();
        for (node, source) in &self.pending_skeleton_sources_3d {
            if let Some(bones) = self.resource_api.cached_bones_3d(source)
                && let Some(scene_node) = self.nodes.get_mut(*node)
                && let perro_nodes::SceneNodeData::Skeleton3D(skeleton) = &mut scene_node.data
            {
                skeleton.bones = bones;
                skeleton.refresh_inv_bind_cache();
                changed_3d.push(*node);
            }
        }
        for node in &changed_3d {
            self.pending_skeleton_sources_3d.remove(node);
            self.mark_transform_dirty_recursive(*node);
        }

        if !changed_2d.is_empty() {
            self.render_2d.request_full_scan_once();
        }
        if !changed_3d.is_empty() {
            self.render_3d.request_full_scan_once();
        }
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
        let static_animation_lookup = project.static_animation_lookup;
        let static_animation_tree_lookup = project.static_animation_tree_lookup;
        let static_localization_lookup = project.static_localization_lookup;
        let static_csv_lookup = project.static_csv_lookup;
        let localization_config = project.config.localization.clone();
        let input_map = project.config.input_map.clone();
        #[cfg(feature = "steamworks")]
        let steam_config = project.config.steam.clone();
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = provider_mode;
        runtime.startup_input_clear_frames_left = STARTUP_INPUT_CLEAR_FRAMES;
        runtime.resource_api = RuntimeResourceApi::new(
            static_material_lookup,
            static_audio_lookup,
            static_skeleton_lookup,
            static_animation_lookup,
            static_animation_tree_lookup,
            static_localization_lookup,
            static_csv_lookup,
            localization_config,
        );
        runtime.configure_audio_from_project();
        runtime.input.set_input_map(input_map);
        if let Some(entries) = script_registry {
            for (path_hash, ctor) in entries {
                runtime
                    .script_runtime
                    .dynamic_script_registry
                    .insert(*path_hash, *ctor);
            }
        }
        #[cfg(feature = "steamworks")]
        if let Err(err) = perro_steamworks::runtime::init_from_config_with_input(
            steam_config.enabled,
            steam_config.app_id,
            map_steam_input_mode(steam_config.input_mode),
        ) {
            eprintln!(
                "[runtime][warn] Steam enabled but init failed: {err}. Steam features stay unavailable. Check that Steam is open, the app_id is valid, and the account has access."
            );
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
        self.clear_startup_keyboard_mouse();
        self.time.delta = delta_time;
        self.flush_queued_ui_signals();
        self.process_pending_web_route_change();
        self.apply_loaded_skeleton_bones();
        self.run_start_schedule();
        self.schedules.snapshot_update(&self.scripts);
        self.run_update_schedule();
        #[cfg(feature = "steamworks")]
        let _ = perro_steamworks::runtime::run_callbacks();
        self.run_internal_update_schedule();
        self.propagate_pending_transform_dirty();
        self.update_audio_propagation(delta_time);
    }

    #[inline]
    pub fn update_timed(&mut self, delta_time: f32) -> RuntimeUpdateTiming {
        let total_start = Instant::now();
        self.clear_startup_keyboard_mouse();
        self.time.delta = delta_time;
        self.flush_queued_ui_signals();
        self.process_pending_web_route_change();
        self.apply_loaded_skeleton_bones();

        let start_schedule_start = Instant::now();
        self.run_start_schedule();
        let start_schedule = start_schedule_start.elapsed();

        let snapshot_start = Instant::now();
        self.schedules.snapshot_update(&self.scripts);
        let snapshot_update = snapshot_start.elapsed();

        let update_schedule = self.run_update_schedule_timed();

        #[cfg(feature = "steamworks")]
        let _ = perro_steamworks::runtime::run_callbacks();

        let internal_start = Instant::now();
        self.run_internal_update_schedule();
        let internal_update = internal_start.elapsed();
        self.propagate_pending_transform_dirty();
        self.update_audio_propagation(delta_time);

        RuntimeUpdateTiming {
            start_schedule,
            snapshot_update,
            update_schedule,
            internal_update,
            total: total_start.elapsed(),
        }
    }

    #[inline]
    pub fn fixed_update(&mut self, fixed_delta_time: f32) {
        self.clear_startup_keyboard_mouse();
        self.time.fixed_delta = fixed_delta_time;
        self.schedules.snapshot_fixed(&self.scripts);
        self.run_fixed_schedule();
        self.physics_fixed_step();
        self.run_internal_fixed_update_schedule();
        self.propagate_pending_transform_dirty();
    }

    #[inline]
    pub fn fixed_update_timed(&mut self, fixed_delta_time: f32) -> RuntimeFixedUpdateTiming {
        let total_start = Instant::now();
        self.clear_startup_keyboard_mouse();
        self.time.fixed_delta = fixed_delta_time;

        let snapshot_start = Instant::now();
        self.schedules.snapshot_fixed(&self.scripts);
        let snapshot_update = snapshot_start.elapsed();

        let script_fixed_start = Instant::now();
        self.run_fixed_schedule();
        let script_fixed_update = script_fixed_start.elapsed();

        let physics_timing = self.physics_fixed_step_timed();

        let internal_fixed_start = Instant::now();
        self.run_internal_fixed_update_schedule();
        let internal_fixed_update = internal_fixed_start.elapsed();
        self.propagate_pending_transform_dirty();

        RuntimeFixedUpdateTiming {
            snapshot_update,
            script_fixed_update,
            physics: physics_timing.total,
            physics_pre_transforms: physics_timing.pre_transforms,
            physics_collect: physics_timing.collect,
            physics_sync_world: physics_timing.sync_world,
            physics_apply_forces_impulses: physics_timing.apply_forces_impulses,
            physics_step: physics_timing.step,
            physics_sync_nodes: physics_timing.sync_nodes,
            physics_post_transforms: physics_timing.post_transforms,
            physics_signals: physics_timing.signals,
            internal_fixed_update,
            total: total_start.elapsed(),
        }
    }
}

#[cfg(feature = "steamworks")]
fn map_steam_input_mode(
    mode: perro_project::SteamInputMode,
) -> perro_steamworks::input::SteamInputMode {
    match mode {
        perro_project::SteamInputMode::Off => perro_steamworks::input::SteamInputMode::Off,
        perro_project::SteamInputMode::Metadata => {
            perro_steamworks::input::SteamInputMode::Metadata
        }
        perro_project::SteamInputMode::Actions => perro_steamworks::input::SteamInputMode::Actions,
    }
}

/// Cheap slot-lane occupancy+type scan. Type mirror lane only meaningful on
/// occupied slots (see `NodeArena::node_type_slots`), so pair every read w/
/// `slot_get` before trusting the payload match.
fn scan_node_type_slots(
    arena: &NodeArena,
    want: perro_nodes::NodeType,
    keep: impl Fn(&perro_nodes::SceneNode) -> bool,
    out: &mut Vec<NodeID>,
) {
    let types = arena.node_type_slots();
    for (index, node_type) in types.iter().enumerate() {
        if *node_type != want {
            continue;
        }
        let Some((id, node)) = arena.slot_get(index) else {
            continue;
        };
        if keep(node) {
            out.push(id);
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let mut script_ids = Vec::new();
        self.scripts.append_instance_ids(&mut script_ids);
        for id in script_ids {
            let _ = self.remove_script_instance(id);
        }

        #[cfg(feature = "steamworks")]
        let _ = perro_steamworks::runtime::run_callbacks();
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_hotpath_tests.rs"]
mod runtime_hotpath_tests;

#[cfg(test)]
#[path = "../tests/unit/rt_ctx_node_mut_dirty_tests.rs"]
mod rt_ctx_node_mut_dirty_tests;

#[cfg(test)]
#[path = "../tests/unit/dirty_state_transform_count_tests.rs"]
mod dirty_state_transform_count_tests;
