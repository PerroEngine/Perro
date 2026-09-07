use crate::{
    cns::{NodeArena, ScriptCollection},
    rs_ctx::RuntimeResourceApi,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::{AHashMap, AHashSet};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_input_api::InputSnapshot;
use perro_runtime_api::sub_apis::{PreloadedSceneID, WindowRequest};
use perro_scene::Scene;
use perro_scripting::{DynamicScriptConstructor, ScriptAPI, ScriptBehavior, ScriptConstructor};
use std::time::Duration;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const STARTUP_INPUT_CLEAR_FRAMES: u32 = 100;

// Runtime subsystem leaves. Public API glue stays here; heavy behavior lives in folders.
mod audio;
#[path = "runtime/render/state.rs"]
mod extraction_state;
mod input_bridge;
mod internal_updates;
mod mesh_query;
pub(crate) mod navmesh;
mod phases;
mod physics;
#[path = "runtime/physics/state.rs"]
mod physics_sync_state;
#[path = "runtime/render/two_d.rs"]
mod render_2d;
#[path = "runtime/render/three_d.rs"]
mod render_3d;
#[path = "runtime/render/bridge.rs"]
mod render_bridge;
#[path = "runtime/render/ui.rs"]
mod render_ui;
mod scene_loader;
#[path = "runtime/scene_loader/state.rs"]
mod scene_state;
mod scheduling;
pub(crate) mod state;
mod timers;
mod transforms;
mod world_state;

use audio::AudioPropagationState;
use extraction_state::ExtractionState;
use physics_sync_state::PhysicsSyncState;
pub(crate) use scene_loader::PendingScriptAttach;
#[cfg(feature = "bench")]
pub use scene_loader::{
    BenchPreparedScene, BenchSceneSpawner, bench_compile_scene, bench_merge_compiled_scene,
    bench_prepare_and_merge_scene, bench_prepare_merge_extract_scene, bench_prepare_scene,
};
use scene_state::SceneRuntimeState;
pub(crate) use state::CollisionDebugState;
pub(crate) use state::ScriptCallbackContext;
use state::{
    DirtyState, InternalUpdateState, NodeApiScratchState, NodeIndexState, Render2DState,
    Render3DState, RenderState, RenderUiState, ScriptRuntimeState, ScriptSchedules,
    SignalRuntimeState, TransformRuntimeState,
};
use timers::TimerRuntimeState;

pub struct RuntimeScriptApi;
impl ScriptAPI for RuntimeScriptApi {
    type RT = Runtime;
    type RS = RuntimeResourceApi;
    type IP = InputSnapshot;
}
pub(crate) type RuntimeScriptBehavior = dyn ScriptBehavior<RuntimeScriptApi>;
type StaticScriptRegistry = &'static [(u64, ScriptConstructor<RuntimeScriptApi>)];

#[derive(Clone, Copy)]
pub(crate) enum RuntimeScriptCtor {
    Static(ScriptConstructor<RuntimeScriptApi>),
    Dynamic(DynamicScriptConstructor<RuntimeScriptApi>),
}

impl RuntimeScriptCtor {
    #[inline]
    pub(crate) fn call(self) -> *mut RuntimeScriptBehavior {
        match self {
            Self::Static(ctor) => ctor(),
            Self::Dynamic(ctor) => ctor(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ForceWaterImpact2D {
    pub(crate) world: NodeID,
    pub(crate) position: perro_structs::Vector2,
    pub(crate) force: perro_structs::Vector2,
    pub(crate) strength: f32,
    pub(crate) radius: f32,
    pub(crate) cavitation: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ForceWaterImpact3D {
    pub(crate) world: NodeID,
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

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WaterEntryState3D {
    pub(crate) touching: bool,
    pub(crate) last_contact_time: f32,
}

/// Entry cap for the by-path scene caches (`scene_cache` /
/// `prepared_scene_cache`). Must stay comfortably above typical preload
/// counts (the demo hub preloads 16 scenes) so preloaded scenes keep their
/// cached entries; preloaded scenes also hold their own `Rc` clones in the
/// `preloaded_*` maps, so eviction here never invalidates a preload handle.
pub(crate) const SCENE_CACHE_MAX_ENTRIES: usize = 32;

/// By-path scene cache with a simple entry-count LRU cap
/// ([`SCENE_CACHE_MAX_ENTRIES`]). `get` touches recency, so scenes cycled
/// through repeatedly (demo hub) stay resident while one-off loads age out.
pub(crate) struct ScenePathLruCache<T> {
    map: AHashMap<String, Arc<T>>,
    /// LRU order, most-recently-used at the back.
    order: std::collections::VecDeque<String>,
}

impl<T> Default for ScenePathLruCache<T> {
    fn default() -> Self {
        Self {
            map: AHashMap::new(),
            order: std::collections::VecDeque::new(),
        }
    }
}

impl<T> ScenePathLruCache<T> {
    pub(crate) fn get(&mut self, path: &str) -> Option<Arc<T>> {
        let value = self.map.get(path).cloned()?;
        self.touch(path);
        Some(value)
    }

    pub(crate) fn insert(&mut self, path: String, value: Arc<T>) {
        if self.map.insert(path.clone(), value).is_some() {
            self.touch(&path);
        } else {
            self.order.push_back(path);
            while self.map.len() > SCENE_CACHE_MAX_ENTRIES {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                self.map.remove(&oldest);
            }
        }
    }

    pub(crate) fn remove(&mut self, path: &str) -> Option<Arc<T>> {
        let removed = self.map.remove(path)?;
        if let Some(index) = self.order.iter().position(|entry| entry == path) {
            self.order.remove(index);
        }
        Some(removed)
    }

    /// Move `path` to the most-recently-used slot. `order` is capped at
    /// [`SCENE_CACHE_MAX_ENTRIES`], so the linear scan stays trivial.
    fn touch(&mut self, path: &str) {
        if let Some(index) = self.order.iter().position(|entry| entry == path)
            && index + 1 != self.order.len()
        {
            let entry = self.order.remove(index).expect("index from position");
            self.order.push_back(entry);
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
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

#[derive(Default)]
pub(crate) struct WorldMembershipCache {
    revision: u64,
    initialized: bool,
    owner_by_slot: Vec<NodeID>,
    members: AHashMap<NodeID, Vec<NodeID>>,
    /// lazily built `Arc` views of `members`, valid for the same revision.
    /// hands member lists to per-frame readers as a refcount clone instead of
    /// a full `Vec` memcpy per caller (~8x/frame).
    members_shared: AHashMap<NodeID, Arc<[NodeID]>>,
    /// every stream/sub-view node in the arena (all 6 types), refreshed with
    /// the membership walk. lets extraction passes visit stream candidates
    /// without a full arena type-scan per pass.
    stream_nodes: Vec<NodeID>,
    /// SubView2D/3D + UiSubView count seen by the membership walk. 0 = no node
    /// can own a non-nil world, so per-dispatch suspend checks early-out w/o
    /// touching the owner table.
    sub_view_count: usize,
}

/// Live game runtime state.
///
/// Keeps scene nodes, script schedules, resource APIs, input snapshots,
/// physics state, audio propagation, and retained render state in one owner.
pub struct Runtime {
    // Keep per-callback state together; cold service caches follow.
    pub nodes: NodeArena,
    pub(crate) scripts: ScriptCollection,
    schedules: ScriptSchedules,
    pub(crate) script_runtime: ScriptRuntimeState,
    pub(crate) active_runtime_nodes: Vec<NodeID>,
    pub time: Timing,
    pub(crate) input: InputSnapshot,

    pub(crate) timer_runtime: TimerRuntimeState,
    provider_mode: ProviderMode,
    project: Option<Rc<RuntimeProject>>,
    pub(crate) scene_runtime: SceneRuntimeState,

    pub(crate) render: RenderState,
    pub(crate) extraction: ExtractionState,
    pub(crate) world_membership: RefCell<WorldMembershipCache>,
    /// Split-revision memo for effective visibility + ancestor modulation.
    /// RefCell lets read-only per-frame query paths stamp results.
    pub(crate) vis_memo: RefCell<world_state::VisibilityModulateMemo>,
    /// Per-world suspension result memo, invalidated by visibility, sub-view
    /// flags, and topology changes.
    pub(crate) suspension_memo: RefCell<world_state::SuspensionMemo>,
    pub(crate) dirty: DirtyState,
    pub(crate) transforms: TransformRuntimeState,
    internal_updates: InternalUpdateState,

    render_2d: Render2DState,
    render_3d: Render3DState,
    /// reusable buffer 4 modulated mesh-surface resolve; avoid per-frame Vec alloc
    /// per moving mesh (perro_nodes type; not storable in perro_runtime_render).
    mesh_surface_scratch: Vec<perro_nodes::MeshSurfaceBinding>,
    /// reusable sets 4 batched retained-draw material invalidation; avoid 2
    /// fresh AHashSets per resource-event batch.
    material_invalidation_ids_scratch: AHashSet<MaterialID>,
    material_invalidation_nodes_scratch: AHashSet<NodeID>,
    /// reusable node list 4 MeshCreated same-source rerender marks.
    mesh_source_dirty_scratch: Vec<NodeID>,
    /// reusable wire-segment + batched-line buffers 4 collision debug draws;
    /// one DrawDebugLines3D command per body instead of one box per edge.
    collision_wire_scratch: Vec<(glam::Vec3, glam::Vec3)>,
    collision_debug_lines_scratch: Vec<perro_render_bridge::DebugLine3D>,
    render_ui: RenderUiState,
    locale_text: state::LocaleTextState,
    pub(crate) signal_runtime: SignalRuntimeState,
    pub(crate) node_index: NodeIndexState,
    pub(crate) node_api_scratch: NodeApiScratchState,
    pub(crate) resource_api: Rc<RuntimeResourceApi>,
    /// Deferred-boot flag: ctor skip load_boot_scene -> runner load it aft
    /// window+splash up (see `load_boot_scene_if_pending`). Sync ctors kp false.
    boot_scene_pending: bool,
    startup_input_clear_frames_left: u32,
    cursor_icon_request: Option<perro_ui::CursorIcon>,
    pub(crate) window_requests: Vec<WindowRequest>,
    pub(crate) active_refresh_rate: Option<f32>,
    pub(crate) physics_gravity_override: Option<f32>,
    pub(crate) physics_coef_override: Option<f32>,
    physics: physics::PhysicsState,
    pub(crate) physics_sync: PhysicsSyncState,
    // Value = (asset source, scene-authored bone pose overrides applied
    // after the async bone load lands).
    pending_skeleton_sources_2d:
        AHashMap<NodeID, (String, Vec<scene_loader::prepare::PendingBonePoseOverride>)>,
    pending_skeleton_sources_3d:
        AHashMap<NodeID, (String, Vec<scene_loader::prepare::PendingBonePoseOverride>)>,
    /// reusable subtree-walk stack 4 force_rerender; avoid per-node
    /// children_slice().to_vec() alloc on every visited node.
    force_rerender_stack_scratch: Vec<NodeID>,
    /// reusable type-lane scan buf 4 UI control sync/update passes.
    ui_node_ids_scratch: Vec<NodeID>,
    pub(crate) audio: AudioPropagationState,
    /// Per-node cache 4 mesh point/ray/region queries; avoids re-cloning
    /// surfaces + rebuilding per-instance Mat4s (MultiMeshInstance3D) on
    /// every query. Keyed by NodeID (generation-safe on slot reuse) +
    /// validated against `(structural_revision, node_change_stamp)` @ build
    /// time, so only writes 2 THAT node retire the entry.
    mesh_query_node_cache: mesh_query::QueryNodeDataCache,
    /// Decoded query geometry (verts + tris + BVH) 4 mesh point/ray/region
    /// queries. Per-runtime, NOT static: both key spaces (`MeshID` + revision,
    /// source path) only mean anything inside one Runtime, so a shared map fed
    /// runtime B the geometry runtime A cached under the same id.
    /// `Mutex` cuz the query paths read it thru `&self`.
    mesh_query_mesh_cache: std::sync::Mutex<mesh_query::QueryMeshCache>,
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
    fn advance_timers(&mut self, delta_time: f32) {
        use perro_runtime_api::sub_apis::SignalAPI;

        let due = self.timer_runtime.advance(delta_time);
        for signal in due.iter().copied() {
            self.signal_emit(signal, &[]);
        }
        self.timer_runtime.reuse_due(due);
    }

    #[cfg(feature = "bench")]
    pub fn bench_timer_start(
        &mut self,
        timer: perro_ids::TimerID,
        finished: perro_ids::SignalID,
        duration: Duration,
    ) {
        self.timer_runtime.start(timer, finished, duration);
    }

    #[cfg(feature = "bench")]
    pub fn bench_timer_advance(&mut self, delta_time: f32) -> usize {
        let due = self.timer_runtime.advance(delta_time);
        let count = due.len();
        self.timer_runtime.reuse_due(due);
        count
    }

    #[cfg(feature = "bench")]
    pub fn bench_timer_counts(&self) -> (usize, usize, usize) {
        self.timer_runtime.counts()
    }

    pub fn new() -> Self {
        Self {
            physics_sync: PhysicsSyncState::new(),
            extraction: ExtractionState::new(),
            scene_runtime: SceneRuntimeState::new(),
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
            timer_runtime: TimerRuntimeState::new(),
            provider_mode: ProviderMode::Dynamic,
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            schedules: ScriptSchedules::new(),
            script_runtime: ScriptRuntimeState::new(),
            active_runtime_nodes: Vec::new(),
            project: None,
            render: RenderState::new(),
            world_membership: RefCell::new(WorldMembershipCache::default()),
            vis_memo: RefCell::new(world_state::VisibilityModulateMemo::default()),
            suspension_memo: RefCell::new(world_state::SuspensionMemo::default()),
            dirty: DirtyState::new(),
            transforms: TransformRuntimeState::new(),
            internal_updates: InternalUpdateState::new(),
            render_2d: Render2DState::new(),
            render_3d: Render3DState::new(),
            mesh_surface_scratch: Vec::new(),
            material_invalidation_ids_scratch: AHashSet::default(),
            material_invalidation_nodes_scratch: AHashSet::default(),
            mesh_source_dirty_scratch: Vec::new(),
            collision_wire_scratch: Vec::new(),
            collision_debug_lines_scratch: Vec::new(),
            render_ui: RenderUiState::new(),
            locale_text: state::LocaleTextState::new(),
            signal_runtime: SignalRuntimeState::new(),
            node_index: NodeIndexState::new(),
            node_api_scratch: NodeApiScratchState::new(),
            resource_api: RuntimeResourceApi::new(None, None, None, None, None, None, None, None),
            input: InputSnapshot::new(),
            boot_scene_pending: false,
            startup_input_clear_frames_left: 0,
            cursor_icon_request: None,
            window_requests: Vec::new(),
            active_refresh_rate: None,
            physics_gravity_override: None,
            physics_coef_override: None,
            physics: physics::PhysicsState::new(),
            pending_skeleton_sources_2d: AHashMap::new(),
            pending_skeleton_sources_3d: AHashMap::new(),
            force_rerender_stack_scratch: Vec::new(),
            ui_node_ids_scratch: Vec::new(),
            audio: AudioPropagationState::new(),
            mesh_query_node_cache: mesh_query::QueryNodeDataCache::default(),
            mesh_query_mesh_cache: std::sync::Mutex::default(),
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

    /// Set a node's Node3D-base modulate directly (bench-only), so the
    /// effective-modulate microbench can build a deep tinted hierarchy without a
    /// scene load.
    #[cfg(feature = "bench")]
    pub fn bench_set_node3d_modulate(&mut self, id: NodeID, modulate: perro_structs::NodeModulate) {
        if let Some(mut node) = self.nodes.get_mut(id) {
            node.with_base_mut::<perro_nodes::Node3D, _>(|base| base.modulate = modulate);
        }
    }

    /// Set a node's Node3D-base visibility (bench-only). The skinned-extraction
    /// microbench hides the static meshes so the retained visible-set copy stays
    /// O(1) and the measured cost isolates the dirty-skeleton scan (F2).
    #[cfg(feature = "bench")]
    pub fn bench_set_node3d_visible(&mut self, id: NodeID, visible: bool) {
        if let Some(mut node) = self.nodes.get_mut(id) {
            node.with_base_mut::<perro_nodes::Node3D, _>(|base| base.visible = visible);
        }
    }

    /// Fold `effective_self_modulate` over `ids` and sum channels so the result
    /// escapes optimization. Isolates the ancestor-walk cost (F1 microbench).
    #[cfg(feature = "bench")]
    pub fn bench_effective_self_modulate_sum(&self, ids: &[NodeID]) -> f32 {
        let mut acc = 0.0f32;
        for &id in ids {
            let color = self.effective_self_modulate(id);
            acc += color.r() + color.g() + color.b() + color.a();
        }
        acc
    }

    /// Bind a mesh instance to a skeleton (bench-only), mirroring the scene
    /// loader's mesh->skeleton link so the skinned-extraction microbench can wire
    /// a skeleton without a full scene load.
    #[cfg(feature = "bench")]
    pub fn bench_bind_mesh_skeleton(&mut self, mesh: NodeID, skeleton: NodeID) {
        if let Some(mut node) = self.nodes.get_mut(mesh)
            && let perro_nodes::SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data
        {
            mesh_instance.skeleton = skeleton;
        }
    }

    /// Mark a skeleton node dirty (bench-only), simulating a per-frame bone-write
    /// so repeated `extract_render_snapshot_commands` exercises the dirty-skeleton
    /// scan (F2 microbench).
    #[cfg(feature = "bench")]
    pub fn bench_touch_node(&mut self, id: NodeID) {
        self.dirty.mark_rerender(id);
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

    pub(crate) fn cached_rigid_body_ids_2d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.physics_sync.water_rigid_body_ids_2d_cache_version != Some(version) {
            self.physics_sync.water_rigid_body_ids_2d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::RigidBody2D,
                |node| matches!(&node.data, perro_nodes::SceneNodeData::RigidBody2D(body) if body.enabled),
                &mut self.physics_sync.water_rigid_body_ids_2d_cache,
            );
            self.physics_sync.water_rigid_body_ids_2d_cache_version = Some(version);
        }
        &self.physics_sync.water_rigid_body_ids_2d_cache
    }

    pub(crate) fn cached_rigid_body_ids_3d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.physics_sync.water_rigid_body_ids_3d_cache_version != Some(version) {
            self.physics_sync.water_rigid_body_ids_3d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::RigidBody3D,
                |node| matches!(&node.data, perro_nodes::SceneNodeData::RigidBody3D(body) if body.enabled),
                &mut self.physics_sync.water_rigid_body_ids_3d_cache,
            );
            self.physics_sync.water_rigid_body_ids_3d_cache_version = Some(version);
        }
        &self.physics_sync.water_rigid_body_ids_3d_cache
    }

    /// Coastline scan candidates: StaticBody2D + RigidBody2D + CharacterBody2D.
    /// `enabled` + mask filter applied by caller; keep cache type/existence only
    /// so it self-invalidates on physics_revision like the rigid-body caches.
    pub(crate) fn cached_water_collision_body_ids_2d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.physics_sync.water_collision_body_ids_2d_cache_version != Some(version) {
            self.physics_sync.water_collision_body_ids_2d_cache.clear();
            scan_node_type_slots_any(
                &self.nodes,
                &[
                    perro_nodes::NodeType::StaticBody2D,
                    perro_nodes::NodeType::RigidBody2D,
                    perro_nodes::NodeType::CharacterBody2D,
                ],
                &mut self.physics_sync.water_collision_body_ids_2d_cache,
            );
            self.physics_sync.water_collision_body_ids_2d_cache_version = Some(version);
        }
        &self.physics_sync.water_collision_body_ids_2d_cache
    }

    /// Coastline scan candidates: StaticBody3D + RigidBody3D + CharacterBody3D.
    pub(crate) fn cached_water_collision_body_ids_3d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.physics_sync.water_collision_body_ids_3d_cache_version != Some(version) {
            self.physics_sync.water_collision_body_ids_3d_cache.clear();
            scan_node_type_slots_any(
                &self.nodes,
                &[
                    perro_nodes::NodeType::StaticBody3D,
                    perro_nodes::NodeType::RigidBody3D,
                    perro_nodes::NodeType::CharacterBody3D,
                ],
                &mut self.physics_sync.water_collision_body_ids_3d_cache,
            );
            self.physics_sync.water_collision_body_ids_3d_cache_version = Some(version);
        }
        &self.physics_sync.water_collision_body_ids_3d_cache
    }

    pub(crate) fn cached_water_ids_2d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.physics_sync.water_ids_2d_cache_version != Some(version) {
            self.physics_sync.water_ids_2d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::WaterBody2D,
                |node| matches!(node.data, perro_nodes::SceneNodeData::WaterBody2D(_)),
                &mut self.physics_sync.water_ids_2d_cache,
            );
            self.physics_sync.water_ids_2d_cache_version = Some(version);
        }
        &self.physics_sync.water_ids_2d_cache
    }

    pub(crate) fn cached_water_ids_3d(&mut self) -> &[NodeID] {
        let version = self.nodes.physics_revision();
        if self.physics_sync.water_ids_3d_cache_version != Some(version) {
            self.physics_sync.water_ids_3d_cache.clear();
            scan_node_type_slots(
                &self.nodes,
                perro_nodes::NodeType::WaterBody3D,
                |node| matches!(node.data, perro_nodes::SceneNodeData::WaterBody3D(_)),
                &mut self.physics_sync.water_ids_3d_cache,
            );
            self.physics_sync.water_ids_3d_cache_version = Some(version);
        }
        &self.physics_sync.water_ids_3d_cache
    }

    pub(crate) fn apply_loaded_skeleton_bones(&mut self) {
        // no pending source = nothing 2 apply. skip the poll (4 mutex locks +
        // 2 try_recv / frame): every consumer of the bone cache
        // (cached_bones_2d/3d, load_skeleton_*) polls on its own, so a landed
        // load can never starve while both maps stay empty.
        if self.pending_skeleton_sources_2d.is_empty()
            && self.pending_skeleton_sources_3d.is_empty()
        {
            return;
        }
        self.resource_api.poll_skeleton_bone_loads();
        let mut changed_2d = Vec::new();
        // Nodes freed before their async bone load landed: the entry can never
        // resolve, so drop it instead of rescanning it every frame forever.
        let mut dropped_2d = Vec::new();
        for (node, (source, overrides)) in &self.pending_skeleton_sources_2d {
            if self.nodes.get(*node).is_none() {
                dropped_2d.push(*node);
                continue;
            }
            if let Some(bones) = self.resource_api.cached_bones_2d(source)
                && let Some(scene_node) = self.nodes.get_mut_untracked(*node)
                && let perro_nodes::SceneNodeData::Skeleton2D(skeleton) = &mut scene_node.data
            {
                skeleton.bones = bones;
                scene_loader::prepare::apply_bone_pose_overrides_2d(skeleton, overrides);
                changed_2d.push(*node);
            }
        }
        for node in &dropped_2d {
            self.pending_skeleton_sources_2d.remove(node);
        }
        for node in &changed_2d {
            self.pending_skeleton_sources_2d.remove(node);
            self.mark_transform_dirty_recursive(*node);
        }

        let mut changed_3d = Vec::new();
        let mut dropped_3d = Vec::new();
        for (node, (source, overrides)) in &self.pending_skeleton_sources_3d {
            if self.nodes.get(*node).is_none() {
                dropped_3d.push(*node);
                continue;
            }
            if let Some(bones) = self.resource_api.cached_bones_3d(source)
                && let Some(scene_node) = self.nodes.get_mut_untracked(*node)
                && let perro_nodes::SceneNodeData::Skeleton3D(skeleton) = &mut scene_node.data
            {
                skeleton.bones = bones;
                skeleton.refresh_inv_bind_cache();
                scene_loader::prepare::apply_bone_pose_overrides_3d(skeleton, overrides);
                changed_3d.push(*node);
            }
        }
        for node in &dropped_3d {
            self.pending_skeleton_sources_3d.remove(node);
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
        Self::from_project_inner(project, provider_mode, script_registry, false)
    }

    /// Same as `from_project_with_script_registry` but skip boot-scene load.
    /// Caller MUST cal `load_boot_scene_if_pending` b4 first update/present of
    /// gameplay. Lets windowed runners show window+splash b4 multi-second
    /// scene load; headless/editor/tests kp sync ctor.
    pub fn from_project_with_script_registry_deferred_boot(
        project: RuntimeProject,
        provider_mode: ProviderMode,
        script_registry: Option<StaticScriptRegistry>,
    ) -> Self {
        Self::from_project_inner(project, provider_mode, script_registry, true)
    }

    /// False while deferred boot load ! run yet.
    #[inline]
    pub fn boot_scene_loaded(&self) -> bool {
        !self.boot_scene_pending
    }

    /// Run deferred boot-scene load once; no-op aft. Same panic contract as
    /// sync ctor path.
    pub fn load_boot_scene_if_pending(&mut self) {
        if !self.boot_scene_pending {
            return;
        }
        self.boot_scene_pending = false;
        if let Err(err) = self.load_boot_scene() {
            panic!("failed to load boot scene: {err}");
        }
    }

    fn from_project_inner(
        project: RuntimeProject,
        provider_mode: ProviderMode,
        script_registry: Option<StaticScriptRegistry>,
        defer_boot: bool,
    ) -> Self {
        let mut runtime = Self::new();
        perro_structs::structs::boot_log::mark("runtime_state_built");
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
        runtime.project = Some(Rc::new(project));
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
        perro_structs::structs::boot_log::mark("runtime_resources_built");
        runtime.configure_audio_from_project();
        perro_structs::structs::boot_log::mark("runtime_audio_cfg_ready");
        runtime.input.set_input_map(input_map);
        perro_structs::structs::boot_log::mark("runtime_input_map_ready");
        if let Some(entries) = script_registry {
            debug_assert!(entries.windows(2).all(|pair| pair[0].0 < pair[1].0));
            runtime.script_runtime.static_script_registry = entries;
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
        if defer_boot {
            runtime.boot_scene_pending = true;
        } else {
            perro_structs::structs::boot_log::mark("runtime_boot_scene_load_start");
            if let Err(err) = runtime.load_boot_scene() {
                panic!("failed to load boot scene: {err}");
            }
        }
        perro_structs::structs::boot_log::mark("runtime_boot_scene_loaded");
        runtime
    }

    pub fn project(&self) -> Option<&RuntimeProject> {
        self.project.as_deref()
    }

    pub fn provider_mode(&self) -> ProviderMode {
        self.provider_mode
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
        perro_project::SteamInputMode::Fallback => {
            perro_steamworks::input::SteamInputMode::Fallback
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

/// Collect ids of any node whose type matches one in `want`, in arena slot
/// order. Cheap tag compare per slot; used for the multi-type water coastline
/// body cache (static/rigid/character bodies).
fn scan_node_type_slots_any(
    arena: &NodeArena,
    want: &[perro_nodes::NodeType],
    out: &mut Vec<NodeID>,
) {
    let types = arena.node_type_slots();
    for (index, node_type) in types.iter().enumerate() {
        if !want.contains(node_type) {
            continue;
        }
        if let Some((id, _node)) = arena.slot_get(index) {
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
