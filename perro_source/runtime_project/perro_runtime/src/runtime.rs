use crate::{
    cns::{NodeArena, ScriptCollection, TerrainStore},
    rs_ctx::RuntimeResourceApi,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_input::InputSnapshot;
use perro_runtime_context::sub_apis::PreloadedSceneID;
use perro_scene::Scene;
use perro_scripting::ScriptConstructor;
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};
use std::time::Duration;

mod input_bridge;
mod internal_updates;
mod physics;
mod render_2d;
mod render_3d;
mod render_bridge;
mod scene_loader;
mod scheduling;
mod state;
mod transforms;
mod world_state;

pub(crate) use scene_loader::PendingScriptAttach;
use state::{
    DirtyState, InternalUpdateState, NodeIndexState, Render2DState, Render3DState, RenderState,
    ScriptRuntimeState, ScriptSchedules, SignalRuntimeState, TransformRuntimeState,
};
pub(crate) use state::{
    CollisionDebugState, TerrainChunkMeshKey, TerrainChunkMeshState, TerrainDebugState,
};

type RuntimeScriptCtor = ScriptConstructor<Runtime, RuntimeResourceApi, InputSnapshot>;
type StaticScriptRegistry = &'static [(&'static str, RuntimeScriptCtor)];

pub struct Runtime {
    pub time: Timing,
    provider_mode: ProviderMode,
    project: Option<Arc<RuntimeProject>>,
    pub(crate) scene_cache: RefCell<AHashMap<String, Arc<Scene>>>,
    pub(crate) preloaded_scenes: AHashMap<PreloadedSceneID, Arc<Scene>>,
    pub(crate) preloaded_scene_paths: AHashMap<String, PreloadedSceneID>,
    pub(crate) preloaded_scene_reverse_paths: AHashMap<PreloadedSceneID, String>,
    pub(crate) next_preloaded_scene_id: u64,

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

#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateScheduleTiming {
    pub total: Duration,
    pub scripts_total: Duration,
    pub script_count: u32,
    pub slowest_script_id: Option<NodeID>,
    pub slowest_script: Duration,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeUpdateTiming {
    pub start_schedule: Duration,
    pub snapshot_update: Duration,
    pub update_schedule: UpdateScheduleTiming,
    pub internal_update: Duration,
    pub total: Duration,
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
            terrain_store: terrain_store.clone(),
            signal_runtime: SignalRuntimeState::new(),
            node_index: NodeIndexState::new(),
            resource_api: RuntimeResourceApi::new(None, None, None, None, terrain_store),
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
        let static_animation_lookup = project.static_animation_lookup;
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = provider_mode;
        runtime.resource_api = RuntimeResourceApi::new(
            static_material_lookup,
            static_audio_lookup,
            static_skeleton_lookup,
            static_animation_lookup,
            runtime.terrain_store.clone(),
        );
        if let Some(entries) = script_registry {
            for (path, ctor) in entries {
                runtime
                    .script_runtime
                    .dynamic_script_registry
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
    pub fn update_timed(&mut self, delta_time: f32) -> RuntimeUpdateTiming {
        let total_start = std::time::Instant::now();
        self.time.delta = delta_time;

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
