use crate::ResPathSource;
use perro_ids::TextureID;

pub trait TextureAPI {
    fn load_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID;
    fn reserve_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID;
    fn reserve_texture_id(&self, id: TextureID) -> bool;
    fn load_texture(&self, source: &str) -> TextureID {
        self.load_texture_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_texture(&self, source: &str) -> TextureID {
        self.reserve_texture_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_texture(&self, id: TextureID) -> bool;
    fn is_texture_loaded(&self, id: TextureID) -> bool;
}

pub trait TextureReserveArg<R: TextureAPI + ?Sized> {
    type Output;
    fn reserve_with(self, api: &R) -> Self::Output;
}

impl<R, S> TextureReserveArg<R> for S
where
    R: TextureAPI + ?Sized,
    S: ResPathSource,
{
    type Output = TextureID;

    #[inline]
    fn reserve_with(self, api: &R) -> Self::Output {
        api.reserve_texture(self.as_res_path_str())
    }
}

impl<R> TextureReserveArg<R> for TextureID
where
    R: TextureAPI + ?Sized,
{
    type Output = TextureID;

    #[inline]
    fn reserve_with(self, api: &R) -> Self::Output {
        if api.reserve_texture_id(self) {
            self
        } else {
            TextureID::nil()
        }
    }
}

impl<R> TextureReserveArg<R> for &TextureID
where
    R: TextureAPI + ?Sized,
{
    type Output = TextureID;

    #[inline]
    fn reserve_with(self, api: &R) -> Self::Output {
        (*self).reserve_with(api)
    }
}

pub struct TextureModule<'res, R: TextureAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: TextureAPI + ?Sized> TextureModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load<S: ResPathSource>(&self, source: S) -> TextureID {
        self.api.load_texture(source.as_res_path_str())
    }

    #[inline]
    pub fn load_hashed(&self, source_hash: u64) -> TextureID {
        self.api.load_texture_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> TextureID {
        self.api
            .load_texture_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn reserve<A>(&self, arg: A) -> A::Output
    where
        A: TextureReserveArg<R>,
    {
        arg.reserve_with(self.api)
    }

    #[inline]
    pub fn reserve_hashed(&self, source_hash: u64) -> TextureID {
        self.api.reserve_texture_hashed(source_hash, None)
    }

    #[inline]
    pub fn reserve_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> TextureID {
        self.api
            .reserve_texture_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn drop(&self, id: TextureID) -> bool {
        self.api.drop_texture(id)
    }

    #[inline]
    pub fn is_loaded(&self, id: TextureID) -> bool {
        self.api.is_texture_loaded(id)
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

#[macro_export]
macro_rules! texture_is_loaded {
    ($res:expr, $id:expr) => {
        $res.Textures().is_loaded($id)
    };
}
