use perro_ids::MeshID;

pub trait MeshAPI {
    fn load_mesh_hashed(&self, source_hash: u64, source: Option<&str>) -> MeshID;
    fn reserve_mesh_hashed(&self, source_hash: u64, source: Option<&str>) -> MeshID;
    fn load_mesh(&self, source: &str) -> MeshID {
        self.load_mesh_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_mesh(&self, source: &str) -> MeshID {
        self.reserve_mesh_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_mesh(&self, id: MeshID) -> bool;
}

pub struct MeshModule<'res, R: MeshAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: MeshAPI + ?Sized> MeshModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: AsRef<str>>(&self, source: S) -> MeshID {
        self.api.load_mesh(source.as_ref())
    }

    #[inline]
    pub fn load_hashed(&self, source_hash: u64) -> MeshID {
        self.api.load_mesh_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source(&self, source_hash: u64, source: &str) -> MeshID {
        self.api.load_mesh_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> MeshID {
        self.api.reserve_mesh(source.as_ref())
    }

    #[inline]
    pub fn reserve_hashed(&self, source_hash: u64) -> MeshID {
        self.api.reserve_mesh_hashed(source_hash, None)
    }

    #[inline]
    pub fn reserve_hashed_with_source(&self, source_hash: u64, source: &str) -> MeshID {
        self.api.reserve_mesh_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn drop(&self, id: MeshID) -> bool {
        self.api.drop_mesh(id)
    }
}

#[macro_export]
macro_rules! mesh_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Meshes().load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Meshes().load($source)
    };
}

#[macro_export]
macro_rules! mesh_reserve {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Meshes().reserve_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Meshes().reserve($source)
    };
}

#[macro_export]
macro_rules! mesh_drop {
    ($res:expr, $id:expr) => {
        $res.Meshes().drop($id)
    };
}
