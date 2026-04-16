use perro_ids::MaterialID;
use perro_render_bridge::Material3D;

pub trait MaterialAPI {
    fn load_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID;
    fn create_material(&self, material: Material3D) -> MaterialID;
    fn reserve_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID;
    fn load_material_source(&self, source: &str) -> MaterialID {
        self.load_material_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_material_source(&self, source: &str) -> MaterialID {
        self.reserve_material_source_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_material_source(&self, id: MaterialID) -> bool;
}

pub struct MaterialModule<'res, R: MaterialAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: MaterialAPI + ?Sized> MaterialModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: AsRef<str>>(&self, source: S) -> MaterialID {
        self.api.load_material_source(source.as_ref())
    }

    #[inline]
    pub fn load_hashed(&self, source_hash: u64) -> MaterialID {
        self.api.load_material_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source(&self, source_hash: u64, source: &str) -> MaterialID {
        self.api
            .load_material_source_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn create(&self, material: Material3D) -> MaterialID {
        self.api.create_material(material)
    }

    #[inline]
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> MaterialID {
        self.api.reserve_material_source(source.as_ref())
    }

    #[inline]
    pub fn reserve_hashed(&self, source_hash: u64) -> MaterialID {
        self.api.reserve_material_source_hashed(source_hash, None)
    }

    #[inline]
    pub fn reserve_hashed_with_source(&self, source_hash: u64, source: &str) -> MaterialID {
        self.api
            .reserve_material_source_hashed(source_hash, Some(source))
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
