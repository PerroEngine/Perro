//! Resource API domains.
//!
//! Each child module defines one script-facing resource domain. Traits describe
//! the backing store contract, `*Module` wrappers provide method-call syntax
//! through `ResourceWindow`, and exported macros provide compact script syntax.

// ---- Resource domains ----

mod animation;
mod animation_tree;
mod audio;
mod csv_table;
mod draw_2d;
mod gltf;
mod localization;
mod material;
mod mesh;
mod mic;
mod navmesh;
mod post_processing;
mod scene_doc;
mod skeleton;
mod texture;
mod video;
mod visual_accessibility;
mod webcam;

// ---- Animation + audio ----

pub use animation::{AnimationAPI, AnimationModule};
pub use animation_tree::{AnimationTreeAPI, AnimationTreeModule};
pub use audio::{
    Audio, Audio2D, Audio2DModule, Audio3D, Audio3DModule, AudioAPI, AudioClip, AudioCompression,
    AudioDirection, AudioEffects, AudioEq, AudioModule, AudioPan, AudioPlayConfig, MidiChannel,
    MidiModule, MidiNoteHandle, MidiNoteOptions, MidiProgram, MidiSong, MidiSound, MidiSpatialPos,
    MidiSpatialPosition, Note, PannedAudio, SpatialAudioOptions, bus_id, program,
};

// ---- Data + draw resources ----

pub use csv_table::{CsvAPI, CsvModule};
pub use draw_2d::{Draw2DAPI, Draw2DModule};
pub use gltf::{GlbModule, GltfAPI, GltfInfo};
pub use localization::{IntoLocale, Locale, LocalizationAPI, LocalizationModule};

// ---- Render assets ----

pub use material::{MaterialAPI, MaterialModule, MaterialReserveArg};
pub use mesh::{MeshAPI, MeshModule, MeshReserveArg};
pub use mic::{MicAPI, MicClip, MicDenoiseSettings, MicModule, MicSettings};
pub use navmesh::{
    NavMesh3D, NavMeshAPI, NavMeshModule, NavMeshTriangle3D, parse_pnav_bytes, parse_pnav_text,
};
pub use perro_ids::{AudioBusID, SoundFontID};
pub use post_processing::PostProcessingAPI;

// ---- Scene/accessibility ----

pub use scene_doc::{IntoSceneDoc, SceneDocAPI, SceneDocModule};
pub use skeleton::{SkeletonAPI, SkeletonModule};
pub use texture::{TextureAPI, TextureModule, TextureReserveArg};
pub use video::{VideoAPI, VideoModule, VideoUpdate};
pub use visual_accessibility::VisualAccessibilityAPI;
pub use webcam::{WebcamAPI, WebcamConfig, WebcamDevice, WebcamFrame, WebcamModule};
