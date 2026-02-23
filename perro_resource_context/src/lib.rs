pub mod api;
pub mod sub_apis;

pub use api::ResourceContext;

#[macro_export]
macro_rules! load_texture {
    ($res:expr, $source:expr) => {
        $res.Textures().load($source)
    };
}

#[macro_export]
macro_rules! load_mesh {
    ($res:expr, $source:expr) => {
        $res.Meshes().load($source)
    };
}

#[macro_export]
macro_rules! load_material {
    ($res:expr, $source:expr) => {
        $res.Materials().load($source)
    };
}

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceContext};
    pub use crate::sub_apis::{MaterialAPI, MaterialModule, MeshAPI, MeshModule, TextureAPI, TextureModule};
    pub use crate::{load_material, load_mesh, load_texture};
}
