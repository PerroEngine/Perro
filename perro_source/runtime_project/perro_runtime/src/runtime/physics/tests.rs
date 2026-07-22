use super::*;
use crate::runtime::render_2d::{
    ParsedTile2D, ParsedTileCollisionShape2D, ParsedTileset2D, TileSetShape2D,
};
use perro_nodes::{
    Area2D, Area3D, CharacterBody2D, CharacterBody3D, CollisionShape2D, CollisionShape3D,
    FixedJoint2D, FixedJoint3D, MeshInstance3D, RigidBody2D, RigidBody3D, Sprite2D, StaticBody2D,
    StaticBody3D, UiSubView, WaterBody2D, WaterBody3D, WaterIdleMode, WaterShape,
    WaterSurfaceParams,
};
use perro_runtime_api::sub_apis::PhysicsAPI;
use perro_structs::CollisionPolicy;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1.0e-4
}

fn quat_y(angle: f32) -> Quaternion {
    Quaternion::from_euler_xyz(0.0, angle, 0.0)
}

// SoA writeback (sync_world_to_nodes): nested rigid body back-solves local
// frm rapier global thru parent offset. guards nested slow path.

// SoA writeback stages poses then sorts by slot; guard no cross-contam:
// each body keeps its own velocity + moves per its own vx.

// ================= Fix 1: move_body fast-path sync =================

/// helper: floor + char above it. ret (char_id, floor_id).
fn char_over_floor_3d(runtime: &mut Runtime) -> (NodeID, NodeID) {
    let floor_id = NodeAPI::create::<StaticBody3D>(runtime);
    let floor_shape = NodeAPI::create::<CollisionShape3D>(runtime);
    assert!(NodeAPI::reparent(runtime, floor_id, floor_shape));
    if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
        && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
    {
        shape.shape = Shape3D::Cube {
            size: Vector3::new(20.0, 1.0, 20.0),
        };
    }
    let char_id = NodeAPI::create::<CharacterBody3D>(runtime);
    let char_shape = NodeAPI::create::<CollisionShape3D>(runtime);
    assert!(NodeAPI::reparent(runtime, char_id, char_shape));
    assert!(NodeAPI::set_global_transform_3d(
        runtime,
        char_id,
        Transform3D::new(
            Vector3::new(0.0, 3.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE
        ),
    ));
    (char_id, floor_id)
}

// ================= Fix 2: physics-scoped dirty gate =================

include!("tests/interpolation.rs");
include!("tests/bodies.rs");
include!("tests/water.rs");
include!("tests/joints_signals.rs");
