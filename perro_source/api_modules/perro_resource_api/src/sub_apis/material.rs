//! Material resource API.
//!
//! Loads, reserves, creates, reads, writes, drops, and checks material resources.

use crate::ResPathSource;
use perro_ids::MaterialID;
use perro_render_bridge::Material3D;

pub trait MaterialAPI {
    fn load_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID;
    fn create_material(&self, material: Material3D) -> MaterialID;
    fn get_material_data(&self, id: MaterialID) -> Option<Material3D>;
    fn write_material_data(&self, id: MaterialID, material: Material3D) -> bool;
    fn is_material_loaded(&self, id: MaterialID) -> bool;
    fn reserve_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID;
    fn reserve_material_id(&self, id: MaterialID) -> bool;
    fn load_material_source(&self, source: &str) -> MaterialID {
        self.load_material_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_material_source(&self, source: &str) -> MaterialID {
        self.reserve_material_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_material_source(&self, id: MaterialID) -> bool;
}

pub trait MaterialReserveArg<R: MaterialAPI + ?Sized> {
    type Output;
    fn reserve_with(self, api: &R) -> Self::Output;
}

impl<R, S> MaterialReserveArg<R> for S
where
    R: MaterialAPI + ?Sized,
    S: ResPathSource,
{
    type Output = MaterialID;

    #[inline]
    fn reserve_with(self, api: &R) -> Self::Output {
        api.reserve_material_source(self.as_res_path_str())
    }
}

impl<R> MaterialReserveArg<R> for MaterialID
where
    R: MaterialAPI + ?Sized,
{
    type Output = MaterialID;

    #[inline]
    fn reserve_with(self, api: &R) -> Self::Output {
        if api.reserve_material_id(self) {
            self
        } else {
            MaterialID::nil()
        }
    }
}

impl<R> MaterialReserveArg<R> for &MaterialID
where
    R: MaterialAPI + ?Sized,
{
    type Output = MaterialID;

    #[inline]
    fn reserve_with(self, api: &R) -> Self::Output {
        (*self).reserve_with(api)
    }
}

pub struct MaterialModule<'res, R: MaterialAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: MaterialAPI + ?Sized> MaterialModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: ResPathSource>(&self, source: S) -> MaterialID {
        self.api.load_material_source(source.as_res_path_str())
    }

    #[inline]
    pub fn load_hashed(&self, source_hash: u64) -> MaterialID {
        self.api.load_material_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> MaterialID {
        self.api
            .load_material_source_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn create(&self, material: Material3D) -> MaterialID {
        self.api.create_material(material)
    }

    #[inline]
    pub fn get_data(&self, id: MaterialID) -> Option<Material3D> {
        self.api.get_material_data(id)
    }

    #[inline]
    pub fn write(&self, id: MaterialID, material: Material3D) -> bool {
        self.api.write_material_data(id, material)
    }

    #[inline]
    pub fn is_loaded(&self, id: MaterialID) -> bool {
        self.api.is_material_loaded(id)
    }

    #[inline]
    pub fn reserve<A>(&self, arg: A) -> A::Output
    where
        A: MaterialReserveArg<R>,
    {
        arg.reserve_with(self.api)
    }

    #[inline]
    pub fn reserve_hashed(&self, source_hash: u64) -> MaterialID {
        self.api.reserve_material_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn reserve_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> MaterialID {
        self.api
            .reserve_material_source_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn drop(&self, id: MaterialID) -> bool {
        self.api.drop_material_source(id)
    }
}

#[macro_export]
macro_rules! material_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Materials().load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Materials().load($source)
    };
}

#[macro_export]
macro_rules! material_reserve {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Materials().reserve_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Materials().reserve($source)
    };
}

#[macro_export]
macro_rules! material_drop {
    ($res:expr, $id:expr) => {
        $res.Materials().drop($id)
    };
}

#[macro_export]
macro_rules! material_create {
    ($res:expr, $material:expr) => {
        $res.Materials().create($material)
    };
}

#[macro_export]
macro_rules! material_get_data {
    ($res:expr, $id:expr) => {
        $res.Materials().get_data($id)
    };
}

#[macro_export]
macro_rules! material_write {
    ($res:expr, $id:expr, $material:expr) => {
        $res.Materials().write($id, $material)
    };
}

#[macro_export]
macro_rules! material_is_loaded {
    ($res:expr, $id:expr) => {
        $res.Materials().is_loaded($id)
    };
}
