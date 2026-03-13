pub mod api;
pub mod sub_apis;

pub use api::ResourceContext;

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceContext};
    pub use crate::sub_apis::{
        AudioAPI, AudioModule, MaterialAPI, MaterialModule, MeshAPI, MeshModule, TerrainAPI,
        TerrainModule, TextureAPI, TextureModule,
    };
    pub use crate::{
        create_material, drop_material, drop_mesh, drop_texture, load_material, load_mesh,
        load_texture, loop_audio, play_audio, reserve_material, reserve_mesh, reserve_texture,
        stop_all_audio, stop_audio,
    };
}
