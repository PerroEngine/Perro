pub mod api;
pub mod sub_apis;

pub use api::ResourceContext;

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceContext};
    pub use crate::sub_apis::{
        Audio, AudioAPI, AudioModule, BusID, MaterialAPI, MaterialModule, MeshAPI, MeshModule,
        TerrainAPI, TerrainModule, TextureAPI, TextureModule,
    };
    pub use crate::{
        bus, create_material, drop_material, drop_mesh, drop_texture, load_material, load_mesh,
        load_texture, pause_bus, play_audio, reserve_material, reserve_mesh, reserve_texture,
        resume_bus, set_bus_speed, set_bus_volume, set_master_volume, stop_all_audio, stop_audio,
        stop_audio_source, stop_bus,
    };
}
