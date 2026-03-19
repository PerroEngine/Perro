use crate::sub_apis::{
    AudioAPI, AudioModule, MaterialAPI, MaterialModule, MeshAPI, MeshModule, SkeletonAPI,
    SkeletonModule, TerrainAPI, TerrainModule, TextureAPI, TextureModule,
};

pub trait ResourceAPI:
    AudioAPI + TextureAPI + MeshAPI + MaterialAPI + SkeletonAPI + TerrainAPI + Send + Sync
{
}
impl<T> ResourceAPI for T where
    T: AudioAPI + TextureAPI + MeshAPI + MaterialAPI + SkeletonAPI + TerrainAPI + Send + Sync
{
}

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
    pub fn Audio(&self) -> AudioModule<'_, R> {
        AudioModule::new(self.api)
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
    pub fn Skeletons(&self) -> SkeletonModule<'_, R> {
        SkeletonModule::new(self.api)
    }

    #[inline]
    pub fn Terrain(&self) -> TerrainModule<'_, R> {
        TerrainModule::new(self.api)
    }
}
