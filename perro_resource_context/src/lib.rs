pub mod api;
pub mod sub_apis;

pub use api::ResourceContext;

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceContext};
    pub use crate::sub_apis::{MaterialAPI, MaterialModule, MeshAPI, MeshModule, TextureAPI, TextureModule};
    pub use crate::{load_material, load_mesh, load_texture};
}
