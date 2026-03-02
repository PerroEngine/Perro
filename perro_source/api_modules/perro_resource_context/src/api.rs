use crate::sub_apis::{
    MaterialAPI, MaterialModule, MeshAPI, MeshModule, TerrainAPI, TerrainModule, TextureAPI,
    TextureModule,
};

pub trait ResourceAPI: TextureAPI + MeshAPI + MaterialAPI + TerrainAPI + Send + Sync {}
impl<T> ResourceAPI for T where T: TextureAPI + MeshAPI + MaterialAPI + TerrainAPI + Send + Sync {}

pub struct ResourceContext<'res, R: ResourceAPI + ?Sized> {
    api: &'res R,
}

#[allow(non_snake_case)]
impl<'res, R: ResourceAPI + ?Sized> ResourceContext<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn Textures(&self) -> TextureModule<'_, R> {
        TextureModule::new(self.api)
    }

    #[inline]
    pub fn Meshes(&self) -> MeshModule<'_, R> {
        MeshModule::new(self.api)
    }

    #[inline]
    pub fn Materials(&self) -> MaterialModule<'_, R> {
        MaterialModule::new(self.api)
    }

    #[inline]
    pub fn Terrain(&self) -> TerrainModule<'_, R> {
        TerrainModule::new(self.api)
    }
}
