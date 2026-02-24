use perro_ids::MeshID;

pub trait MeshAPI {
    fn load_mesh(&self, source: &str) -> MeshID;
    fn reserve_mesh(&self, source: &str) -> MeshID;
    fn drop_mesh(&self, source: &str) -> bool;
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
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> MeshID {
        self.api.reserve_mesh(source.as_ref())
    }

    #[inline]
    pub fn drop<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.drop_mesh(source.as_ref())
    }
}

#[macro_export]
macro_rules! load_mesh {
    ($res:expr, $source:expr) => {
        $res.Meshes().load($source)
    };
}
