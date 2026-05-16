//! Perro runtime crate.
//!
//! Owns scene execution, script schedules, runtime APIs, physics sync,
//! audio propagation, and render command extraction. Leaf modules keep
//! subsystem behavior split by domain so this crate stays navigable.

mod cns;
mod material_schema;
mod render_result;
mod rs_ctx;
mod runtime;
mod runtime_project;

pub mod rt_ctx;
pub use rt_ctx as api;

pub use cns::node_arena::NodeArena;
pub use perro_input_api::InputSnapshot as RuntimeInputApi;
pub use perro_project::{bootstrap_project, create_new_project};
pub use perro_runtime_api::sub_apis::{WindowMode, WindowRequest};
pub use render_result::RuntimeRenderResult;
pub use rs_ctx::RuntimeResourceApi;
pub use runtime::{Runtime, RuntimeFixedUpdateTiming, RuntimeScriptApi, RuntimeUpdateTiming};
#[cfg(feature = "bench")]
pub use runtime::{bench_prepare_and_merge_scene, bench_prepare_scene};
pub use runtime_project::{
    AudioConfig, AudioPropagationConfig, LocalizationConfig, OcclusionCulling, ParticleSimDefault,
    ProjectLoadError, ProjectMetadata, ProjectRoute, ProjectRoutesConfig, ProviderMode,
    RuntimeProject, RuntimeProjectConfig, StaticAnimationLookup, StaticAnimationTreeLookup,
    StaticAudioLookup, StaticBytesLookup, StaticCsvLookup, StaticLocalizationLookup,
    StaticMaterialLookup, StaticParticleLookup, StaticProjectConfig, StaticSceneLookup,
    StaticShaderLookup, StaticSkeletonLookup, StaticTilesetLookup, StaticUiStyleLookup,
    default_project_toml, default_routes_config, ensure_project_layout, ensure_project_toml,
    load_project_toml, load_routes_toml, normalize_route_href, parse_project_toml,
    parse_routes_toml,
};
