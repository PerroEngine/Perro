//! Runtime API domains.
//!
//! Each child module defines one script-facing runtime domain. Traits describe
//! the engine implementation contract, `*Module` wrappers provide method-call
//! syntax through `RuntimeWindow`, and exported macros provide compact script
//! syntax.

// ---- Runtime domains ----

mod animation;
mod animation_tree;
mod audio;
mod navmesh;
mod node;
mod physics;
mod scene;
mod script;
mod signal;
mod time;
mod window;

// ---- Animation ----

pub use animation::{AnimPlayerAPI, AnimPlayerModule};
pub use animation_tree::{AnimTreeAPI, AnimTreeModule, AnimTreeSlotArg, IntoAnimTreeSlotArg};

// ---- Audio ----

pub use audio::{
    AttachedMidiTarget, AudioCompression, AudioDirection, AudioEffects, AudioEq, MidiChannel,
    MidiNoteHandle, MidiNoteOptions, MidiProgram, MidiSong, MidiSound, Note, RuntimeAudio,
    RuntimeAudioAPI, RuntimeAudioModule, RuntimeMidiModule, SpatialAudioOptions, program,
};
pub use navmesh::{
    NavMeshAPI, NavMeshModule, NavMeshPath3D, NavMeshPathOptions, NavMeshPathStatus,
};

// ---- Nodes + queries ----

pub use node::{
    __query_base_type_mask, __query_type_mask, CameraRay3D, IntoNodeCollection,
    IntoNodeCreateBatch, IntoNodeTag, IntoNodeTags, IntoQueryBounds, MeshDataSurfaceHit3D,
    MeshDataSurfaceRegion3D, MeshMaterialRegion3D, MeshQueryModule, MeshSurfaceHit3D,
    MeshSurfaceRay3D, NodeAPI, NodeCollection, NodeCollectionEntry, NodeCreateBatch, NodeModule,
    NodeQuery, NodeQueryModule, NodeQueryView, NodeRootPatch, NodeSceneSpec, NodeScriptSpec,
    NodeScriptVar, NodeSpec, QueryBounds, QueryExpr, QueryScope, QueryTypeMask,
    collect_subtree_ids,
};

// ---- Simulation domains ----

pub use physics::{
    IntoImpulseDirection, PhysicsAPI, PhysicsBodyPrediction2D, PhysicsBodyPrediction3D,
    PhysicsContact2D, PhysicsContact3D, PhysicsLaunchSolution2D, PhysicsLaunchSolution3D,
    PhysicsModule, PhysicsMoveResult2D, PhysicsMoveResult3D, PhysicsQueryFilter, PhysicsRayHit2D,
    PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D, PhysicsSlideResult2D,
    PhysicsSlideResult3D,
};

// ---- Scene/script bus ----

pub use scene::{
    IntoPreloadedSceneID, IntoPreloadedSceneTarget, IntoSceneLoadSource, IntoScenePath,
    PreloadedSceneID, PreloadedSceneTarget, SceneAPI, SceneLoadSource, SceneModule,
};
pub use script::{IntoScriptMemberID, ScriptAPI, ScriptModule};
pub use signal::{SignalAPI, SignalModule};

// ---- Frame/window ----

pub use time::{ProfilingSnapshot, TimeAPI, TimeModule};
pub use window::{CursorIcon, FrameRateCap, WindowAPI, WindowMode, WindowModule, WindowRequest};
