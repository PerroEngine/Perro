use perro_asset_formats::ptset::{MAGIC as TILESET2D_MAGIC, VERSION as TILESET2D_VERSION};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
pub use perro_particle_math::Op as ParticleExprOp2D;
pub use perro_particle_math::Op as ParticleExprOp3D;
use perro_structs::{
    Color, ColorBlindFilter, DrawShape2D, PostProcessEffect, PostProcessSet, UnitVector4,
};
pub use perro_structs::{HdrColorSpace, HdrFallback, HdrMode, HdrStatus};
use std::borrow::Cow;
use std::sync::Arc;

mod commands;
mod request;
mod three_d;
mod two_d;
mod ui;

pub use commands::*;
pub use request::*;
pub use three_d::*;
pub use two_d::*;
pub use ui::*;

/// Length-zero `Arc<[T]>` sharing. `Arc::from([])` still heap-allocates the
/// `ArcInner` header on every call, so hot paths that emit empty payloads each
/// frame reuse one static empty slice per element type instead.
pub trait EmptyArcSlice: Sized + Send + Sync + 'static {
    fn empty_arc_slice() -> Arc<[Self]>;
}

macro_rules! impl_empty_arc_slice {
    ($($ty:ty),* $(,)?) => {$(
        impl EmptyArcSlice for $ty {
            fn empty_arc_slice() -> Arc<[Self]> {
                static EMPTY: std::sync::OnceLock<Arc<[$ty]>> = std::sync::OnceLock::new();
                EMPTY.get_or_init(|| Arc::from([])).clone()
            }
        }
    )*};
}

impl_empty_arc_slice!(
    f32,
    NodeID,
    PostProcessEffect,
    Sprite2DCommand,
    ShadowCaster2DState,
    Light2DState,
    CameraStreamDraw3DState,
    (NodeID, PointParticles2DState),
    (NodeID, Water2DState),
    (NodeID, PointParticles3DState),
    (NodeID, Water3DState),
    WaterCoastlineShape2D,
    WaterCoastlineShape3D,
    WaterBodyQueryState,
    WaterImpact2D,
    WaterImpact3D,
    WaterLinkState,
);

/// Refcount-shared empty `Arc<[T]>`; see [`EmptyArcSlice`].
pub fn empty_arc_slice<T: EmptyArcSlice>() -> Arc<[T]> {
    T::empty_arc_slice()
}

/// `Vec` -> `Arc<[T]>` that routes the (common) empty case through the shared
/// empty slice instead of allocating a zero-length `ArcInner`.
pub fn arc_slice_from_vec<T: EmptyArcSlice>(vec: Vec<T>) -> Arc<[T]> {
    if vec.is_empty() {
        empty_arc_slice()
    } else {
        Arc::from(vec)
    }
}

#[cfg(test)]
mod tests;
