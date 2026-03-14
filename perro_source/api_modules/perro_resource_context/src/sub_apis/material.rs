use perro_ids::MaterialID;
use perro_render_bridge::Material3D;

pub trait MaterialAPI {
    fn load_material_source(&self, source: &str) -> MaterialID;
    fn create_material(&self, material: Material3D) -> MaterialID;
    fn reserve_material_source(&self, source: &str) -> MaterialID;
    fn drop_material_source(&self, source: &str) -> bool;
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
    pub fn create(&self, material: Material3D) -> MaterialID {
        self.api.create_material(material)
    }

    #[inline]
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> MaterialID {
        self.api.reserve_material_source(source.as_ref())
    }

    #[inline]
    pub fn drop<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.drop_material_source(source.as_ref())
    }
}

#[macro_export]
macro_rules! material_load {
    ($res:expr, $source:expr) => {
        $res.Materials().load($source)
    };
}

#[macro_export]
macro_rules! material_reserve {
    ($res:expr, $source:expr) => {
        $res.Materials().reserve($source)
    };
}

#[macro_export]
macro_rules! material_drop {
    ($res:expr, $source:expr) => {
        $res.Materials().drop($source)
    };
}

#[macro_export]
macro_rules! material_create {
    ($res:expr, $material:expr) => {
        $res.Materials().create($material)
    };
}
