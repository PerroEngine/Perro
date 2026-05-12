use perro_ids::NodeID;
use perro_nodes::{Shape2D, Shape3D};
use perro_runtime_context::sub_apis::{PhysicsRayHit2D, PhysicsRayHit3D};
use perro_structs::{BitMask, Transform2D, Transform3D, Vector2, Vector3};

use crate::na3;

pub type TriMeshData = (Vec<na3::Point3<f32>>, Vec<[u32; 3]>);

#[derive(Clone, Copy, Debug)]
pub enum AudioRaycastInput {
    TwoD {
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        mask: BitMask,
    },
    ThreeD {
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub enum AudioRaycastResult {
    #[default]
    None,
    TwoD(Option<PhysicsRayHit2D>),
    ThreeD(Option<PhysicsRayHit3D>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Static,
    Area,
    Rigid,
}

#[derive(Clone, Debug)]
pub struct ShapeDesc2D {
    pub local: Transform2D,
    pub shape: ShapeKind2D,
    pub sensor: bool,
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub friction: f32,
    pub restitution: f32,
}

#[derive(Clone, Debug)]
pub enum ShapeKind2D {
    Primitive(Shape2D),
    Polygon(Vec<Vector2>),
}

#[derive(Clone, Debug)]
pub struct ShapeDesc3D {
    pub local: Transform3D,
    pub shape: ShapeKind3D,
    pub sensor: bool,
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub friction: f32,
    pub restitution: f32,
}

#[derive(Clone, Debug)]
pub enum ShapeKind3D {
    Primitive(Shape3D),
    TriMesh { source: String },
}

#[derive(Clone, Debug)]
pub struct BodyDesc2D {
    pub id: NodeID,
    pub kind: BodyKind,
    pub enabled: bool,
    pub global: Transform2D,
    pub rigid: Option<RigidProps2D>,
    pub shape_signature: u64,
    pub shapes: Vec<ShapeDesc2D>,
}

#[derive(Clone, Debug)]
pub struct BodyDesc3D {
    pub id: NodeID,
    pub kind: BodyKind,
    pub enabled: bool,
    pub global: Transform3D,
    pub rigid: Option<RigidProps3D>,
    pub shape_signature: u64,
    pub shapes: Vec<ShapeDesc3D>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointKind2D {
    Pin,
    Distance { min: f32, max: f32 },
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointKind3D {
    Ball,
    Hinge { axis: Vector3 },
    Fixed,
}

#[derive(Clone, Copy, Debug)]
pub struct JointDesc2D {
    pub id: NodeID,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector2,
    pub anchor_b: Vector2,
    pub enabled: bool,
    pub collide_connected: bool,
    pub kind: JointKind2D,
    pub signature: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct JointDesc3D {
    pub id: NodeID,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector3,
    pub anchor_b: Vector3,
    pub enabled: bool,
    pub collide_connected: bool,
    pub kind: JointKind3D,
    pub signature: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct RigidProps2D {
    pub enabled: bool,
    pub can_sleep: bool,
    pub lock_rotation: bool,
    pub continuous_collision_detection: bool,
    pub linear_velocity: Vector2,
    pub angular_velocity: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RigidProps3D {
    pub enabled: bool,
    pub can_sleep: bool,
    pub mass: f32,
    pub continuous_collision_detection: bool,
    pub linear_velocity: Vector3,
    pub angular_velocity: Vector3,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyPair {
    pub a: NodeID,
    pub b: NodeID,
}

impl BodyPair {
    pub fn sorted(a: NodeID, b: NodeID) -> Self {
        if a.as_u64() <= b.as_u64() {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AreaOverlap {
    pub area: NodeID,
    pub other: NodeID,
}

#[derive(Clone, Copy, Debug)]
pub struct PendingImpulse2D {
    pub id: NodeID,
    pub impulse: Vector2,
}

#[derive(Clone, Copy, Debug)]
pub struct PendingImpulse3D {
    pub id: NodeID,
    pub impulse: Vector3,
}

#[derive(Clone, Copy, Debug)]
pub struct PendingForce2D {
    pub id: NodeID,
    pub force: Vector2,
}

#[derive(Clone, Copy, Debug)]
pub struct PendingForce3D {
    pub id: NodeID,
    pub force: Vector3,
}
