use ahash::{AHashMap, AHashSet};
use perro_asset_formats::pmesh::{
    FLAG_INDEX_U16 as PMESH_FLAG_INDEX_U16, FLAG_PAYLOAD_RAW as PMESH_FLAG_PAYLOAD_RAW,
    VERSION as PMESH_VERSION,
};
use perro_ids::{NodeID, parse_hashed_source_uri, string_to_u64};
use perro_io::{decompress_zlib, load_asset};
use perro_nodes::{
    CollisionShape2D, CollisionShape3D, Shape2D, Shape3D, TileMap2D, Triangle2DKind,
};
use perro_render_bridge::{
    TileSet2D as ParsedTileset2D, TileSetCollisionShape2D as ParsedTileCollisionShape2D,
    TileSetShape2D,
};
use perro_runtime_context::sub_apis::{PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D};
use perro_structs::{BitMask, Transform2D, Transform3D, Vector2, Vector3};

use crate::{
    BodyDesc2D, BodyDesc3D, BodyKind, JointDesc2D, JointDesc3D, JointKind2D, JointKind3D,
    PhysicsWorld2D, PhysicsWorld3D, ShapeDesc2D, ShapeDesc3D, ShapeKind2D, ShapeKind3D,
    TriMeshData, na2, na3, r2, r3,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PhysicsProviderMode {
    Dynamic,
    Static,
}

pub type StaticBytesLookup = fn(u64) -> &'static [u8];

#[derive(Clone, Copy, Debug)]
pub struct PhysicsAssetContext {
    pub provider_mode: PhysicsProviderMode,
    pub static_mesh_lookup: Option<StaticBytesLookup>,
    pub static_collision_trimesh_lookup: Option<StaticBytesLookup>,
}

mod audio;
mod geometry;
mod hashing;
mod joints;
mod shapes;
mod trimesh;

pub use audio::*;
pub use geometry::*;
pub use hashing::*;
pub use joints::*;
pub use shapes::*;
pub use trimesh::*;
