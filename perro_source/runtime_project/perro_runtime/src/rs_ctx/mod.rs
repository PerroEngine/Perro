mod animation;
mod animation_tree;
mod audio;
mod core;
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
mod state;
mod texture;
mod viewport;
mod visual_accessibility;

pub use core::RuntimeResourceApi;
pub(crate) use core::{QueuedSpatialAudioPos, QueuedSpatialMidiKind};

#[cfg(test)]
pub(crate) static PROJECT_ROOT_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));
