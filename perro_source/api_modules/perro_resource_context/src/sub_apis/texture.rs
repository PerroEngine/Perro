use perro_ids::TextureID;

pub trait TextureAPI {
    fn load_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID;
    fn reserve_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID;
    fn load_texture(&self, source: &str) -> TextureID {
        self.load_texture_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_texture(&self, source: &str) -> TextureID {
        self.reserve_texture_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_texture(&self, id: TextureID) -> bool;
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
    pub fn load_hashed(&self, source_hash: u64) -> TextureID {
        self.api.load_texture_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source(&self, source_hash: u64, source: &str) -> TextureID {
        self.api.load_texture_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn reserve<S: AsRef<str>>(&self, source: S) -> TextureID {
        self.api.reserve_texture(source.as_ref())
    }

    #[inline]
    pub fn reserve_hashed(&self, source_hash: u64) -> TextureID {
        self.api.reserve_texture_hashed(source_hash, None)
    }

    #[inline]
    pub fn reserve_hashed_with_source(&self, source_hash: u64, source: &str) -> TextureID {
        self.api.reserve_texture_hashed(source_hash, Some(source))
    }

    #[inline]
    pub fn drop(&self, id: TextureID) -> bool {
        self.api.drop_texture(id)
    }
}

#[macro_export]
macro_rules! texture_load {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Textures().load_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Textures().load($source)
    };
}

#[macro_export]
macro_rules! texture_reserve {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Textures().reserve_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Textures().reserve($source)
    };
}

#[macro_export]
macro_rules! texture_drop {
    ($res:expr, $id:expr) => {
        $res.Textures().drop($id)
    };
}
