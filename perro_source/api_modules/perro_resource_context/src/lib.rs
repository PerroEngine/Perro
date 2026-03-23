pub mod api;
pub mod sub_apis;

pub use api::ResourceContext;

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceContext};
    pub use crate::sub_apis::{
        AnimationAPI, AnimationModule, Audio, AudioAPI, AudioBusID, AudioModule, MaterialAPI,
        MaterialModule, MeshAPI, MeshModule, PostProcessingAPI, SkeletonAPI, SkeletonModule,
        TerrainAPI, TerrainModule, TextureAPI, TextureModule, VisualAccessibilityAPI,
    };
    pub use crate::{
        animation_drop, animation_load, animation_reserve, audio_bus,
        audio_bus_pause, audio_bus_resume, audio_bus_set_speed, audio_bus_set_volume,
        audio_bus_stop, audio_drop, audio_length_millis, audio_length_seconds, audio_load,
        audio_play, audio_reserve, audio_set_master_volume, audio_stop, audio_stop_all,
        audio_stop_source, disable_colorblind_filter, enable_colorblind_filter, material_create,
        material_drop, material_load, material_reserve, mesh_drop, mesh_load, mesh_reserve,
        post_processing_add, post_processing_clear, post_processing_remove, post_processing_set,
        skeleton_load_bones, texture_drop, texture_load, texture_reserve,
    };
}
