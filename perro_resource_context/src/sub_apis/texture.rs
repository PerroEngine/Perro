use perro_ids::TextureID;

pub trait TextureAPI {
    fn load_texture(&self, source: &str) -> TextureID;
}

pub struct TextureModule<'res, R: TextureAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: TextureAPI + ?Sized> TextureModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: AsRef<str>>(&self, source: S) -> TextureID {
        self.api.load_texture(source.as_ref())
    }
}

#[macro_export]
macro_rules! load_texture {
    ($res:expr, $source:expr) => {
        $res.Textures().load($source)
    };
}
