use perro_ids::TextureID;

pub trait TextureAPI {
    fn load_texture(&self, source: &str) -> TextureID;
    fn reserve_texture(&self, source: &str) -> TextureID;
    fn drop_texture(&self, source: &str) -> bool;
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

    #[inline]
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> TextureID {
        self.api.reserve_texture(source.as_ref())
    }

    #[inline]
    pub fn drop<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.drop_texture(source.as_ref())
    }
}

#[macro_export]
macro_rules! texture_load {
    ($res:expr, $source:expr) => {
        $res.Textures().load($source)
    };
}

#[macro_export]
macro_rules! texture_reserve {
    ($res:expr, $source:expr) => {
        $res.Textures().reserve($source)
    };
}

#[macro_export]
macro_rules! texture_drop {
    ($res:expr, $source:expr) => {
        $res.Textures().drop($source)
    };
}
