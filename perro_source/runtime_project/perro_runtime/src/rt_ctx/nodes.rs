//! Runtime node API implementation.
//!
//! API methods stay here. Node creation prep, UI dirty classification, and
//! small helper scans live in `nodes/helpers.rs`.

use perro_ids::{IntoTagID, MaterialID, NodeID};
use perro_nodes::{
    CameraProjection, Node2D, Node3D, NodeBaseDispatch, NodeType, NodeTypeDispatch, Renderable,
    SceneNode, SceneNodeData, Spatial, UiNode,
};
use perro_runtime_api::sub_apis::{
    CameraRay3D, IntoNodeCreateBatch, IntoNodeTag, IntoNodeTags, MeshDataSurfaceHit3D,
    MeshDataSurfaceRegion3D, MeshMaterialRegion3D, MeshSurfaceHit3D, MeshSurfaceRay3D, NodeAPI,
    NodeCollection, NodeCollectionEntry, NodeCreateBatch, NodeQueryView, NodeScriptSpec,
    NodeScriptVar, NodeSpec, QueryExpr, QueryScope, ScriptAPI,
};
use perro_structs::{Transform2D, Transform3D, Vector2, Vector3};
use rayon::prelude::*;
use std::borrow::Cow;

use crate::Runtime;
use crate::runtime::state::{DirtyState, TransformRuntimeState};

mod helpers;
use helpers::*;

const SPATIAL_INVERSE_SCALE_EPSILON: f32 = 1.0e-5;

/// Below this slot count the spatial fill runs single-threaded.
const QUERY_SPATIAL_PAR_MIN_SLOTS: usize = 10_000;

/// Candidate-restricted fill only pays off once the candidate set is a small
/// fraction of the arena; otherwise touching every candidate one-by-one
/// (random slot order, no prefetch) loses to the cache-friendly linear
/// whole-arena walk. Picked so a candidate set under ~1/8 of the arena
/// switches to the restricted path.
const QUERY_SPATIAL_CANDIDATE_FILL_DIVISOR: usize = 8;

/// Lock-free read of a clean cached global 2D position. `None` means the
/// cache is stale or missing and the caller must use the full getter.
#[inline]
fn read_clean_global_pos_2d(
    transforms: &TransformRuntimeState,
    dirty: &DirtyState,
    id: NodeID,
    index: usize,
) -> Option<Vector2> {
    if transforms
        .global_transform_2d_valid
        .get(index)
        .copied()
        .unwrap_or(0)
        == 0
    {
        return None;
    }
    if transforms
        .global_transform_2d_generation
        .get(index)
        .copied()
        .unwrap_or(u32::MAX)
        != id.generation()
    {
        return None;
    }
    if dirty.has_transform_dirty(id, perro_nodes::Spatial::TwoD) {
        return None;
    }
    transforms
        .global_transform_2d
        .get(index)
        .map(|transform| transform.position)
}

/// Lock-free read of a clean cached global 3D position. `None` means the
/// cache is stale or missing and the caller must use the full getter.
#[inline]
fn read_clean_global_pos_3d(
    transforms: &TransformRuntimeState,
    dirty: &DirtyState,
    id: NodeID,
    index: usize,
) -> Option<Vector3> {
    if transforms
        .global_transform_3d_valid
        .get(index)
        .copied()
        .unwrap_or(0)
        == 0
    {
        return None;
    }
    if transforms
        .global_transform_3d_generation
        .get(index)
        .copied()
        .unwrap_or(u32::MAX)
        != id.generation()
    {
        return None;
    }
    if dirty.has_transform_dirty(id, perro_nodes::Spatial::ThreeD) {
        return None;
    }
    transforms
        .global_transform_3d
        .get(index)
        .map(|transform| transform.position)
}

#[inline]
fn inverse_basis_mat4(transform: Transform3D) -> glam::Mat4 {
    let mut safe = transform;
    if safe.scale.x.abs() <= SPATIAL_INVERSE_SCALE_EPSILON {
        safe.scale.x = 1.0;
    }
    if safe.scale.y.abs() <= SPATIAL_INVERSE_SCALE_EPSILON {
        safe.scale.y = 1.0;
    }
    if safe.scale.z.abs() <= SPATIAL_INVERSE_SCALE_EPSILON {
        safe.scale.z = 1.0;
    }
    safe.to_mat4().inverse()
}

mod camera_streams;
mod node_api;
mod relations;
mod spatial;
mod specs;

#[cfg(test)]
#[path = "../../tests/unit/rt_ctx_nodes_transform_api_tests.rs"]
mod tests;
