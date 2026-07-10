//! Texture resource API.
//!
//! Loads, reserves, drops, and checks texture resources.

use crate::ResPathSource;
use perro_ids::{NodeID, TextureID, WebcamID};

pub trait TextureAPI {
    fn load_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID;
    fn reserve_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID;
    fn reserve_texture_id(&self, id: TextureID) -> bool;
    fn create_texture_from_bytes(&self, bytes: &[u8]) -> TextureID;
    fn create_texture_from_rgba(&self, width: u32, height: u32, rgba: &[u8]) -> TextureID;
    fn write_texture_rgba(&self, id: TextureID, width: u32, height: u32, rgba: &[u8]) -> bool;
    fn write_texture_rgba_region(
        &self,
        id: TextureID,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool;
    fn load_texture(&self, source: &str) -> TextureID {
        self.load_texture_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn reserve_texture(&self, source: &str) -> TextureID {
        self.reserve_texture_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn drop_texture(&self, id: TextureID) -> bool;
    fn is_texture_loaded(&self, id: TextureID) -> bool;
    fn camera_stream_texture(&self, stream_node: NodeID) -> TextureID {
        TextureID::from_parts(stream_node.index(), stream_node.generation())
    }
    fn webcam_texture(&self, webcam: WebcamID) -> TextureID {
        TextureID::from_parts(webcam.index(), webcam.generation())
    }
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
    pub fn create_from_rgba(&self, width: u32, height: u32, rgba: &[u8]) -> TextureID {
        self.api.create_texture_from_rgba(width, height, rgba)
    }

    #[inline]
    pub fn create_from_bytes(&self, bytes: &[u8]) -> TextureID {
        self.api.create_texture_from_bytes(bytes)
    }

    #[inline]
    pub fn write_rgba(&self, id: TextureID, width: u32, height: u32, rgba: &[u8]) -> bool {
        self.api.write_texture_rgba(id, width, height, rgba)
    }

    #[inline]
    pub fn write_rgba_region(
        &self,
        id: TextureID,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        self.api
            .write_texture_rgba_region(id, x, y, width, height, rgba)
    }

    #[inline]
    pub fn is_loaded(&self, id: TextureID) -> bool {
        self.api.is_texture_loaded(id)
    }

    #[inline]
    pub fn camera_stream(&self, stream_node: NodeID) -> TextureID {
        self.api.camera_stream_texture(stream_node)
    }

    #[inline]
    pub fn webcam(&self, webcam: WebcamID) -> TextureID {
        self.api.webcam_texture(webcam)
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
macro_rules! texture_create_from_rgba {
    ($res:expr, $width:expr, $height:expr, $rgba:expr) => {
        $res.Textures().create_from_rgba($width, $height, $rgba)
    };
}

#[macro_export]
macro_rules! texture_create_from_bytes {
    ($res:expr, $bytes:expr) => {
        $res.Textures().create_from_bytes($bytes)
    };
}

#[macro_export]
macro_rules! texture_write_rgba {
    ($res:expr, $id:expr, $width:expr, $height:expr, $rgba:expr) => {
        $res.Textures().write_rgba($id, $width, $height, $rgba)
    };
}

#[macro_export]
macro_rules! texture_write_rgba_region {
    ($res:expr, $id:expr, $x:expr, $y:expr, $width:expr, $height:expr, $rgba:expr) => {
        $res.Textures()
            .write_rgba_region($id, $x, $y, $width, $height, $rgba)
    };
}

#[macro_export]
macro_rules! texture_is_loaded {
    ($res:expr, $id:expr) => {
        $res.Textures().is_loaded($id)
    };
}
