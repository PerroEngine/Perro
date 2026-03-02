pub mod api;
pub mod sub_apis;

pub use api::ResourceContext;

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceContext};
    pub use crate::sub_apis::{
        MaterialAPI, MaterialModule, MeshAPI, MeshModule, TerrainAPI, TerrainModule, TextureAPI,
        TextureModule,
    };
    pub use crate::{
        create_material, drop_material, drop_mesh, drop_texture, load_material, load_mesh,
        load_texture, reserve_material, reserve_mesh, reserve_texture,
    };
}
