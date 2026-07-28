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
    fn save_texture_image(&self, _id: TextureID, _path: &str) -> bool {
        false
    }
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
    fn camera_texture(&self, stream_node: NodeID) -> TextureID {
        self.camera_stream_texture(stream_node)
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

    /// Queue a texture save to PNG, JPEG, WebP, BMP, TGA, or ICO.
    ///
    /// GPU-only textures, including camera output, use async readback. `true`
    /// means the save was queued, not that disk I/O has finished.
    #[inline]
    pub fn save_image<P: ResPathSource>(&self, id: TextureID, path: P) -> bool {
        self.api.save_texture_image(id, path.as_res_path_str())
    }

    #[inline]
    pub fn is_loaded(&self, id: TextureID) -> bool {
        self.api.is_texture_loaded(id)
    }

    #[inline]
    pub fn camera_stream(&self, stream_node: NodeID) -> TextureID {
        self.api.camera_stream_texture(stream_node)
    }

    /// Return live output texture for a camera-stream node.
    ///
    /// Pass a `CameraStream2D`, `CameraStream3D`, or `UiCameraStream` node ID.
    /// Assign the result to an image, sprite, or material.
    #[inline]
    pub fn camera(&self, stream_node: NodeID) -> TextureID {
        self.api.camera_texture(stream_node)
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

/// Queue a texture save to an image file.
#[macro_export]
macro_rules! texture_save_image {
    ($res:expr, $id:expr, $path:expr) => {
        $res.Textures().save_image($id, $path)
    };
}

/// Return a camera-stream node's live output texture.
#[macro_export]
macro_rules! camera_texture {
    ($res:expr, $stream_node:expr) => {
        $res.Textures().camera($stream_node)
    };
}

/// Return a camera-stream node's live output texture.
#[macro_export]
macro_rules! camera_stream_texture {
    ($res:expr, $stream_node:expr) => {
        $res.Textures().camera_stream($stream_node)
    };
}

/// Assign a camera-stream node's live output to a `UiImage`.
#[macro_export]
macro_rules! camera_to_image {
    ($ctx:expr, $stream_node:expr, $image_node:expr) => {{
        let __texture = $ctx.res.Textures().camera($stream_node);
        $ctx.run
            .Nodes()
            .with_node_mut::<$crate::__PerroUiImage, _, _>($image_node, |__image| {
                __image.texture = __texture;
            })
            .is_some()
    }};
}

/// Queue a `Camera2D` or `Camera3D` save at viewport resolution.
#[macro_export]
macro_rules! camera_save_image {
    ($ctx:expr, $camera_node:expr, $path:expr) => {
        $ctx.run.Nodes().save_camera_image($camera_node, $path)
    };
}

/// Queue a `Camera2D` or `Camera3D` save at an explicit resolution.
#[macro_export]
macro_rules! camera_save_image_sized {
    ($ctx:expr, $camera_node:expr, $path:expr, $width:expr, $height:expr) => {
        $ctx.run
            .Nodes()
            .save_camera_image_sized($camera_node, $path, $width, $height)
    };
}

/// Queue an existing camera-stream output save to an image file.
#[macro_export]
macro_rules! camera_stream_save_image {
    ($res:expr, $stream_node:expr, $path:expr) => {{
        let __texture = $res.Textures().camera($stream_node);
        $res.Textures().save_image(__texture, $path)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TextureApiStub;

    impl TextureAPI for TextureApiStub {
        fn load_texture_hashed(&self, _: u64, _: Option<&str>) -> TextureID {
            TextureID::nil()
        }

        fn reserve_texture_hashed(&self, _: u64, _: Option<&str>) -> TextureID {
            TextureID::nil()
        }

        fn reserve_texture_id(&self, _: TextureID) -> bool {
            false
        }

        fn create_texture_from_bytes(&self, _: &[u8]) -> TextureID {
            TextureID::nil()
        }

        fn create_texture_from_rgba(&self, _: u32, _: u32, _: &[u8]) -> TextureID {
            TextureID::nil()
        }

        fn write_texture_rgba(&self, _: TextureID, _: u32, _: u32, _: &[u8]) -> bool {
            false
        }

        fn write_texture_rgba_region(
            &self,
            _: TextureID,
            _: u32,
            _: u32,
            _: u32,
            _: u32,
            _: &[u8],
        ) -> bool {
            false
        }

        fn save_texture_image(&self, _: TextureID, _: &str) -> bool {
            true
        }

        fn drop_texture(&self, _: TextureID) -> bool {
            false
        }

        fn is_texture_loaded(&self, _: TextureID) -> bool {
            false
        }
    }

    struct ResourceStub(TextureApiStub);

    #[allow(non_snake_case)]
    impl ResourceStub {
        fn Textures(&self) -> TextureModule<'_, TextureApiStub> {
            TextureModule::new(&self.0)
        }
    }

    struct NodeModuleStub;

    impl NodeModuleStub {
        fn with_node_mut<T, V, F>(&mut self, _: NodeID, f: F) -> Option<V>
        where
            T: Default,
            F: FnOnce(&mut T) -> V,
        {
            Some(f(&mut T::default()))
        }

        fn save_camera_image(&mut self, _: NodeID, _: &str) -> bool {
            true
        }

        fn save_camera_image_sized(&mut self, _: NodeID, _: &str, _: u32, _: u32) -> bool {
            true
        }
    }

    struct RuntimeStub;

    #[allow(non_snake_case)]
    impl RuntimeStub {
        fn Nodes(&mut self) -> NodeModuleStub {
            NodeModuleStub
        }
    }

    struct ContextStub {
        res: ResourceStub,
        run: RuntimeStub,
    }

    #[test]
    fn camera_output_keeps_stream_node_identity() {
        let res = ResourceStub(TextureApiStub);
        let stream = NodeID::from_parts(42, 7);
        let expected = TextureID::from_parts(42, 7);

        assert_eq!(res.Textures().camera(stream), expected);
        assert_eq!(camera_texture!(res, stream), expected);
        assert_eq!(camera_stream_texture!(res, stream), expected);
        assert!(texture_save_image!(res, expected, "user://shot.png"));
        assert!(camera_stream_save_image!(res, stream, "user://camera.png"));
    }

    #[test]
    fn camera_to_image_macro_assigns_ui_image() {
        let mut ctx = ContextStub {
            res: ResourceStub(TextureApiStub),
            run: RuntimeStub,
        };

        assert!(camera_to_image!(
            ctx,
            NodeID::from_parts(42, 7),
            NodeID::from_parts(8, 1)
        ));
        assert!(camera_save_image!(
            ctx,
            NodeID::from_parts(3, 1),
            "user://camera.png"
        ));
        assert!(camera_save_image_sized!(
            ctx,
            NodeID::from_parts(3, 1),
            "user://camera.png",
            1920,
            1080
        ));
    }
}
