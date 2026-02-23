use perro_ids::MaterialID;
use perro_render_bridge::Material3D;

pub trait MaterialAPI {
    fn load_material_source(&self, source: &str) -> MaterialID;
    fn create_material(&self, material: Material3D) -> MaterialID;
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
}
