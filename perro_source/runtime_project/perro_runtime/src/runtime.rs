use crate::{
    cns::{NodeArena, ScriptCollection},
    rs_ctx::RuntimeResourceApi,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_input_api::InputSnapshot;
use perro_runtime_api::sub_apis::{PreloadedSceneID, WindowRequest};
use perro_scene::Scene;
use perro_scripting::{ScriptAPI, ScriptBehavior, ScriptConstructor};
use std::time::{Duration, Instant};
use std::{cell::RefCell, sync::Arc};

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
mod state;
mod transforms;
mod world_state;

use audio::AudioPropagationState;
pub(crate) use scene_loader::PendingScriptAttach;
#[cfg(feature = "bench")]
pub use scene_loader::{bench_prepare_and_merge_scene, bench_prepare_scene};
pub(crate) use state::CollisionDebugState;
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

/// Live game runtime state.
///
/// Keeps scene nodes, script schedules, resource APIs, input snapshots,
/// physics state, audio propagation, and retained render state in one owner.
pub struct Runtime {
    pub time: Timing,
    provider_mode: ProviderMode,
    project: Option<Arc<RuntimeProject>>,
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
    dirty: DirtyState,
    transforms: TransformRuntimeState,
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
    cursor_icon_request: Option<perro_ui::CursorIcon>,
    pub(crate) window_requests: Vec<WindowRequest>,
    physics: physics::PhysicsState,
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
    pub(crate) force_water_impacts_2d: Vec<ForceWaterImpact2D>,
    pub(crate) force_water_impacts_3d: Vec<ForceWaterImpact3D>,
    pub(crate) pending_force_emitters_2d: Vec<perro_nodes::PhysicsForceEmitter2D>,
    pub(crate) pending_force_emitters_3d: Vec<perro_nodes::PhysicsForceEmitter3D>,
    pub(crate) audio: AudioPropagationState,
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
    /// Last measured frames per second.
    pub fps: f32,
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
            },
            provider_mode: ProviderMode::Dynamic,
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
            cursor_icon_request: None,
            window_requests: Vec::new(),
            physics: physics::PhysicsState::new(),
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
            force_water_impacts_2d: Vec::new(),
            force_water_impacts_3d: Vec::new(),
            pending_force_emitters_2d: Vec::new(),
            pending_force_emitters_3d: Vec::new(),
            audio: AudioPropagationState::new(),
        }
    }

    pub fn from_project(project: RuntimeProject, provider_mode: ProviderMode) -> Self {
        Self::from_project_with_script_registry(project, provider_mode, None)
    }

    #[inline]
    pub(crate) fn reset_water_scan_cache_2d(&mut self) {
        self.water_rigid_body_ids_2d_cache.clear();
        self.water_ids_2d_cache.clear();
    }

    #[inline]
    pub(crate) fn reset_water_scan_cache_3d(&mut self) {
        self.water_rigid_body_ids_3d_cache.clear();
        self.water_ids_3d_cache.clear();
    }

    #[inline]
    pub(crate) fn reset_water_scan_cache_all(&mut self) {
        self.reset_water_scan_cache_2d();
        self.reset_water_scan_cache_3d();
    }

    pub(crate) fn cached_rigid_body_ids_2d(&mut self) -> &[NodeID] {
        if self.water_rigid_body_ids_2d_cache.is_empty() {
            self.water_rigid_body_ids_2d_cache
                .extend(self.nodes.iter().filter_map(|(id, node)| match &node.data {
                    perro_nodes::SceneNodeData::RigidBody2D(body) if body.enabled => Some(id),
                    _ => None,
                }));
        }
        &self.water_rigid_body_ids_2d_cache
    }

    pub(crate) fn cached_rigid_body_ids_3d(&mut self) -> &[NodeID] {
        if self.water_rigid_body_ids_3d_cache.is_empty() {
            self.water_rigid_body_ids_3d_cache
                .extend(self.nodes.iter().filter_map(|(id, node)| match &node.data {
                    perro_nodes::SceneNodeData::RigidBody3D(body) if body.enabled => Some(id),
                    _ => None,
                }));
        }
        &self.water_rigid_body_ids_3d_cache
    }

    pub(crate) fn cached_water_ids_2d(&mut self) -> &[NodeID] {
        if self.water_ids_2d_cache.is_empty() {
            self.water_ids_2d_cache
                .extend(self.nodes.iter().filter_map(|(id, node)| match node.data {
                    perro_nodes::SceneNodeData::WaterBody2D(_) => Some(id),
                    _ => None,
                }));
        }
        &self.water_ids_2d_cache
    }

    pub(crate) fn cached_water_ids_3d(&mut self) -> &[NodeID] {
        if self.water_ids_3d_cache.is_empty() {
            self.water_ids_3d_cache
                .extend(self.nodes.iter().filter_map(|(id, node)| match node.data {
                    perro_nodes::SceneNodeData::WaterBody3D(_) => Some(id),
                    _ => None,
                }));
        }
        &self.water_ids_3d_cache
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
        #[cfg(feature = "steamworks")]
        let steam_config = project.config.steam.clone();
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = provider_mode;
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
        if let Some(entries) = script_registry {
            for (path_hash, ctor) in entries {
                runtime
                    .script_runtime
                    .dynamic_script_registry
                    .insert(*path_hash, *ctor);
            }
        }
        #[cfg(feature = "steamworks")]
        if let Err(err) =
            perro_steamworks::runtime::init_from_config(steam_config.enabled, steam_config.app_id)
        {
            panic!("failed to initialize Steam: {err}");
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
        #[cfg(feature = "steamworks")]
        let _ = perro_steamworks::runtime::run_callbacks();
        self.run_start_schedule();
        self.schedules.snapshot_update(&self.scripts);
        self.run_update_schedule();
        self.run_internal_update_schedule();
        self.propagate_pending_transform_dirty();
        self.update_audio_propagation(delta_time);
    }

    #[inline]
    pub fn update_timed(&mut self, delta_time: f32) -> RuntimeUpdateTiming {
        let total_start = std::time::Instant::now();
        self.time.delta = delta_time;
        #[cfg(feature = "steamworks")]
        let _ = perro_steamworks::runtime::run_callbacks();

        let start_schedule_start = std::time::Instant::now();
        self.run_start_schedule();
        let start_schedule = start_schedule_start.elapsed();

        let snapshot_start = std::time::Instant::now();
        self.schedules.snapshot_update(&self.scripts);
        let snapshot_update = snapshot_start.elapsed();

        let update_schedule = self.run_update_schedule_timed();

        let internal_start = std::time::Instant::now();
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

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_hotpath_tests.rs"]
mod runtime_hotpath_tests;
