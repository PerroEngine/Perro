use super::RuntimePhysicsStepTiming;
use crate::Runtime;
use ahash::{AHashMap, AHashSet};
use perro_ids::{NodeID, SignalID, parse_hashed_source_uri, string_to_u64};
use perro_io::{decompress_zlib, load_asset};
use perro_nodes::{
    CollisionShape2D, CollisionShape3D, SceneNodeData, Shape2D, Shape3D, TileMap2D, Triangle2DKind,
};
use perro_runtime_context::sub_apis::{
    NodeAPI, PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D,
    PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D, SignalAPI,
};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;
use rapier2d::{na as na2, prelude as r2};
use rapier3d::{na as na3, prelude as r3};

const MAX_CCD_SUBSTEPS: usize = 1;
const PMESH_FLAG_PAYLOAD_RAW: u32 = 1 << 31;
const MAX_RIGID_SPEED_2D: f32 = 80.0;
const MAX_RIGID_SPEED_3D: f32 = 80.0;
const CCD_MIN_SPEED_RATIO_OF_MAX: f32 = 0.5;
const CCD_MIN_SPEED_SQ_2D: f32 = MAX_RIGID_SPEED_2D
    * CCD_MIN_SPEED_RATIO_OF_MAX
    * MAX_RIGID_SPEED_2D
    * CCD_MIN_SPEED_RATIO_OF_MAX;
const CCD_MIN_SPEED_SQ_3D: f32 = MAX_RIGID_SPEED_3D
    * CCD_MIN_SPEED_RATIO_OF_MAX
    * MAX_RIGID_SPEED_3D
    * CCD_MIN_SPEED_RATIO_OF_MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyKind {
    Static,
    Area,
    Rigid,
}

#[derive(Clone, Debug)]
struct ShapeDesc2D {
    local: Transform2D,
    shape: ShapeKind2D,
    sensor: bool,
    collision_layer: u32,
    collision_mask: u32,
    friction: f32,
    restitution: f32,
}

#[derive(Clone, Debug)]
enum ShapeKind2D {
    Primitive(Shape2D),
    Polygon(Vec<Vector2>),
}

#[derive(Clone, Debug)]
struct ShapeDesc3D {
    local: Transform3D,
    shape: ShapeKind3D,
    sensor: bool,
    collision_layer: u32,
    collision_mask: u32,
    friction: f32,
    restitution: f32,
}

#[derive(Clone, Debug)]
enum ShapeKind3D {
    Primitive(Shape3D),
    TriMesh { source: String },
}

#[derive(Clone, Debug)]
struct BodyDesc2D {
    id: NodeID,
    kind: BodyKind,
    enabled: bool,
    global: Transform2D,
    rigid: Option<RigidProps2D>,
    shape_signature: u64,
    shapes: Vec<ShapeDesc2D>,
}

#[derive(Clone, Debug)]
struct BodyDesc3D {
    id: NodeID,
    kind: BodyKind,
    enabled: bool,
    global: Transform3D,
    rigid: Option<RigidProps3D>,
    shape_signature: u64,
    shapes: Vec<ShapeDesc3D>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum JointKind2D {
    Pin,
    Distance { min: f32, max: f32 },
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum JointKind3D {
    Ball,
    Hinge { axis: Vector3 },
    Fixed,
}

#[derive(Clone, Copy, Debug)]
struct JointDesc2D {
    id: NodeID,
    body_a: NodeID,
    body_b: NodeID,
    anchor_a: Vector2,
    anchor_b: Vector2,
    enabled: bool,
    collide_connected: bool,
    kind: JointKind2D,
    signature: u64,
}

#[derive(Clone, Copy, Debug)]
struct JointDesc3D {
    id: NodeID,
    body_a: NodeID,
    body_b: NodeID,
    anchor_a: Vector3,
    anchor_b: Vector3,
    enabled: bool,
    collide_connected: bool,
    kind: JointKind3D,
    signature: u64,
}

#[derive(Clone, Copy, Debug)]
struct RigidProps2D {
    enabled: bool,
    can_sleep: bool,
    lock_rotation: bool,
    continuous_collision_detection: bool,
    linear_velocity: Vector2,
    angular_velocity: f32,
    gravity_scale: f32,
    linear_damping: f32,
    angular_damping: f32,
}

#[derive(Clone, Copy, Debug)]
struct RigidProps3D {
    enabled: bool,
    can_sleep: bool,
    mass: f32,
    continuous_collision_detection: bool,
    linear_velocity: Vector3,
    angular_velocity: Vector3,
    gravity_scale: f32,
    linear_damping: f32,
    angular_damping: f32,
}

#[derive(Clone, Debug)]
struct BodyState2D {
    handle: r2::RigidBodyHandle,
    colliders: Vec<r2::ColliderHandle>,
    kind: BodyKind,
    shape_signature: u64,
    opaque_handle: u64,
}

#[derive(Clone, Debug)]
struct BodyState3D {
    handle: r3::RigidBodyHandle,
    colliders: Vec<r3::ColliderHandle>,
    kind: BodyKind,
    shape_signature: u64,
    opaque_handle: u64,
}

#[derive(Clone, Debug)]
struct JointState2D {
    handle: r2::ImpulseJointHandle,
    signature: u64,
}

#[derive(Clone, Debug)]
struct JointState3D {
    handle: r3::ImpulseJointHandle,
    signature: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BodyPair {
    a: NodeID,
    b: NodeID,
}

impl BodyPair {
    fn sorted(a: NodeID, b: NodeID) -> Self {
        if a.as_u64() <= b.as_u64() {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct AreaOverlap {
    area: NodeID,
    other: NodeID,
}

#[derive(Clone, Copy, Debug)]
struct PendingImpulse2D {
    id: NodeID,
    impulse: Vector2,
}

#[derive(Clone, Copy, Debug)]
struct PendingImpulse3D {
    id: NodeID,
    impulse: Vector3,
}

#[derive(Clone, Copy, Debug)]
struct PendingForce2D {
    id: NodeID,
    force: Vector2,
}

#[derive(Clone, Copy, Debug)]
struct PendingForce3D {
    id: NodeID,
    force: Vector3,
}

pub(crate) struct PhysicsState {
    paused: bool,
    world_2d: Option<PhysicsWorld2D>,
    world_3d: Option<PhysicsWorld3D>,
    active_collision_pairs_2d: AHashSet<BodyPair>,
    active_collision_pairs_3d: AHashSet<BodyPair>,
    active_area_overlaps_2d: AHashSet<AreaOverlap>,
    active_area_overlaps_3d: AHashSet<AreaOverlap>,
    pending_forces_2d: Vec<PendingForce2D>,
    pending_forces_3d: Vec<PendingForce3D>,
    pending_impulses_2d: Vec<PendingImpulse2D>,
    pending_impulses_3d: Vec<PendingImpulse3D>,
    stale_ids_2d: Vec<NodeID>,
    stale_ids_3d: Vec<NodeID>,
    trimesh_cache: AHashMap<u64, TriMeshData>,
    next_opaque_handle: u64,
}

struct PhysicsWorld2D {
    pipeline: r2::PhysicsPipeline,
    gravity: r2::Vector<f32>,
    integration_parameters: r2::IntegrationParameters,
    islands: r2::IslandManager,
    broad_phase: r2::DefaultBroadPhase,
    narrow_phase: r2::NarrowPhase,
    bodies: r2::RigidBodySet,
    colliders: r2::ColliderSet,
    impulse_joints: r2::ImpulseJointSet,
    multibody_joints: r2::MultibodyJointSet,
    ccd_solver: r2::CCDSolver,
    collider_owners: AHashMap<r2::ColliderHandle, NodeID>,
    body_map: AHashMap<NodeID, BodyState2D>,
    joint_map: AHashMap<NodeID, JointState2D>,
}

struct PhysicsWorld3D {
    pipeline: r3::PhysicsPipeline,
    gravity: r3::Vector<f32>,
    integration_parameters: r3::IntegrationParameters,
    islands: r3::IslandManager,
    broad_phase: r3::DefaultBroadPhase,
    narrow_phase: r3::NarrowPhase,
    bodies: r3::RigidBodySet,
    colliders: r3::ColliderSet,
    impulse_joints: r3::ImpulseJointSet,
    multibody_joints: r3::MultibodyJointSet,
    ccd_solver: r3::CCDSolver,
    collider_owners: AHashMap<r3::ColliderHandle, NodeID>,
    body_map: AHashMap<NodeID, BodyState3D>,
    joint_map: AHashMap<NodeID, JointState3D>,
}

impl PhysicsState {
    pub(crate) fn new() -> Self {
        Self {
            paused: false,
            world_2d: None,
            world_3d: None,
            active_collision_pairs_2d: AHashSet::default(),
            active_collision_pairs_3d: AHashSet::default(),
            active_area_overlaps_2d: AHashSet::default(),
            active_area_overlaps_3d: AHashSet::default(),
            pending_forces_2d: Vec::new(),
            pending_forces_3d: Vec::new(),
            pending_impulses_2d: Vec::new(),
            pending_impulses_3d: Vec::new(),
            stale_ids_2d: Vec::new(),
            stale_ids_3d: Vec::new(),
            trimesh_cache: AHashMap::default(),
            next_opaque_handle: 1,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.world_2d = None;
        self.world_3d = None;
        self.active_collision_pairs_2d.clear();
        self.active_collision_pairs_3d.clear();
        self.active_area_overlaps_2d.clear();
        self.active_area_overlaps_3d.clear();
        self.pending_forces_2d.clear();
        self.pending_forces_3d.clear();
        self.pending_impulses_2d.clear();
        self.pending_impulses_3d.clear();
        self.stale_ids_2d.clear();
        self.stale_ids_3d.clear();
        self.trimesh_cache.clear();
        self.next_opaque_handle = 1;
    }

    fn alloc_opaque_handle(&mut self) -> u64 {
        let handle = self.next_opaque_handle;
        self.next_opaque_handle = self.next_opaque_handle.saturating_add(1);
        handle
    }
}

impl Default for PhysicsState {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsWorld2D {
    fn new() -> Self {
        let integration_parameters = r2::IntegrationParameters {
            max_ccd_substeps: MAX_CCD_SUBSTEPS,
            ..r2::IntegrationParameters::default()
        };
        Self {
            pipeline: r2::PhysicsPipeline::new(),
            gravity: na2::Vector2::new(0.0, -9.81),
            integration_parameters,
            islands: r2::IslandManager::new(),
            broad_phase: r2::DefaultBroadPhase::new(),
            narrow_phase: r2::NarrowPhase::new(),
            bodies: r2::RigidBodySet::new(),
            colliders: r2::ColliderSet::new(),
            impulse_joints: r2::ImpulseJointSet::new(),
            multibody_joints: r2::MultibodyJointSet::new(),
            ccd_solver: r2::CCDSolver::new(),
            collider_owners: AHashMap::default(),
            body_map: AHashMap::default(),
            joint_map: AHashMap::default(),
        }
    }
}

impl PhysicsWorld3D {
    fn new() -> Self {
        let integration_parameters = r3::IntegrationParameters {
            max_ccd_substeps: MAX_CCD_SUBSTEPS,
            ..r3::IntegrationParameters::default()
        };
        Self {
            pipeline: r3::PhysicsPipeline::new(),
            gravity: na3::Vector3::new(0.0, -9.81, 0.0),
            integration_parameters,
            islands: r3::IslandManager::new(),
            broad_phase: r3::DefaultBroadPhase::new(),
            narrow_phase: r3::NarrowPhase::new(),
            bodies: r3::RigidBodySet::new(),
            colliders: r3::ColliderSet::new(),
            impulse_joints: r3::ImpulseJointSet::new(),
            multibody_joints: r3::MultibodyJointSet::new(),
            ccd_solver: r3::CCDSolver::new(),
            collider_owners: AHashMap::default(),
            body_map: AHashMap::default(),
            joint_map: AHashMap::default(),
        }
    }
}

impl Runtime {
    pub fn set_physics_paused(&mut self, paused: bool) {
        self.physics.paused = paused;
    }

    pub fn physics_paused(&self) -> bool {
        self.physics.paused
    }

    pub(crate) fn physics_fixed_step_timed(&mut self) -> RuntimePhysicsStepTiming {
        let total_start = std::time::Instant::now();

        let pre_transforms_start = std::time::Instant::now();
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let pre_transforms = pre_transforms_start.elapsed();

        let collect_start = std::time::Instant::now();
        let bodies_2d = self.collect_body_descs_2d();
        let bodies_3d = self.collect_body_descs_3d();
        let joints_2d = self.collect_joint_descs_2d();
        let joints_3d = self.collect_joint_descs_3d();
        let collect = collect_start.elapsed();

        let sync_world_start = std::time::Instant::now();
        self.sync_world_2d(&bodies_2d);
        self.sync_world_3d(&bodies_3d);
        self.sync_joints_2d(&joints_2d);
        self.sync_joints_3d(&joints_3d);
        let sync_world = sync_world_start.elapsed();

        if self.physics.paused {
            return RuntimePhysicsStepTiming {
                pre_transforms,
                collect,
                sync_world,
                apply_forces_impulses: std::time::Duration::ZERO,
                step: std::time::Duration::ZERO,
                sync_nodes: std::time::Duration::ZERO,
                post_transforms: std::time::Duration::ZERO,
                signals: std::time::Duration::ZERO,
                total: total_start.elapsed(),
            };
        }

        let apply_forces_impulses_start = std::time::Instant::now();
        self.apply_pending_forces_2d();
        self.apply_pending_forces_3d();
        self.apply_pending_impulses_2d();
        self.apply_pending_impulses_3d();
        let apply_forces_impulses = apply_forces_impulses_start.elapsed();

        let step_start = std::time::Instant::now();
        self.step_world_2d();
        self.step_world_3d();
        let step = step_start.elapsed();

        let sync_nodes_start = std::time::Instant::now();
        self.sync_world_to_nodes_2d();
        self.sync_world_to_nodes_3d();
        let sync_nodes = sync_nodes_start.elapsed();

        let post_transforms_start = std::time::Instant::now();
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let post_transforms = post_transforms_start.elapsed();

        let signals_start = std::time::Instant::now();
        self.emit_collision_signals_2d();
        self.emit_collision_signals_3d();
        self.emit_area_signals_2d();
        self.emit_area_signals_3d();
        let signals = signals_start.elapsed();

        RuntimePhysicsStepTiming {
            pre_transforms,
            collect,
            sync_world,
            apply_forces_impulses,
            step,
            sync_nodes,
            post_transforms,
            signals,
            total: total_start.elapsed(),
        }
    }

    pub(crate) fn physics_fixed_step(&mut self) {
        let _ = self.physics_fixed_step_timed();
    }

    pub(crate) fn queue_impulse_2d(&mut self, id: NodeID, impulse: Vector2) {
        self.physics
            .pending_impulses_2d
            .push(PendingImpulse2D { id, impulse });
    }

    pub(crate) fn queue_force_2d(&mut self, id: NodeID, force: Vector2) {
        self.physics
            .pending_forces_2d
            .push(PendingForce2D { id, force });
    }

    pub(crate) fn queue_impulse_3d(&mut self, id: NodeID, impulse: Vector3) {
        self.physics
            .pending_impulses_3d
            .push(PendingImpulse3D { id, impulse });
    }

    pub(crate) fn queue_force_3d(&mut self, id: NodeID, force: Vector3) {
        self.physics
            .pending_forces_3d
            .push(PendingForce3D { id, force });
    }

    pub(crate) fn clear_physics(&mut self) {
        self.physics.clear();
    }

    pub fn physics_raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }

        let dir = na3::Vector3::new(direction.x, direction.y, direction.z);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }
        let dir = dir / dir_len;

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);

        let world = self.physics.world_3d.as_ref()?;
        let mut query = r3::QueryPipeline::new();
        query.update(&world.colliders);

        let ray = r3::Ray::new(na3::Point3::new(origin.x, origin.y, origin.z), dir);
        let filter = if include_areas {
            r3::QueryFilter::new()
        } else {
            r3::QueryFilter::new().exclude_sensors()
        };
        let (collider, hit) = query.cast_ray_and_get_normal(
            &world.bodies,
            &world.colliders,
            &ray,
            max_distance,
            true,
            filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = ray.point_at(hit.time_of_impact);

        Some(PhysicsRayHit3D {
            node,
            point: Vector3::new(point.x, point.y, point.z),
            normal: Vector3::new(hit.normal.x, hit.normal.y, hit.normal.z),
            distance: hit.time_of_impact,
        })
    }

    pub fn physics_raycast_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }

        let dir = na2::Vector2::new(direction.x, direction.y);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }
        let dir = dir / dir_len;

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);

        let world = self.physics.world_2d.as_ref()?;
        let mut query = r2::QueryPipeline::new();
        query.update(&world.colliders);

        let ray = r2::Ray::new(na2::Point2::new(origin.x, origin.y), dir);
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask;
        let predicate = |handle, collider: &r2::Collider| {
            (collider.collision_groups().memberships.bits() & mask) != 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_2d(filter).predicate(&predicate);
        let (collider, hit) = query.cast_ray_and_get_normal(
            &world.bodies,
            &world.colliders,
            &ray,
            max_distance,
            true,
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = ray.point_at(hit.time_of_impact);

        Some(PhysicsRayHit2D {
            node,
            point: Vector2::new(point.x, point.y),
            normal: Vector2::new(hit.normal.x, hit.normal.y),
            distance: hit.time_of_impact,
        })
    }

    pub fn physics_shape_cast_2d(
        &mut self,
        shape: Shape2D,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }
        let dir = na2::Vector2::new(direction.x, direction.y);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);

        let world = self.physics.world_2d.as_ref()?;
        let shape = shared_shape_2d(shape)?;
        let mut query = r2::QueryPipeline::new();
        query.update(&world.colliders);

        let shape_pos = na2::Isometry2::new(na2::Vector2::new(origin.x, origin.y), 0.0);
        let shape_vel = dir / dir_len * max_distance;
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask;
        let predicate = |handle, collider: &r2::Collider| {
            (collider.collision_groups().memberships.bits() & mask) != 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_2d(filter).predicate(&predicate);
        let (collider, hit) = query.cast_shape(
            &world.bodies,
            &world.colliders,
            &shape_pos,
            &shape_vel,
            shape.as_ref(),
            rapier2d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = hit.transform1_by(&shape_pos).witness1;

        Some(PhysicsShapeHit2D {
            node,
            point: Vector2::new(point.x, point.y),
            normal: Vector2::new(hit.normal1.x, hit.normal1.y),
            distance: hit.time_of_impact * max_distance,
        })
    }

    pub fn physics_shape_cast_3d(
        &mut self,
        shape: Shape3D,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }
        let dir = na3::Vector3::new(direction.x, direction.y, direction.z);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);

        let world = self.physics.world_3d.as_ref()?;
        let shape = shared_shape_3d(shape)?;
        let mut query = r3::QueryPipeline::new();
        query.update(&world.colliders);

        let shape_pos = na3::Isometry3::translation(origin.x, origin.y, origin.z);
        let shape_vel = dir / dir_len * max_distance;
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask;
        let predicate = |handle, collider: &r3::Collider| {
            (collider.collision_groups().memberships.bits() & mask) != 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_3d(filter).predicate(&predicate);
        let (collider, hit) = query.cast_shape(
            &world.bodies,
            &world.colliders,
            &shape_pos,
            &shape_vel,
            shape.as_ref(),
            rapier3d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = hit.transform1_by(&shape_pos).witness1;

        Some(PhysicsShapeHit3D {
            node,
            point: Vector3::new(point.x, point.y, point.z),
            normal: Vector3::new(hit.normal1.x, hit.normal1.y, hit.normal1.z),
            distance: hit.time_of_impact * max_distance,
        })
    }

    pub fn physics_contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);

        let Some(world) = self.physics.world_2d.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            let other = if a == body_id {
                b
            } else if b == body_id {
                a
            } else {
                continue;
            };
            for manifold in &pair.manifolds {
                let normal = if a == body_id {
                    manifold.data.normal
                } else {
                    -manifold.data.normal
                };
                for contact in &manifold.data.solver_contacts {
                    out.push(PhysicsContact2D {
                        node: other,
                        point: Vector2::new(contact.point.x, contact.point.y),
                        normal: Vector2::new(normal.x, normal.y),
                        impulse: contact.warmstart_impulse,
                    });
                }
            }
        }
        out
    }

    pub fn physics_contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);

        let Some(world) = self.physics.world_3d.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            let other = if a == body_id {
                b
            } else if b == body_id {
                a
            } else {
                continue;
            };
            for manifold in &pair.manifolds {
                let normal = if a == body_id {
                    manifold.data.normal
                } else {
                    -manifold.data.normal
                };
                for contact in &manifold.data.solver_contacts {
                    out.push(PhysicsContact3D {
                        node: other,
                        point: Vector3::new(contact.point.x, contact.point.y, contact.point.z),
                        normal: Vector3::new(normal.x, normal.y, normal.z),
                        impulse: contact.warmstart_impulse,
                    });
                }
            }
        }
        out
    }

    fn collect_body_descs_2d(&mut self) -> Vec<BodyDesc2D> {
        let node_count = self.internal_updates.physics_body_nodes_2d.len();
        let mut out = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let id = self.internal_updates.physics_body_nodes_2d[i];
            let (kind, enabled, rigid, material, groups) = {
                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                match &node.data {
                    SceneNodeData::StaticBody2D(body) => (
                        BodyKind::Static,
                        body.enabled,
                        None,
                        (body.friction, body.restitution),
                        (body.collision_layer, body.collision_mask),
                    ),
                    SceneNodeData::Area2D(body) => (
                        BodyKind::Area,
                        body.enabled,
                        None,
                        (0.7, 0.0),
                        (body.collision_layer, body.collision_mask),
                    ),
                    SceneNodeData::RigidBody2D(body) => (
                        BodyKind::Rigid,
                        body.enabled,
                        Some(RigidProps2D {
                            enabled: body.enabled,
                            can_sleep: body.can_sleep,
                            lock_rotation: body.lock_rotation,
                            continuous_collision_detection: body.continuous_collision_detection,
                            linear_velocity: body.linear_velocity,
                            angular_velocity: body.angular_velocity,
                            gravity_scale: body.gravity_scale,
                            linear_damping: body.linear_damping,
                            angular_damping: body.angular_damping,
                        }),
                        (body.friction, body.restitution),
                        (body.collision_layer, body.collision_mask),
                    ),
                    SceneNodeData::TileMap2D(tilemap) => (
                        BodyKind::Static,
                        tilemap.collision_enabled,
                        None,
                        (0.7, 0.0),
                        (tilemap.collision_layer, tilemap.collision_mask),
                    ),
                    _ => continue,
                }
            };
            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            let tilemap_for_body = self.nodes.get(id).and_then(|node| match &node.data {
                SceneNodeData::TileMap2D(tilemap) => Some(tilemap.clone()),
                _ => None,
            });
            let mut shape_signature = body_signature_seed(kind);
            if let Some(tilemap) = tilemap_for_body.as_ref() {
                shape_signature = hash_tilemap_2d(shape_signature, tilemap);
                if let Some(tileset) =
                    crate::runtime::render_2d::resolve_tileset_2d(self, &tilemap.tileset)
                {
                    for tile in tileset.tiles.iter() {
                        if tile.collision {
                            shape_signature = hash_u64(shape_signature, tile.id as u64);
                            shape_signature = hash_tile_collision_shape_2d(
                                shape_signature,
                                tile.collision_shape.clone(),
                            );
                        }
                    }
                }
            } else if let Some(node) = self.nodes.get(id) {
                for &child_id in node.children_slice() {
                    let Some(child) = self.nodes.get(child_id) else {
                        continue;
                    };
                    if let SceneNodeData::CollisionShape2D(shape) = &child.data {
                        shape_signature = hash_collision_shape_2d(shape_signature, shape, kind);
                    }
                }
            }
            shape_signature = hash_u32(shape_signature, groups.0);
            shape_signature = hash_u32(shape_signature, groups.1);

            let needs_shape_rebuild = self
                .physics
                .world_2d
                .as_ref()
                .and_then(|world| world.body_map.get(&id))
                .map(|state| state.shape_signature != shape_signature)
                .unwrap_or(true);

            let mut shapes = Vec::new();
            if needs_shape_rebuild {
                if let Some(tilemap) = tilemap_for_body.as_ref() {
                    let tileset =
                        crate::runtime::render_2d::resolve_tileset_2d(self, &tilemap.tileset);
                    shapes.extend(tilemap_shape_descs_2d(
                        tilemap,
                        groups.0,
                        groups.1,
                        material.0,
                        material.1,
                        tileset.as_ref(),
                    ));
                } else if let Some(node) = self.nodes.get(id) {
                    let child_count = node.children_slice().len();
                    if shapes.capacity() < child_count {
                        shapes.reserve(child_count - shapes.capacity());
                    }
                    for &child_id in node.children_slice() {
                        let Some(child) = self.nodes.get(child_id) else {
                            continue;
                        };
                        if let SceneNodeData::CollisionShape2D(shape) = &child.data {
                            let mut desc = shape_desc_2d(shape, material.0, material.1);
                            desc.sensor = kind == BodyKind::Area;
                            desc.collision_layer = groups.0;
                            desc.collision_mask = groups.1;
                            shapes.push(desc);
                        }
                    }
                }
            }

            out.push(BodyDesc2D {
                id,
                kind,
                enabled,
                global,
                rigid,
                shape_signature,
                shapes,
            });
        }
        out
    }

    fn collect_body_descs_3d(&mut self) -> Vec<BodyDesc3D> {
        let node_count = self.internal_updates.physics_body_nodes_3d.len();
        let mut out = Vec::with_capacity(node_count);
        for i in 0..node_count {
            let id = self.internal_updates.physics_body_nodes_3d[i];
            let (kind, enabled, rigid, material, groups) = {
                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                match &node.data {
                    SceneNodeData::StaticBody3D(body) => (
                        BodyKind::Static,
                        body.enabled,
                        None,
                        (body.friction, body.restitution),
                        (body.collision_layer, body.collision_mask),
                    ),
                    SceneNodeData::Area3D(body) => (
                        BodyKind::Area,
                        body.enabled,
                        None,
                        (0.7, 0.0),
                        (body.collision_layer, body.collision_mask),
                    ),
                    SceneNodeData::RigidBody3D(body) => (
                        BodyKind::Rigid,
                        body.enabled,
                        Some(RigidProps3D {
                            enabled: body.enabled,
                            can_sleep: body.can_sleep,
                            mass: body.mass,
                            continuous_collision_detection: body.continuous_collision_detection,
                            linear_velocity: body.linear_velocity,
                            angular_velocity: body.angular_velocity,
                            gravity_scale: body.gravity_scale,
                            linear_damping: body.linear_damping,
                            angular_damping: body.angular_damping,
                        }),
                        (body.friction, body.restitution),
                        (body.collision_layer, body.collision_mask),
                    ),
                    _ => continue,
                }
            };

            let Some(global) = self.get_global_transform_3d(id) else {
                continue;
            };
            let mut shape_signature = body_signature_seed(kind);
            shape_signature = hash_f32(shape_signature, global.scale.x.to_bits());
            shape_signature = hash_f32(shape_signature, global.scale.y.to_bits());
            shape_signature = hash_f32(shape_signature, global.scale.z.to_bits());
            shape_signature = hash_u32(shape_signature, groups.0);
            shape_signature = hash_u32(shape_signature, groups.1);

            if let Some(node) = self.nodes.get(id) {
                for &child_id in node.children_slice() {
                    let Some(child) = self.nodes.get(child_id) else {
                        continue;
                    };
                    if let SceneNodeData::CollisionShape3D(shape) = &child.data {
                        shape_signature =
                            hash_collision_shape_3d(shape_signature, shape, kind, global.scale);
                    }
                }
            }

            let needs_shape_rebuild = self
                .physics
                .world_3d
                .as_ref()
                .and_then(|world| world.body_map.get(&id))
                .map(|state| state.shape_signature != shape_signature)
                .unwrap_or(true);

            let mut shapes = Vec::new();
            if needs_shape_rebuild && let Some(node) = self.nodes.get(id) {
                let child_count = node.children_slice().len();
                if shapes.capacity() < child_count {
                    shapes.reserve(child_count - shapes.capacity());
                }
                for &child_id in node.children_slice() {
                    let Some(child) = self.nodes.get(child_id) else {
                        continue;
                    };
                    if let SceneNodeData::CollisionShape3D(shape) = &child.data {
                        let mut desc = shape_desc_3d(shape, material.0, material.1);
                        // Physics colliders inherit parent body global scale.
                        desc.local.scale = Vector3::new(
                            desc.local.scale.x * global.scale.x,
                            desc.local.scale.y * global.scale.y,
                            desc.local.scale.z * global.scale.z,
                        );
                        desc.sensor = kind == BodyKind::Area;
                        desc.collision_layer = groups.0;
                        desc.collision_mask = groups.1;
                        shapes.push(desc);
                    }
                }
            }

            out.push(BodyDesc3D {
                id,
                kind,
                enabled,
                global,
                rigid,
                shape_signature,
                shapes,
            });
        }
        out
    }

    fn collect_joint_descs_2d(&self) -> Vec<JointDesc2D> {
        let mut out = Vec::new();
        for i in 0..self.internal_updates.internal_fixed_update_nodes.len() {
            let id = self.internal_updates.internal_fixed_update_nodes[i];
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let (body_a, body_b, anchor_a, anchor_b, enabled, collide_connected, kind) =
                match &node.data {
                    SceneNodeData::PinJoint2D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind2D::Pin,
                    ),
                    SceneNodeData::DistanceJoint2D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind2D::Distance {
                            min: joint.min_distance,
                            max: joint.max_distance,
                        },
                    ),
                    SceneNodeData::FixedJoint2D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind2D::Fixed,
                    ),
                    _ => continue,
                };
            let signature = joint_signature_2d(
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
            );
            out.push(JointDesc2D {
                id,
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
                signature,
            });
        }
        out
    }

    fn collect_joint_descs_3d(&self) -> Vec<JointDesc3D> {
        let mut out = Vec::new();
        for i in 0..self.internal_updates.internal_fixed_update_nodes.len() {
            let id = self.internal_updates.internal_fixed_update_nodes[i];
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let (body_a, body_b, anchor_a, anchor_b, enabled, collide_connected, kind) =
                match &node.data {
                    SceneNodeData::BallJoint3D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind3D::Ball,
                    ),
                    SceneNodeData::HingeJoint3D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind3D::Hinge { axis: joint.axis },
                    ),
                    SceneNodeData::FixedJoint3D(joint) => (
                        joint.body_a,
                        joint.body_b,
                        joint.anchor_a,
                        joint.anchor_b,
                        joint.enabled,
                        joint.collide_connected,
                        JointKind3D::Fixed,
                    ),
                    _ => continue,
                };
            let signature = joint_signature_3d(
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
            );
            out.push(JointDesc3D {
                id,
                body_a,
                body_b,
                anchor_a,
                anchor_b,
                enabled,
                collide_connected,
                kind,
                signature,
            });
        }
        out
    }

    fn sync_world_2d(&mut self, bodies: &[BodyDesc2D]) {
        if bodies.is_empty() {
            if let Some(world) = self.physics.world_2d.take() {
                for id in world.body_map.keys().copied() {
                    self.set_body_handle_2d(id, None);
                }
            }
            return;
        }
        let mut world = self
            .physics
            .world_2d
            .take()
            .unwrap_or_else(PhysicsWorld2D::new);
        let mut alive = AHashSet::default();
        for body in bodies {
            alive.insert(body.id);
            if !world.body_map.contains_key(&body.id) {
                let rb_handle = world.bodies.insert(build_rigid_body_2d(body));
                let opaque = self.physics.alloc_opaque_handle();
                world.body_map.insert(
                    body.id,
                    BodyState2D {
                        handle: rb_handle,
                        colliders: Vec::new(),
                        kind: body.kind,
                        shape_signature: 0,
                        opaque_handle: opaque,
                    },
                );
                self.set_body_handle_2d(body.id, Some(opaque));
            }

            let Some(state) = world.body_map.get_mut(&body.id) else {
                continue;
            };

            state.kind = body.kind;
            if let Some(rb) = world.bodies.get_mut(state.handle) {
                rb.set_enabled(body.enabled);
                let target_body_type = match body.kind {
                    BodyKind::Static => r2::RigidBodyType::Fixed,
                    BodyKind::Area => r2::RigidBodyType::Fixed,
                    BodyKind::Rigid => r2::RigidBodyType::Dynamic,
                };
                if rb.body_type() != target_body_type {
                    rb.set_body_type(target_body_type, true);
                }

                let target_pos = transform_to_iso2(body.global);
                let current_pos = rb.position();
                let pos_changed =
                    !approx_eq_f32(
                        current_pos.translation.vector.x,
                        target_pos.translation.vector.x,
                    ) || !approx_eq_f32(
                        current_pos.translation.vector.y,
                        target_pos.translation.vector.y,
                    ) || !approx_eq_f32(current_pos.rotation.angle(), target_pos.rotation.angle());
                if pos_changed {
                    rb.set_position(target_pos, true);
                }

                if let Some(rigid) = body.rigid {
                    let target_lin =
                        na2::Vector2::new(rigid.linear_velocity.x, rigid.linear_velocity.y);
                    let current_lin = rb.linvel();
                    if !approx_eq_f32(current_lin.x, target_lin.x)
                        || !approx_eq_f32(current_lin.y, target_lin.y)
                    {
                        rb.set_linvel(target_lin, true);
                    }
                    if !approx_eq_f32(rb.angvel(), rigid.angular_velocity) {
                        rb.set_angvel(rigid.angular_velocity, true);
                    }
                    if !approx_eq_f32(rb.gravity_scale(), rigid.gravity_scale) {
                        rb.set_gravity_scale(rigid.gravity_scale, true);
                    }
                    if !approx_eq_f32(rb.linear_damping(), rigid.linear_damping) {
                        rb.set_linear_damping(rigid.linear_damping);
                    }
                    if !approx_eq_f32(rb.angular_damping(), rigid.angular_damping) {
                        rb.set_angular_damping(rigid.angular_damping);
                    }
                    let target_speed_sq = target_lin.norm_squared();
                    let target_ccd = rigid.continuous_collision_detection
                        && target_speed_sq >= CCD_MIN_SPEED_SQ_2D;
                    if rb.is_ccd_enabled() != target_ccd {
                        rb.enable_ccd(target_ccd);
                    }
                } else {
                    if rb.is_ccd_enabled() {
                        rb.enable_ccd(false);
                    }
                }
            }

            if state.shape_signature != body.shape_signature {
                for handle in state.colliders.drain(..) {
                    world.collider_owners.remove(&handle);
                    let _ =
                        world
                            .colliders
                            .remove(handle, &mut world.islands, &mut world.bodies, true);
                }

                for shape in &body.shapes {
                    let Some(builder) = collider_builder_2d(shape) else {
                        continue;
                    };
                    let handle = world.colliders.insert_with_parent(
                        builder,
                        state.handle,
                        &mut world.bodies,
                    );
                    world.collider_owners.insert(handle, body.id);
                    state.colliders.push(handle);
                }
                state.shape_signature = body.shape_signature;
            }
        }

        let mut stale = std::mem::take(&mut self.physics.stale_ids_2d);
        stale.clear();
        stale.extend(
            world
                .body_map
                .keys()
                .copied()
                .filter(|id| !alive.contains(id)),
        );

        for id in stale.iter().copied() {
            if let Some(state) = world.body_map.remove(&id) {
                for handle in &state.colliders {
                    world.collider_owners.remove(handle);
                }
                let _ = world.bodies.remove(
                    state.handle,
                    &mut world.islands,
                    &mut world.colliders,
                    &mut world.impulse_joints,
                    &mut world.multibody_joints,
                    true,
                );
            }
            self.set_body_handle_2d(id, None);
        }
        stale.clear();
        self.physics.stale_ids_2d = stale;
        self.physics.world_2d = Some(world);
    }

    fn sync_world_3d(&mut self, bodies: &[BodyDesc3D]) {
        if bodies.is_empty() {
            if let Some(world) = self.physics.world_3d.take() {
                for id in world.body_map.keys().copied() {
                    self.set_body_handle_3d(id, None);
                }
            }
            return;
        }
        let mut world = self
            .physics
            .world_3d
            .take()
            .unwrap_or_else(PhysicsWorld3D::new);
        let static_mesh_lookup = self
            .project()
            .and_then(|project| project.static_mesh_lookup);
        let static_collision_trimesh_lookup = self
            .project()
            .and_then(|project| project.static_collision_trimesh_lookup);
        let mut alive = AHashSet::default();
        for body in bodies {
            alive.insert(body.id);
            if !world.body_map.contains_key(&body.id) {
                let rb_handle = world.bodies.insert(build_rigid_body_3d(body));
                let opaque = self.physics.alloc_opaque_handle();
                world.body_map.insert(
                    body.id,
                    BodyState3D {
                        handle: rb_handle,
                        colliders: Vec::new(),
                        kind: body.kind,
                        shape_signature: 0,
                        opaque_handle: opaque,
                    },
                );
                self.set_body_handle_3d(body.id, Some(opaque));
            }

            let Some(state) = world.body_map.get_mut(&body.id) else {
                continue;
            };

            state.kind = body.kind;
            if let Some(rb) = world.bodies.get_mut(state.handle) {
                rb.set_enabled(body.enabled);
                let target_body_type = match body.kind {
                    BodyKind::Static => r3::RigidBodyType::Fixed,
                    BodyKind::Area => r3::RigidBodyType::Fixed,
                    BodyKind::Rigid => r3::RigidBodyType::Dynamic,
                };
                if rb.body_type() != target_body_type {
                    rb.set_body_type(target_body_type, true);
                }

                let target_pos = transform_to_iso3(body.global);
                let current_pos = rb.position();
                let pos_changed =
                    !approx_eq_f32(
                        current_pos.translation.vector.x,
                        target_pos.translation.vector.x,
                    ) || !approx_eq_f32(
                        current_pos.translation.vector.y,
                        target_pos.translation.vector.y,
                    ) || !approx_eq_f32(
                        current_pos.translation.vector.z,
                        target_pos.translation.vector.z,
                    ) || !approx_eq_f32(current_pos.rotation.i, target_pos.rotation.i)
                        || !approx_eq_f32(current_pos.rotation.j, target_pos.rotation.j)
                        || !approx_eq_f32(current_pos.rotation.k, target_pos.rotation.k)
                        || !approx_eq_f32(current_pos.rotation.w, target_pos.rotation.w);
                if pos_changed {
                    rb.set_position(target_pos, true);
                }

                if let Some(rigid) = body.rigid {
                    let target_lin = na3::Vector3::new(
                        rigid.linear_velocity.x,
                        rigid.linear_velocity.y,
                        rigid.linear_velocity.z,
                    );
                    let current_lin = rb.linvel();
                    if !approx_eq_f32(current_lin.x, target_lin.x)
                        || !approx_eq_f32(current_lin.y, target_lin.y)
                        || !approx_eq_f32(current_lin.z, target_lin.z)
                    {
                        rb.set_linvel(target_lin, true);
                    }

                    let target_ang = na3::Vector3::new(
                        rigid.angular_velocity.x,
                        rigid.angular_velocity.y,
                        rigid.angular_velocity.z,
                    );
                    let current_ang = rb.angvel();
                    if !approx_eq_f32(current_ang.x, target_ang.x)
                        || !approx_eq_f32(current_ang.y, target_ang.y)
                        || !approx_eq_f32(current_ang.z, target_ang.z)
                    {
                        rb.set_angvel(target_ang, true);
                    }
                    if !approx_eq_f32(rb.gravity_scale(), rigid.gravity_scale) {
                        rb.set_gravity_scale(rigid.gravity_scale, true);
                    }
                    if !approx_eq_f32(rb.linear_damping(), rigid.linear_damping) {
                        rb.set_linear_damping(rigid.linear_damping);
                    }
                    if !approx_eq_f32(rb.angular_damping(), rigid.angular_damping) {
                        rb.set_angular_damping(rigid.angular_damping);
                    }
                    rb.set_additional_mass(rigid.mass.max(0.0), true);
                    let target_speed_sq = target_lin.norm_squared();
                    let target_ccd = rigid.continuous_collision_detection
                        && target_speed_sq >= CCD_MIN_SPEED_SQ_3D;
                    if rb.is_ccd_enabled() != target_ccd {
                        rb.enable_ccd(target_ccd);
                    }
                } else {
                    if rb.is_ccd_enabled() {
                        rb.enable_ccd(false);
                    }
                }
            }

            if state.shape_signature != body.shape_signature {
                for handle in state.colliders.drain(..) {
                    world.collider_owners.remove(&handle);
                    let _ =
                        world
                            .colliders
                            .remove(handle, &mut world.islands, &mut world.bodies, true);
                }

                for shape in &body.shapes {
                    let Some(builder) = collider_builder_3d(
                        shape,
                        self.provider_mode,
                        static_mesh_lookup,
                        static_collision_trimesh_lookup,
                        &mut self.physics.trimesh_cache,
                    ) else {
                        continue;
                    };
                    let handle = world.colliders.insert_with_parent(
                        builder,
                        state.handle,
                        &mut world.bodies,
                    );
                    world.collider_owners.insert(handle, body.id);
                    state.colliders.push(handle);
                }
                state.shape_signature = body.shape_signature;
            }
        }

        let mut stale = std::mem::take(&mut self.physics.stale_ids_3d);
        stale.clear();
        stale.extend(
            world
                .body_map
                .keys()
                .copied()
                .filter(|id| !alive.contains(id)),
        );

        for id in stale.iter().copied() {
            if let Some(state) = world.body_map.remove(&id) {
                for handle in &state.colliders {
                    world.collider_owners.remove(handle);
                }
                let _ = world.bodies.remove(
                    state.handle,
                    &mut world.islands,
                    &mut world.colliders,
                    &mut world.impulse_joints,
                    &mut world.multibody_joints,
                    true,
                );
            }
            self.set_body_handle_3d(id, None);
        }
        stale.clear();
        self.physics.stale_ids_3d = stale;
        self.physics.world_3d = Some(world);
    }

    fn sync_joints_2d(&mut self, joints: &[JointDesc2D]) {
        let Some(world) = self.physics.world_2d.as_mut() else {
            return;
        };
        let mut alive = AHashSet::default();
        for joint in joints {
            alive.insert(joint.id);
            if !joint.enabled || joint.body_a.is_nil() || joint.body_b.is_nil() {
                remove_joint_2d(world, joint.id);
                continue;
            }
            let Some(body_a) = world.body_map.get(&joint.body_a).map(|state| state.handle) else {
                remove_joint_2d(world, joint.id);
                continue;
            };
            let Some(body_b) = world.body_map.get(&joint.body_b).map(|state| state.handle) else {
                remove_joint_2d(world, joint.id);
                continue;
            };
            if world
                .joint_map
                .get(&joint.id)
                .map(|state| state.signature == joint.signature)
                .unwrap_or(false)
            {
                continue;
            }
            remove_joint_2d(world, joint.id);
            let data = build_joint_2d(joint);
            let handle = world.impulse_joints.insert(body_a, body_b, data, true);
            world.joint_map.insert(
                joint.id,
                JointState2D {
                    handle,
                    signature: joint.signature,
                },
            );
        }

        let stale = world
            .joint_map
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect::<Vec<_>>();
        for id in stale {
            remove_joint_2d(world, id);
        }
    }

    fn sync_joints_3d(&mut self, joints: &[JointDesc3D]) {
        let Some(world) = self.physics.world_3d.as_mut() else {
            return;
        };
        let mut alive = AHashSet::default();
        for joint in joints {
            alive.insert(joint.id);
            if !joint.enabled || joint.body_a.is_nil() || joint.body_b.is_nil() {
                remove_joint_3d(world, joint.id);
                continue;
            }
            let Some(body_a) = world.body_map.get(&joint.body_a).map(|state| state.handle) else {
                remove_joint_3d(world, joint.id);
                continue;
            };
            let Some(body_b) = world.body_map.get(&joint.body_b).map(|state| state.handle) else {
                remove_joint_3d(world, joint.id);
                continue;
            };
            if world
                .joint_map
                .get(&joint.id)
                .map(|state| state.signature == joint.signature)
                .unwrap_or(false)
            {
                continue;
            }
            remove_joint_3d(world, joint.id);
            let data = build_joint_3d(joint);
            let handle = world.impulse_joints.insert(body_a, body_b, data, true);
            world.joint_map.insert(
                joint.id,
                JointState3D {
                    handle,
                    signature: joint.signature,
                },
            );
        }

        let stale = world
            .joint_map
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect::<Vec<_>>();
        for id in stale {
            remove_joint_3d(world, id);
        }
    }

    fn step_world_2d(&mut self) {
        let gravity_y = self.physics_gravity();
        let Some(world) = self.physics.world_2d.as_mut() else {
            return;
        };
        world.gravity.y = gravity_y;
        world.integration_parameters.dt = self.time.fixed_delta.max(0.000_1);
        world.pipeline.step(
            &world.gravity,
            &world.integration_parameters,
            &mut world.islands,
            &mut world.broad_phase,
            &mut world.narrow_phase,
            &mut world.bodies,
            &mut world.colliders,
            &mut world.impulse_joints,
            &mut world.multibody_joints,
            &mut world.ccd_solver,
            None,
            &(),
            &(),
        );
    }

    fn step_world_3d(&mut self) {
        let gravity_y = self.physics_gravity();
        let Some(world) = self.physics.world_3d.as_mut() else {
            return;
        };
        world.gravity.y = gravity_y;
        world.integration_parameters.dt = self.time.fixed_delta.max(0.000_1);
        world.pipeline.step(
            &world.gravity,
            &world.integration_parameters,
            &mut world.islands,
            &mut world.broad_phase,
            &mut world.narrow_phase,
            &mut world.bodies,
            &mut world.colliders,
            &mut world.impulse_joints,
            &mut world.multibody_joints,
            &mut world.ccd_solver,
            None,
            &(),
            &(),
        );
    }

    fn apply_pending_impulses_2d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_impulses_2d);
        let coef = self.physics_coef();
        let Some(world) = self.physics.world_2d.as_mut() else {
            return;
        };
        for impulse in pending.drain(..) {
            let Some(state) = world.body_map.get(&impulse.id) else {
                continue;
            };
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq =
                impulse.impulse.x * impulse.impulse.x + impulse.impulse.y * impulse.impulse.y;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na2::Vector2::new(impulse.impulse.x * coef, impulse.impulse.y * coef),
                true,
            );
            clamp_rb_speed_2d(rb, MAX_RIGID_SPEED_2D);
        }
        self.physics.pending_impulses_2d = pending;
    }

    fn apply_pending_forces_2d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_forces_2d);
        let coef = self.physics_coef();
        let Some(world) = self.physics.world_2d.as_mut() else {
            return;
        };
        let dt = self.time.fixed_delta.max(0.000_1);
        for force in pending.drain(..) {
            let Some(state) = world.body_map.get(&force.id) else {
                continue;
            };
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq = force.force.x * force.force.x + force.force.y * force.force.y;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na2::Vector2::new(force.force.x * dt * coef, force.force.y * dt * coef),
                true,
            );
            clamp_rb_speed_2d(rb, MAX_RIGID_SPEED_2D);
        }
        self.physics.pending_forces_2d = pending;
    }

    fn apply_pending_impulses_3d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_impulses_3d);
        let coef = self.physics_coef();
        let Some(world) = self.physics.world_3d.as_mut() else {
            return;
        };
        for impulse in pending.drain(..) {
            let Some(state) = world.body_map.get(&impulse.id) else {
                continue;
            };
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq = impulse.impulse.x * impulse.impulse.x
                + impulse.impulse.y * impulse.impulse.y
                + impulse.impulse.z * impulse.impulse.z;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na3::Vector3::new(
                    impulse.impulse.x * coef,
                    impulse.impulse.y * coef,
                    impulse.impulse.z * coef,
                ),
                true,
            );
            clamp_rb_speed_3d(rb, MAX_RIGID_SPEED_3D);
        }
        self.physics.pending_impulses_3d = pending;
    }

    fn apply_pending_forces_3d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_forces_3d);
        let coef = self.physics_coef();
        let Some(world) = self.physics.world_3d.as_mut() else {
            return;
        };
        let dt = self.time.fixed_delta.max(0.000_1);
        for force in pending.drain(..) {
            let Some(state) = world.body_map.get(&force.id) else {
                continue;
            };
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(rb) = world.bodies.get_mut(state.handle) else {
                continue;
            };
            let len_sq = force.force.x * force.force.x
                + force.force.y * force.force.y
                + force.force.z * force.force.z;
            if len_sq <= 0.000_001 {
                continue;
            }
            rb.apply_impulse(
                na3::Vector3::new(
                    force.force.x * dt * coef,
                    force.force.y * dt * coef,
                    force.force.z * dt * coef,
                ),
                true,
            );
            clamp_rb_speed_3d(rb, MAX_RIGID_SPEED_3D);
        }
        self.physics.pending_forces_3d = pending;
    }

    fn sync_world_to_nodes_2d(&mut self) {
        let Some(world) = self.physics.world_2d.take() else {
            return;
        };

        for (&id, state) in &world.body_map {
            self.set_body_handle_2d(id, Some(state.opaque_handle));
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(body) = world.bodies.get(state.handle) else {
                continue;
            };
            let position = Vector2::new(body.translation().x, body.translation().y);
            let rotation = body.rotation().angle();
            let lin = Vector2::new(body.linvel().x, body.linvel().y);
            let ang = body.angvel();

            let mut target = self
                .get_global_transform_2d(id)
                .unwrap_or(Transform2D::IDENTITY);
            target.position = position;
            target.rotation = rotation;
            let _ = NodeAPI::set_global_transform_2d(self, id, target);

            if let Some(scene_node) = self.nodes.get_mut(id)
                && let SceneNodeData::RigidBody2D(node) = &mut scene_node.data
            {
                node.linear_velocity = lin;
                node.angular_velocity = ang;
            }
        }

        self.physics.world_2d = Some(world);
    }

    fn sync_world_to_nodes_3d(&mut self) {
        let Some(world) = self.physics.world_3d.take() else {
            return;
        };

        for (&id, state) in &world.body_map {
            self.set_body_handle_3d(id, Some(state.opaque_handle));
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(body) = world.bodies.get(state.handle) else {
                continue;
            };
            let position = Vector3::new(
                body.translation().x,
                body.translation().y,
                body.translation().z,
            );
            let rot = body.rotation();
            let rotation = Quaternion::new(rot.i, rot.j, rot.k, rot.w);
            let lin = Vector3::new(body.linvel().x, body.linvel().y, body.linvel().z);
            let ang = Vector3::new(body.angvel().x, body.angvel().y, body.angvel().z);

            let mut target = self
                .get_global_transform_3d(id)
                .unwrap_or(Transform3D::IDENTITY);
            target.position = position;
            target.rotation = rotation;
            let _ = NodeAPI::set_global_transform_3d(self, id, target);

            if let Some(scene_node) = self.nodes.get_mut(id)
                && let SceneNodeData::RigidBody3D(node) = &mut scene_node.data
            {
                node.linear_velocity = lin;
                node.angular_velocity = ang;
            }
        }

        self.physics.world_3d = Some(world);
    }

    fn set_body_handle_2d(&mut self, id: NodeID, handle: Option<u64>) {
        if let Some(node) = self.nodes.get_mut(id) {
            match &mut node.data {
                SceneNodeData::StaticBody2D(body) => body.physics_handle = handle,
                SceneNodeData::Area2D(body) => body.physics_handle = handle,
                SceneNodeData::RigidBody2D(body) => body.physics_handle = handle,
                _ => {}
            }
        }
    }

    fn set_body_handle_3d(&mut self, id: NodeID, handle: Option<u64>) {
        if let Some(node) = self.nodes.get_mut(id) {
            match &mut node.data {
                SceneNodeData::StaticBody3D(body) => body.physics_handle = handle,
                SceneNodeData::Area3D(body) => body.physics_handle = handle,
                SceneNodeData::RigidBody3D(body) => body.physics_handle = handle,
                _ => {}
            }
        }
    }

    fn physics_gravity(&self) -> f32 {
        self.project()
            .map(|p| p.config.physics_gravity)
            .filter(|v| v.is_finite())
            .unwrap_or(-9.81)
            * self.physics_coef()
    }

    fn physics_coef(&self) -> f32 {
        self.project()
            .map(|p| p.config.physics_coef)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0)
    }

    fn emit_collision_signals_2d(&mut self) {
        let Some(world) = self.physics.world_2d.as_ref() else {
            self.physics.active_collision_pairs_2d.clear();
            return;
        };
        let mut current_pairs = AHashSet::default();
        let mut entered_pairs = Vec::new();

        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            if a == b {
                continue;
            }

            let key = BodyPair::sorted(a, b);
            current_pairs.insert(key);
            if !self.physics.active_collision_pairs_2d.contains(&key) {
                entered_pairs.push(key);
            }
        }

        self.physics.active_collision_pairs_2d = current_pairs;
        self.emit_collision_signals_for_pairs(&entered_pairs);
    }

    fn emit_collision_signals_3d(&mut self) {
        let Some(world) = self.physics.world_3d.as_ref() else {
            self.physics.active_collision_pairs_3d.clear();
            return;
        };
        let mut current_pairs = AHashSet::default();
        let mut entered_pairs = Vec::new();

        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            if a == b {
                continue;
            }

            let key = BodyPair::sorted(a, b);
            current_pairs.insert(key);
            if !self.physics.active_collision_pairs_3d.contains(&key) {
                entered_pairs.push(key);
            }
        }

        self.physics.active_collision_pairs_3d = current_pairs;
        self.emit_collision_signals_for_pairs(&entered_pairs);
    }

    fn emit_collision_signals_for_pairs(&mut self, pairs: &[BodyPair]) {
        for pair in pairs {
            self.emit_collision_signal_for_node(pair.a, pair.b);
            self.emit_collision_signal_for_node(pair.b, pair.a);
        }
    }

    fn emit_collision_signal_for_node(&mut self, source: NodeID, other: NodeID) {
        let signal_id = {
            let Some(node) = self.nodes.get(source) else {
                return;
            };
            if node.name.is_empty() {
                return;
            }
            let signal_name = format!("{}_Collided", node.name);
            SignalID::from_string(&signal_name)
        };

        let params = [Variant::from(source), Variant::from(other)];
        let _ = SignalAPI::signal_emit(self, signal_id, &params);
    }

    fn emit_area_signals_2d(&mut self) {
        let Some(world) = self.physics.world_2d.as_ref() else {
            self.physics.active_area_overlaps_2d.clear();
            return;
        };
        let mut current = AHashSet::default();

        for (collider_a, collider_b, intersecting) in world.narrow_phase.intersection_pairs() {
            if !intersecting {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&collider_a) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&collider_b) else {
                continue;
            };
            if a == b {
                continue;
            }

            let kind_a = world.body_map.get(&a).map(|state| state.kind);
            let kind_b = world.body_map.get(&b).map(|state| state.kind);

            if kind_a == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: a, other: b });
            }
            if kind_b == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: b, other: a });
            }
        }

        self.emit_area_overlap_signals(current, true);
    }

    fn emit_area_signals_3d(&mut self) {
        let Some(world) = self.physics.world_3d.as_ref() else {
            self.physics.active_area_overlaps_3d.clear();
            return;
        };
        let mut current = AHashSet::default();

        for (collider_a, collider_b, intersecting) in world.narrow_phase.intersection_pairs() {
            if !intersecting {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&collider_a) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&collider_b) else {
                continue;
            };
            if a == b {
                continue;
            }

            let kind_a = world.body_map.get(&a).map(|state| state.kind);
            let kind_b = world.body_map.get(&b).map(|state| state.kind);

            if kind_a == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: a, other: b });
            }
            if kind_b == Some(BodyKind::Area) {
                current.insert(AreaOverlap { area: b, other: a });
            }
        }

        self.emit_area_overlap_signals(current, false);
    }

    fn emit_area_overlap_signals(&mut self, current: AHashSet<AreaOverlap>, is_2d: bool) {
        let previous = if is_2d {
            std::mem::take(&mut self.physics.active_area_overlaps_2d)
        } else {
            std::mem::take(&mut self.physics.active_area_overlaps_3d)
        };

        for overlap in current.iter().copied() {
            if !previous.contains(&overlap) {
                self.emit_area_signal(overlap.area, overlap.other, "Entered");
            }
            self.emit_area_signal(overlap.area, overlap.other, "Occupied");
        }

        for overlap in previous.iter().copied() {
            if !current.contains(&overlap) {
                self.emit_area_signal(overlap.area, overlap.other, "Exited");
            }
        }

        if is_2d {
            self.physics.active_area_overlaps_2d = current;
        } else {
            self.physics.active_area_overlaps_3d = current;
        }
    }

    fn emit_area_signal(&mut self, area: NodeID, other: NodeID, action: &str) {
        let signal_id = {
            let Some(node) = self.nodes.get(area) else {
                return;
            };
            if node.name.is_empty() {
                return;
            }
            let signal_name = format!("{}_{}", node.name, action);
            SignalID::from_string(&signal_name)
        };

        let params = [Variant::from(area), Variant::from(other)];
        let _ = SignalAPI::signal_emit(self, signal_id, &params);
    }
}

fn body_signature_seed(kind: BodyKind) -> u64 {
    match kind {
        BodyKind::Static => 0xA91B_D58C_24F1_7E31,
        BodyKind::Area => 0xCC42_83B7_9E20_11DD,
        BodyKind::Rigid => 0x6D1E_93A4_F02C_B871,
    }
}

fn hash_u64(mut state: u64, value: u64) -> u64 {
    state ^= value.wrapping_mul(0x9E37_79B1_85EB_CA87);
    state.rotate_left(17)
}

fn hash_f32(state: u64, bits: u32) -> u64 {
    hash_u64(state, bits as u64)
}

fn hash_u32(state: u64, value: u32) -> u64 {
    hash_u64(state, value as u64)
}

fn hash_transform_2d(mut state: u64, transform: Transform2D) -> u64 {
    state = hash_f32(state, transform.position.x.to_bits());
    state = hash_f32(state, transform.position.y.to_bits());
    state = hash_f32(state, transform.rotation.to_bits());
    state = hash_f32(state, transform.scale.x.to_bits());
    hash_f32(state, transform.scale.y.to_bits())
}

fn hash_transform_3d(mut state: u64, transform: Transform3D) -> u64 {
    state = hash_f32(state, transform.position.x.to_bits());
    state = hash_f32(state, transform.position.y.to_bits());
    state = hash_f32(state, transform.position.z.to_bits());
    state = hash_f32(state, transform.rotation.x.to_bits());
    state = hash_f32(state, transform.rotation.y.to_bits());
    state = hash_f32(state, transform.rotation.z.to_bits());
    state = hash_f32(state, transform.rotation.w.to_bits());
    state = hash_f32(state, transform.scale.x.to_bits());
    state = hash_f32(state, transform.scale.y.to_bits());
    hash_f32(state, transform.scale.z.to_bits())
}

fn hash_shape_2d(state: u64, shape: Shape2D) -> u64 {
    match shape {
        Shape2D::Quad { width, height } => {
            let state = hash_u64(state, 1);
            let state = hash_f32(state, width.to_bits());
            hash_f32(state, height.to_bits())
        }
        Shape2D::Circle { radius } => {
            let state = hash_u64(state, 2);
            hash_f32(state, radius.to_bits())
        }
        Shape2D::Triangle {
            kind,
            width,
            height,
        } => {
            let state = hash_u64(state, 3);
            let kind_tag = match kind {
                Triangle2DKind::Equilateral => 1,
                Triangle2DKind::Right => 2,
                Triangle2DKind::Isosceles => 3,
            };
            let state = hash_u64(state, kind_tag);
            let state = hash_f32(state, width.to_bits());
            hash_f32(state, height.to_bits())
        }
    }
}

fn hash_shape_3d(state: u64, shape: &Shape3D) -> u64 {
    match shape {
        Shape3D::Cube { size } => {
            let state = hash_u64(state, 1);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::Sphere { radius } => {
            let state = hash_u64(state, 2);
            hash_f32(state, radius.to_bits())
        }
        Shape3D::Capsule {
            radius,
            half_height,
        } => {
            let state = hash_u64(state, 3);
            let state = hash_f32(state, radius.to_bits());
            hash_f32(state, half_height.to_bits())
        }
        Shape3D::Cylinder {
            radius,
            half_height,
        } => {
            let state = hash_u64(state, 4);
            let state = hash_f32(state, radius.to_bits());
            hash_f32(state, half_height.to_bits())
        }
        Shape3D::Cone {
            radius,
            half_height,
        } => {
            let state = hash_u64(state, 5);
            let state = hash_f32(state, radius.to_bits());
            hash_f32(state, half_height.to_bits())
        }
        Shape3D::TriPrism { size } => {
            let state = hash_u64(state, 6);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::TriangularPyramid { size } => {
            let state = hash_u64(state, 7);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::SquarePyramid { size } => {
            let state = hash_u64(state, 8);
            let state = hash_f32(state, size.x.to_bits());
            let state = hash_f32(state, size.y.to_bits());
            hash_f32(state, size.z.to_bits())
        }
        Shape3D::TriMesh { source } => {
            let mut state = hash_u64(state, 9);
            for b in source.as_bytes() {
                state = hash_u64(state, *b as u64);
            }
            state
        }
    }
}

fn hash_collision_shape_2d(state: u64, shape: &CollisionShape2D, kind: BodyKind) -> u64 {
    let mut state = hash_u64(state, (kind == BodyKind::Area) as u64);
    state = hash_transform_2d(state, shape.base.transform);
    hash_shape_2d(state, shape.shape)
}

fn hash_tilemap_2d(mut state: u64, tilemap: &TileMap2D) -> u64 {
    state = hash_u32(state, tilemap.width);
    state = hash_u32(state, tilemap.height);
    state = hash_u64(state, tilemap.empty_tile as u64);
    for tile in &tilemap.tiles {
        state = hash_u64(state, *tile as u64);
    }
    for b in tilemap.tileset.as_bytes() {
        state = hash_u64(state, *b as u64);
    }
    state
}

fn hash_tile_collision_shape_2d(
    mut state: u64,
    shape: crate::runtime::render_2d::ParsedTileCollisionShape2D,
) -> u64 {
    use crate::runtime::render_2d::ParsedTileCollisionShape2D;

    match shape {
        ParsedTileCollisionShape2D::Auto => hash_u32(state, 1),
        ParsedTileCollisionShape2D::Shape { shape, offset } => {
            state = hash_u32(state, 2);
            state = hash_shape_2d(state, tile_set_shape_to_shape_2d(shape));
            state = hash_f32(state, offset[0].to_bits());
            hash_f32(state, offset[1].to_bits())
        }
        ParsedTileCollisionShape2D::Polygon { points, offset } => {
            state = hash_u32(state, 3);
            state = hash_f32(state, offset[0].to_bits());
            state = hash_f32(state, offset[1].to_bits());
            for point in points.iter() {
                state = hash_f32(state, point.x.to_bits());
                state = hash_f32(state, point.y.to_bits());
            }
            state
        }
    }
}

fn tile_set_shape_to_shape_2d(shape: crate::runtime::render_2d::TileSetShape2D) -> Shape2D {
    use crate::runtime::render_2d::TileSetShape2D;

    match shape {
        TileSetShape2D::Rect { width, height } => Shape2D::Quad { width, height },
        TileSetShape2D::Circle { radius } => Shape2D::Circle { radius },
        TileSetShape2D::Triangle { width, height } => Shape2D::Triangle {
            kind: Triangle2DKind::Isosceles,
            width,
            height,
        },
    }
}

fn tilemap_shape_descs_2d(
    tilemap: &TileMap2D,
    layer: u32,
    mask: u32,
    friction: f32,
    restitution: f32,
    tileset: Option<&crate::runtime::render_2d::ParsedTileset2D>,
) -> Vec<ShapeDesc2D> {
    let Some(tileset) = tileset else {
        return Vec::new();
    };
    let width = tilemap.width as usize;
    let height = tilemap.height as usize;
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let tw = tileset.tile_size[0];
    let th = tileset.tile_size[1];
    use crate::runtime::render_2d::ParsedTileCollisionShape2D;

    let mut solid = vec![false; width.saturating_mul(height)];
    let mut explicit = Vec::new();
    for (idx, tile_id) in tilemap.tiles.iter().take(solid.len()).copied().enumerate() {
        if tile_id == tilemap.empty_tile {
            continue;
        }
        let Some(tile) = tileset.tile(tile_id) else {
            continue;
        };
        if !tile.collision {
            continue;
        }
        match tile.collision_shape.clone() {
            ParsedTileCollisionShape2D::Auto => solid[idx] = true,
            ParsedTileCollisionShape2D::Shape { shape, offset } => {
                explicit.push((
                    idx,
                    ShapeKind2D::Primitive(tile_set_shape_to_shape_2d(shape)),
                    offset,
                ));
            }
            ParsedTileCollisionShape2D::Polygon { points, offset } => {
                explicit.push((idx, ShapeKind2D::Polygon(points.to_vec()), offset));
            }
        }
    }

    let mut out = Vec::new();
    let mut used = vec![false; solid.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if !solid[idx] || used[idx] {
                continue;
            }
            let mut run_w = 1usize;
            while x + run_w < width && solid[idx + run_w] && !used[idx + run_w] {
                run_w += 1;
            }
            let mut run_h = 1usize;
            'grow: while y + run_h < height {
                for ox in 0..run_w {
                    let n = (y + run_h) * width + x + ox;
                    if !solid[n] || used[n] {
                        break 'grow;
                    }
                }
                run_h += 1;
            }
            for yy in y..(y + run_h) {
                for xx in x..(x + run_w) {
                    used[yy * width + xx] = true;
                }
            }
            let w = run_w as f32 * tw;
            let h = run_h as f32 * th;
            out.push(ShapeDesc2D {
                local: Transform2D::new(
                    Vector2::new(x as f32 * tw + w * 0.5, -(y as f32 * th + h * 0.5)),
                    0.0,
                    Vector2::ONE,
                ),
                shape: ShapeKind2D::Primitive(Shape2D::Quad {
                    width: w,
                    height: h,
                }),
                sensor: false,
                collision_layer: layer,
                collision_mask: mask,
                friction,
                restitution,
            });
        }
    }
    for (idx, shape, offset) in explicit {
        let x = idx % width;
        let y = idx / width;
        out.push(ShapeDesc2D {
            local: Transform2D::new(
                Vector2::new(
                    x as f32 * tw + tw * 0.5 + offset[0],
                    -(y as f32 * th + th * 0.5 + offset[1]),
                ),
                0.0,
                Vector2::ONE,
            ),
            shape,
            sensor: false,
            collision_layer: layer,
            collision_mask: mask,
            friction,
            restitution,
        });
    }
    out
}

fn hash_collision_shape_3d(
    state: u64,
    shape: &CollisionShape3D,
    kind: BodyKind,
    inherited_scale: Vector3,
) -> u64 {
    let mut state = hash_u64(state, (kind == BodyKind::Area) as u64);
    let mut transform = shape.base.transform;
    transform.scale = Vector3::new(
        transform.scale.x * inherited_scale.x,
        transform.scale.y * inherited_scale.y,
        transform.scale.z * inherited_scale.z,
    );
    state = hash_transform_3d(state, transform);
    hash_shape_3d(state, &shape.shape)
}

fn shape_desc_2d(shape: &CollisionShape2D, friction: f32, restitution: f32) -> ShapeDesc2D {
    ShapeDesc2D {
        local: shape.base.transform,
        shape: ShapeKind2D::Primitive(shape.shape),
        sensor: false,
        collision_layer: 1,
        collision_mask: u32::MAX,
        friction,
        restitution,
    }
}

fn shape_desc_3d(shape: &CollisionShape3D, friction: f32, restitution: f32) -> ShapeDesc3D {
    ShapeDesc3D {
        local: shape.base.transform,
        shape: match &shape.shape {
            Shape3D::TriMesh { source } => ShapeKind3D::TriMesh {
                source: source.clone(),
            },
            _ => ShapeKind3D::Primitive(shape.shape.clone()),
        },
        sensor: false,
        collision_layer: 1,
        collision_mask: u32::MAX,
        friction,
        restitution,
    }
}

fn approx_eq_f32(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.000_01
}

fn clamp_rb_speed_2d(rb: &mut r2::RigidBody, max_speed: f32) {
    if max_speed <= 0.0 {
        return;
    }
    let current = *rb.linvel();
    let speed_sq = current.norm_squared();
    let max_sq = max_speed * max_speed;
    if speed_sq <= max_sq || speed_sq <= 0.0 {
        return;
    }
    let scale = max_speed / speed_sq.sqrt();
    rb.set_linvel(current * scale, true);
}

fn clamp_rb_speed_3d(rb: &mut r3::RigidBody, max_speed: f32) {
    if max_speed <= 0.0 {
        return;
    }
    let current = *rb.linvel();
    let speed_sq = current.norm_squared();
    let max_sq = max_speed * max_speed;
    if speed_sq <= max_sq || speed_sq <= 0.0 {
        return;
    }
    let scale = max_speed / speed_sq.sqrt();
    rb.set_linvel(current * scale, true);
}

fn build_rigid_body_2d(desc: &BodyDesc2D) -> r2::RigidBody {
    let mut builder = match desc.kind {
        BodyKind::Static => r2::RigidBodyBuilder::fixed(),
        BodyKind::Area => r2::RigidBodyBuilder::fixed(),
        BodyKind::Rigid => r2::RigidBodyBuilder::dynamic(),
    }
    .position(transform_to_iso2(desc.global))
    .enabled(desc.enabled);

    if let Some(rigid) = desc.rigid.as_ref() {
        builder = builder
            .linvel(na2::Vector2::new(
                rigid.linear_velocity.x,
                rigid.linear_velocity.y,
            ))
            .angvel(rigid.angular_velocity)
            .gravity_scale(rigid.gravity_scale)
            .linear_damping(rigid.linear_damping)
            .angular_damping(rigid.angular_damping)
            .ccd_enabled(rigid.continuous_collision_detection)
            .can_sleep(rigid.can_sleep)
            .enabled(rigid.enabled);
        if rigid.lock_rotation {
            builder = builder.lock_rotations();
        }
    }

    builder.build()
}

fn build_rigid_body_3d(desc: &BodyDesc3D) -> r3::RigidBody {
    let mut builder = match desc.kind {
        BodyKind::Static => r3::RigidBodyBuilder::fixed(),
        BodyKind::Area => r3::RigidBodyBuilder::fixed(),
        BodyKind::Rigid => r3::RigidBodyBuilder::dynamic(),
    }
    .position(transform_to_iso3(desc.global))
    .enabled(desc.enabled);

    if let Some(rigid) = desc.rigid.as_ref() {
        builder = builder
            .linvel(na3::Vector3::new(
                rigid.linear_velocity.x,
                rigid.linear_velocity.y,
                rigid.linear_velocity.z,
            ))
            .angvel(na3::Vector3::new(
                rigid.angular_velocity.x,
                rigid.angular_velocity.y,
                rigid.angular_velocity.z,
            ))
            .gravity_scale(rigid.gravity_scale)
            .linear_damping(rigid.linear_damping)
            .angular_damping(rigid.angular_damping)
            .additional_mass(rigid.mass.max(0.0))
            .ccd_enabled(rigid.continuous_collision_detection)
            .can_sleep(rigid.can_sleep)
            .enabled(rigid.enabled);
    }

    builder.build()
}

fn collider_builder_2d(desc: &ShapeDesc2D) -> Option<r2::Collider> {
    let sx = desc.local.scale.x.abs().max(0.0001);
    let sy = desc.local.scale.y.abs().max(0.0001);
    let shape = match &desc.shape {
        ShapeKind2D::Primitive(Shape2D::Quad { width, height }) => r2::ColliderBuilder::cuboid(
            width.abs().max(0.0001) * sx * 0.5,
            height.abs().max(0.0001) * sy * 0.5,
        ),
        ShapeKind2D::Primitive(Shape2D::Circle { radius }) => {
            let scale = sx.max(sy);
            r2::ColliderBuilder::ball(radius.abs().max(0.0001) * scale)
        }
        ShapeKind2D::Primitive(Shape2D::Triangle {
            kind,
            width,
            height,
        }) => {
            let points = triangle_points_2d(*kind, width * sx, height * sy)?;
            r2::ColliderBuilder::triangle(points[0], points[1], points[2])
        }
        ShapeKind2D::Polygon(points) => {
            let points = points
                .iter()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
                .map(|p| na2::Point2::new(p.x * sx, p.y * sy))
                .collect::<Vec<_>>();
            r2::ColliderBuilder::convex_hull(&points)?
        }
    };

    Some(
        shape
            .position(na2::Isometry2::new(
                na2::Vector2::new(desc.local.position.x, desc.local.position.y),
                desc.local.rotation,
            ))
            .sensor(desc.sensor)
            .collision_groups(interaction_groups_2d(
                desc.collision_layer,
                desc.collision_mask,
            ))
            .friction(desc.friction)
            .restitution(desc.restitution)
            .build(),
    )
}

fn shared_shape_2d(shape: Shape2D) -> Option<r2::SharedShape> {
    match shape {
        Shape2D::Quad { width, height } => Some(r2::SharedShape::cuboid(
            width.abs().max(0.0001) * 0.5,
            height.abs().max(0.0001) * 0.5,
        )),
        Shape2D::Circle { radius } => Some(r2::SharedShape::ball(radius.abs().max(0.0001))),
        Shape2D::Triangle {
            kind,
            width,
            height,
        } => {
            let points = triangle_points_2d(kind, width, height)?;
            Some(r2::SharedShape::triangle(points[0], points[1], points[2]))
        }
    }
}

fn shared_shape_3d(shape: Shape3D) -> Option<r3::SharedShape> {
    match shape {
        Shape3D::Cube { size } => Some(r3::SharedShape::cuboid(
            size.x.abs().max(0.0001) * 0.5,
            size.y.abs().max(0.0001) * 0.5,
            size.z.abs().max(0.0001) * 0.5,
        )),
        Shape3D::Sphere { radius } => Some(r3::SharedShape::ball(radius.abs().max(0.0001))),
        Shape3D::Capsule {
            radius,
            half_height,
        } => Some(r3::SharedShape::capsule_y(
            half_height.abs().max(0.0001),
            radius.abs().max(0.0001),
        )),
        Shape3D::Cylinder {
            radius,
            half_height,
        } => Some(r3::SharedShape::cylinder(
            half_height.abs().max(0.0001),
            radius.abs().max(0.0001),
        )),
        Shape3D::Cone {
            radius,
            half_height,
        } => Some(r3::SharedShape::cone(
            half_height.abs().max(0.0001),
            radius.abs().max(0.0001),
        )),
        Shape3D::TriPrism { size } => {
            let points = tri_prism_points(size.x, size.y, size.z);
            r3::SharedShape::convex_hull(&points)
        }
        Shape3D::TriangularPyramid { size } => {
            let points = triangular_pyramid_points(size.x, size.y, size.z);
            r3::SharedShape::convex_hull(&points)
        }
        Shape3D::SquarePyramid { size } => {
            let points = square_pyramid_points(size.x, size.y, size.z);
            r3::SharedShape::convex_hull(&points)
        }
        Shape3D::TriMesh { .. } => None,
    }
}

fn collider_builder_3d(
    desc: &ShapeDesc3D,
    provider_mode: crate::runtime_project::ProviderMode,
    static_mesh_lookup: Option<crate::runtime_project::StaticBytesLookup>,
    static_collision_trimesh_lookup: Option<crate::runtime_project::StaticBytesLookup>,
    trimesh_cache: &mut AHashMap<u64, TriMeshData>,
) -> Option<r3::Collider> {
    let sx = desc.local.scale.x.abs().max(0.0001);
    let sy = desc.local.scale.y.abs().max(0.0001);
    let sz = desc.local.scale.z.abs().max(0.0001);
    let mut trimesh_load = TrimeshLoadCtx {
        provider_mode,
        static_mesh_lookup,
        static_collision_trimesh_lookup,
        trimesh_cache,
    };

    let shape = match &desc.shape {
        ShapeKind3D::Primitive(shape) => match shape {
            Shape3D::Cube { size } => r3::ColliderBuilder::cuboid(
                size.x.abs().max(0.0001) * sx * 0.5,
                size.y.abs().max(0.0001) * sy * 0.5,
                size.z.abs().max(0.0001) * sz * 0.5,
            ),
            Shape3D::Sphere { radius } => {
                let scale = sx.max(sy).max(sz);
                r3::ColliderBuilder::ball(radius.abs().max(0.0001) * scale)
            }
            Shape3D::Capsule {
                radius,
                half_height,
            } => {
                let scale = sx.max(sz);
                r3::ColliderBuilder::capsule_y(
                    half_height.abs().max(0.0001) * sy,
                    radius.abs().max(0.0001) * scale,
                )
            }
            Shape3D::Cylinder {
                radius,
                half_height,
            } => {
                let scale = sx.max(sz);
                r3::ColliderBuilder::cylinder(
                    half_height.abs().max(0.0001) * sy,
                    radius.abs().max(0.0001) * scale,
                )
            }
            Shape3D::Cone {
                radius,
                half_height,
            } => {
                let scale = sx.max(sz);
                r3::ColliderBuilder::cone(
                    half_height.abs().max(0.0001) * sy,
                    radius.abs().max(0.0001) * scale,
                )
            }
            Shape3D::TriPrism { size } => {
                let points = tri_prism_points(size.x * sx, size.y * sy, size.z * sz);
                r3::ColliderBuilder::convex_hull(&points)?
            }
            Shape3D::TriangularPyramid { size } => {
                let points = triangular_pyramid_points(size.x * sx, size.y * sy, size.z * sz);
                r3::ColliderBuilder::convex_hull(&points)?
            }
            Shape3D::SquarePyramid { size } => {
                let points = square_pyramid_points(size.x * sx, size.y * sy, size.z * sz);
                r3::ColliderBuilder::convex_hull(&points)?
            }
            Shape3D::TriMesh { source } => {
                let (vertices, triangles) =
                    load_trimesh_from_source(source, [sx, sy, sz], &mut trimesh_load)?;
                r3::ColliderBuilder::trimesh(vertices, triangles).ok()?
            }
        },
        ShapeKind3D::TriMesh { source } => {
            let (vertices, triangles) =
                load_trimesh_from_source(source, [sx, sy, sz], &mut trimesh_load)?;
            r3::ColliderBuilder::trimesh(vertices, triangles).ok()?
        }
    };

    Some(
        shape
            .position(transform_to_iso3(desc.local))
            .sensor(desc.sensor)
            .collision_groups(interaction_groups_3d(
                desc.collision_layer,
                desc.collision_mask,
            ))
            .friction(desc.friction)
            .restitution(desc.restitution)
            .build(),
    )
}

fn interaction_groups_2d(layer: u32, mask: u32) -> r2::InteractionGroups {
    r2::InteractionGroups::new(
        r2::Group::from_bits_truncate(layer),
        r2::Group::from_bits_truncate(mask),
    )
}

fn interaction_groups_3d(layer: u32, mask: u32) -> r3::InteractionGroups {
    r3::InteractionGroups::new(
        r3::Group::from_bits_truncate(layer),
        r3::Group::from_bits_truncate(mask),
    )
}

fn query_filter_2d(filter: &PhysicsQueryFilter) -> r2::QueryFilter<'_> {
    let mut query_filter = r2::QueryFilter::new();
    if !filter.include_areas {
        query_filter = query_filter.exclude_sensors();
    }
    query_filter
}

fn query_filter_3d(filter: &PhysicsQueryFilter) -> r3::QueryFilter<'_> {
    let mut query_filter = r3::QueryFilter::new();
    if !filter.include_areas {
        query_filter = query_filter.exclude_sensors();
    }
    query_filter
}

type TriMeshData = (Vec<na3::Point3<f32>>, Vec<[u32; 3]>);

struct TrimeshLoadCtx<'a> {
    provider_mode: crate::runtime_project::ProviderMode,
    static_mesh_lookup: Option<crate::runtime_project::StaticBytesLookup>,
    static_collision_trimesh_lookup: Option<crate::runtime_project::StaticBytesLookup>,
    trimesh_cache: &'a mut AHashMap<u64, TriMeshData>,
}

fn load_trimesh_from_source(
    source: &str,
    scale: [f32; 3],
    ctx: &mut TrimeshLoadCtx<'_>,
) -> Option<TriMeshData> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let [sx, sy, sz] = scale;

    let cache_key = trimesh_cache_key(source, sx, sy, sz, ctx.provider_mode);
    if let Some(cached) = ctx.trimesh_cache.get(&cache_key) {
        return Some(cached.clone());
    }

    if ctx.provider_mode == crate::runtime_project::ProviderMode::Static
        && let Some(lookup) = ctx.static_collision_trimesh_lookup
    {
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty()
            && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
        {
            let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
            ctx.trimesh_cache.insert(cache_key, simplified.clone());
            return Some(simplified);
        }

        let normalized = normalize_source_slashes(source);
        if normalized.as_ref() != source {
            let bytes = lookup(string_to_u64(normalized.as_ref()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
        if let Some(alias) = normalized_static_mesh_lookup_alias(source) {
            let bytes = lookup(string_to_u64(alias.as_str()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
        if normalized.as_ref() != source
            && let Some(alias) = normalized_static_mesh_lookup_alias(normalized.as_ref())
        {
            let bytes = lookup(string_to_u64(alias.as_str()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
    }

    if ctx.provider_mode == crate::runtime_project::ProviderMode::Static
        && let Some(lookup) = ctx.static_mesh_lookup
    {
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty()
            && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
        {
            let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
            ctx.trimesh_cache.insert(cache_key, simplified.clone());
            return Some(simplified);
        }
    }

    let (path, fragment) = split_source_fragment(source);
    let mesh_index = if fragment.is_some() {
        parse_fragment_index(fragment, "mesh")?
    } else {
        0
    };

    let bytes = load_asset(path).ok()?;
    if path.ends_with(".pmesh") {
        let loaded = decode_pmesh_trimesh(&bytes, sx, sy, sz)?;
        let simplified = simplify_trimesh_data(loaded.0, loaded.1)?;
        ctx.trimesh_cache.insert(cache_key, simplified.clone());
        return Some(simplified);
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        let loaded = load_trimesh_from_gltf_bytes(&bytes, mesh_index, sx, sy, sz)?;
        let simplified = simplify_trimesh_data(loaded.0, loaded.1)?;
        ctx.trimesh_cache.insert(cache_key, simplified.clone());
        return Some(simplified);
    }
    None
}

fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

fn normalized_static_mesh_lookup_alias(source: &str) -> Option<String> {
    let (path, fragment) = split_source_fragment(source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    match parse_fragment_index(fragment, "mesh") {
        Some(0) => Some(path.to_string()),
        Some(_) => None,
        None => Some(format!("{path}:mesh[0]")),
    }
}

fn decode_pmesh_trimesh(bytes: &[u8], sx: f32, sy: f32, sz: f32) -> Option<TriMeshData> {
    if bytes.len() < 33 || &bytes[0..5] != b"PMESH" {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version == 8 {
        return decode_render_pmesh_v8_trimesh(bytes, sx, sy, sz);
    }
    if version != 6 && version != 7 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let payload_start = 33usize;

    let raw = decode_pmesh_payload(flags, &bytes[payload_start..])?;
    if raw.len() != raw_len {
        return None;
    }

    let index_u16 = version == 7 && (flags & (1 << 4)) != 0;
    let vertex_stride = if version == 7 {
        12
    } else {
        let has_normal = (flags & (1 << 0)) != 0;
        let has_uv0 = (flags & (1 << 1)) != 0;
        let has_joints = (flags & (1 << 2)) != 0;
        let has_weights = (flags & (1 << 3)) != 0;
        12 + if has_normal { 12 } else { 0 }
            + if has_uv0 { 8 } else { 0 }
            + if has_joints { 8 } else { 0 }
            + if has_weights { 16 } else { 0 }
    };
    let vertex_bytes = vertex_count.checked_mul(vertex_stride)?;
    let index_bytes = index_count.checked_mul(if index_u16 { 2 } else { 4 })?;
    if raw.len() < vertex_bytes + index_bytes {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let x = f32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let y = f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let z = f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        vertices.push(na3::Point3::new(x * sx, y * sy, z * sz));
    }

    let mut triangles = Vec::new();
    let index_start = vertex_bytes;
    for tri_idx in (0..index_count / 3).map(|i| i * 3) {
        let ia = read_trimesh_index(raw.as_slice(), index_start, tri_idx, index_u16)?;
        let ib = read_trimesh_index(raw.as_slice(), index_start, tri_idx + 1, index_u16)?;
        let ic = read_trimesh_index(raw.as_slice(), index_start, tri_idx + 2, index_u16)?;
        let a = ia as usize;
        let b = ib as usize;
        let c = ic as usize;
        if a >= vertices.len()
            || b >= vertices.len()
            || c >= vertices.len()
            || a == b
            || b == c
            || a == c
        {
            continue;
        }
        triangles.push([ia, ib, ic]);
    }

    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

fn decode_render_pmesh_v8_trimesh(bytes: &[u8], sx: f32, sy: f32, sz: f32) -> Option<TriMeshData> {
    if bytes.len() < 37 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let meshlet_count = u32::from_le_bytes(bytes[25..29].try_into().ok()?) as usize;
    let lod_count = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[33..37].try_into().ok()?) as usize;
    let raw = decode_pmesh_payload(flags, &bytes[37..])?;
    if raw.len() != raw_len {
        return None;
    }
    let has_normal = (flags & (1 << 0)) != 0;
    let has_uv0 = (flags & (1 << 1)) != 0;
    let has_joints = (flags & (1 << 2)) != 0;
    let has_weights = (flags & (1 << 3)) != 0;
    let stride = 12
        + if has_normal { 12 } else { 0 }
        + if has_uv0 { 8 } else { 0 }
        + if has_joints { 8 } else { 0 }
        + if has_weights { 16 } else { 0 };
    let vertex_bytes = vertex_count.checked_mul(stride)?;
    let index_bytes = index_count.checked_mul(4)?;
    let surface_bytes = surface_count.checked_mul(8)?;
    let meshlet_bytes = meshlet_count.checked_mul(24)?;
    let lod_start = vertex_bytes
        .checked_add(index_bytes)?
        .checked_add(surface_bytes)?
        .checked_add(meshlet_bytes)?;
    if raw.len() < lod_start {
        return None;
    }
    let (lod_index_start, lod_index_count) = if lod_count > 0 && raw.len() >= lod_start + 24 {
        (
            u32::from_le_bytes(raw[lod_start..lod_start + 4].try_into().ok()?) as usize,
            u32::from_le_bytes(raw[lod_start + 4..lod_start + 8].try_into().ok()?) as usize,
        )
    } else {
        (0, index_count)
    };
    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * stride;
        vertices.push(na3::Point3::new(
            f32::from_le_bytes(raw[off..off + 4].try_into().ok()?) * sx,
            f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?) * sy,
            f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?) * sz,
        ));
    }
    let index_start = vertex_bytes + lod_index_start.saturating_mul(4);
    let index_end = index_start
        .saturating_add(lod_index_count.saturating_mul(4))
        .min(vertex_bytes + index_bytes);
    let mut triangles = Vec::new();
    for off in (index_start..index_end).step_by(12) {
        if off + 12 > raw.len() {
            break;
        }
        let ia = u32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let ib = u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let ic = u32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        let a = ia as usize;
        let b = ib as usize;
        let c = ic as usize;
        if a < vertices.len()
            && b < vertices.len()
            && c < vertices.len()
            && a != b
            && b != c
            && a != c
        {
            triangles.push([ia, ib, ic]);
        }
    }
    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

fn decode_pmesh_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PMESH_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

fn read_trimesh_index(
    raw: &[u8],
    index_start: usize,
    index: usize,
    index_u16: bool,
) -> Option<u32> {
    if index_u16 {
        let off = index_start + index * 2;
        Some(u16::from_le_bytes(raw[off..off + 2].try_into().ok()?) as u32)
    } else {
        let off = index_start + index * 4;
        Some(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?))
    }
}

fn load_trimesh_from_gltf_bytes(
    bytes: &[u8],
    mesh_index: usize,
    sx: f32,
    sy: f32,
    sz: f32,
) -> Option<TriMeshData> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;

    let mut vertices = Vec::<na3::Point3<f32>>::new();
    let mut triangles = Vec::<[u32; 3]>::new();

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));
        let Some(pos_iter) = reader.read_positions() else {
            continue;
        };

        let local_positions: Vec<[f32; 3]> = pos_iter.collect();
        if local_positions.len() < 3 {
            continue;
        }

        let Ok(base) = u32::try_from(vertices.len()) else {
            return None;
        };
        for p in &local_positions {
            vertices.push(na3::Point3::new(p[0] * sx, p[1] * sy, p[2] * sz));
        }

        if let Some(indices_reader) = reader.read_indices() {
            let mut flat: Vec<u32> = indices_reader.into_u32().collect();
            let tri_len = flat.len() / 3 * 3;
            flat.truncate(tri_len);
            for tri in flat.chunks_exact(3) {
                let ia = tri[0] as usize;
                let ib = tri[1] as usize;
                let ic = tri[2] as usize;
                if ia >= local_positions.len()
                    || ib >= local_positions.len()
                    || ic >= local_positions.len()
                {
                    continue;
                }
                let a = base + tri[0];
                let b = base + tri[1];
                let c = base + tri[2];
                if a != b && b != c && a != c {
                    triangles.push([a, b, c]);
                }
            }
        } else {
            for i in (0..local_positions.len() / 3 * 3).step_by(3) {
                let a = base + i as u32;
                let b = base + i as u32 + 1;
                let c = base + i as u32 + 2;
                triangles.push([a, b, c]);
            }
        }
    }

    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

fn trimesh_cache_key(
    source: &str,
    sx: f32,
    sy: f32,
    sz: f32,
    provider_mode: crate::runtime_project::ProviderMode,
) -> u64 {
    string_to_u64(&format!(
        "{source}|{:08x}|{:08x}|{:08x}|{}",
        sx.to_bits(),
        sy.to_bits(),
        sz.to_bits(),
        provider_mode as u8
    ))
}

fn simplify_trimesh_data(
    vertices: Vec<na3::Point3<f32>>,
    triangles: Vec<[u32; 3]>,
) -> Option<TriMeshData> {
    let (vertices, triangles) = weld_and_filter_mesh(vertices, triangles)?;
    if let Some((reduced_vertices, reduced_triangles)) =
        simplify_coplanar_mesh(&vertices, &triangles)
    {
        return weld_and_filter_mesh(reduced_vertices, reduced_triangles);
    }
    Some((vertices, triangles))
}

fn weld_and_filter_mesh(
    vertices: Vec<na3::Point3<f32>>,
    triangles: Vec<[u32; 3]>,
) -> Option<TriMeshData> {
    let mut remap = vec![0u32; vertices.len()];
    let mut map = AHashMap::<(i64, i64, i64), u32>::default();
    let mut out_vertices = Vec::<na3::Point3<f32>>::new();
    let eps = 0.0001f32;
    for (idx, v) in vertices.iter().enumerate() {
        let key = (
            (v.x / eps).round() as i64,
            (v.y / eps).round() as i64,
            (v.z / eps).round() as i64,
        );
        let out_idx = if let Some(existing) = map.get(&key) {
            *existing
        } else {
            let next = out_vertices.len() as u32;
            map.insert(key, next);
            out_vertices.push(*v);
            next
        };
        remap[idx] = out_idx;
    }

    let mut unique = AHashSet::<(u32, u32, u32)>::default();
    let mut out_triangles = Vec::<[u32; 3]>::new();
    for tri in triangles {
        let a = remap.get(tri[0] as usize).copied()?;
        let b = remap.get(tri[1] as usize).copied()?;
        let c = remap.get(tri[2] as usize).copied()?;
        if a == b || b == c || a == c {
            continue;
        }
        let pa = out_vertices[a as usize];
        let pb = out_vertices[b as usize];
        let pc = out_vertices[c as usize];
        if triangle_area_sq(pa, pb, pc) <= 1.0e-12 {
            continue;
        }
        let mut ord = [a, b, c];
        ord.sort_unstable();
        if !unique.insert((ord[0], ord[1], ord[2])) {
            continue;
        }
        out_triangles.push([a, b, c]);
    }

    if out_vertices.len() < 3 || out_triangles.is_empty() {
        return None;
    }
    Some((out_vertices, out_triangles))
}

fn simplify_coplanar_mesh(
    vertices: &[na3::Point3<f32>],
    triangles: &[[u32; 3]],
) -> Option<TriMeshData> {
    if triangles.len() < 16 {
        return None;
    }
    let first = triangles[0];
    let p0 = vertices[first[0] as usize];
    let p1 = vertices[first[1] as usize];
    let p2 = vertices[first[2] as usize];
    let n = (p1 - p0).cross(&(p2 - p0));
    let n_len = n.norm();
    if n_len <= 1.0e-6 {
        return None;
    }
    let n = n / n_len;
    let plane_d = n.dot(&p0.coords);
    let plane_eps = 0.0025f32;
    for p in vertices {
        let dist = (n.dot(&p.coords) - plane_d).abs();
        if dist > plane_eps {
            return None;
        }
    }

    let axis = dominant_axis_3d(n.x, n.y, n.z);
    let mut pts2d = Vec::<[f32; 2]>::with_capacity(vertices.len());
    for p in vertices {
        pts2d.push(project_axis_3d(*p, axis));
    }

    let mut unique_2d = pts2d.clone();
    unique_2d.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    unique_2d.dedup_by(|a, b| (a[0] - b[0]).abs() <= 1.0e-5 && (a[1] - b[1]).abs() <= 1.0e-5);
    if unique_2d.len() < 3 {
        return None;
    }

    let hull = convex_hull_2d(&unique_2d);
    if hull.len() < 3 {
        return None;
    }

    let hull_area = polygon_area_abs(&hull);
    if hull_area <= 1.0e-6 {
        return None;
    }
    let mut tri_area_sum = 0.0f32;
    for tri in triangles {
        let a = pts2d[tri[0] as usize];
        let b = pts2d[tri[1] as usize];
        let c = pts2d[tri[2] as usize];
        tri_area_sum += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5;
    }
    if tri_area_sum <= 1.0e-6 {
        return None;
    }
    if hull_area > tri_area_sum * 1.1 {
        return None;
    }

    let mut new_vertices = Vec::<na3::Point3<f32>>::with_capacity(hull.len());
    for p in &hull {
        new_vertices.push(unproject_axis_on_plane(*p, axis, n, plane_d));
    }
    let mut new_triangles = Vec::<[u32; 3]>::new();
    for i in 1..hull.len() - 1 {
        new_triangles.push([0, i as u32, (i + 1) as u32]);
    }
    Some((new_vertices, new_triangles))
}

fn dominant_axis_3d(x: f32, y: f32, z: f32) -> usize {
    let ax = x.abs();
    let ay = y.abs();
    let az = z.abs();
    if ax >= ay && ax >= az {
        0
    } else if ay >= az {
        1
    } else {
        2
    }
}

fn project_axis_3d(p: na3::Point3<f32>, axis: usize) -> [f32; 2] {
    match axis {
        0 => [p.y, p.z],
        1 => [p.x, p.z],
        _ => [p.x, p.y],
    }
}

fn unproject_axis_on_plane(
    p: [f32; 2],
    axis: usize,
    n: na3::Vector3<f32>,
    d: f32,
) -> na3::Point3<f32> {
    match axis {
        0 => {
            let y = p[0];
            let z = p[1];
            let x = (d - n.y * y - n.z * z) / n.x.max(1.0e-6).copysign(n.x);
            na3::Point3::new(x, y, z)
        }
        1 => {
            let x = p[0];
            let z = p[1];
            let y = (d - n.x * x - n.z * z) / n.y.max(1.0e-6).copysign(n.y);
            na3::Point3::new(x, y, z)
        }
        _ => {
            let x = p[0];
            let y = p[1];
            let z = (d - n.x * x - n.y * y) / n.z.max(1.0e-6).copysign(n.z);
            na3::Point3::new(x, y, z)
        }
    }
}

fn convex_hull_2d(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    if pts.len() <= 3 {
        return pts;
    }
    let mut lower = Vec::<[f32; 2]>::new();
    for p in &pts {
        while lower.len() >= 2
            && cross2(
                sub2(lower[lower.len() - 1], lower[lower.len() - 2]),
                sub2(*p, lower[lower.len() - 1]),
            ) <= 0.0
        {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper = Vec::<[f32; 2]>::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2
            && cross2(
                sub2(upper[upper.len() - 1], upper[upper.len() - 2]),
                sub2(*p, upper[upper.len() - 1]),
            ) <= 0.0
        {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn polygon_area_abs(poly: &[[f32; 2]]) -> f32 {
    if poly.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0f32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        area += a[0] * b[1] - a[1] * b[0];
    }
    area.abs() * 0.5
}

fn sub2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

fn triangle_area_sq(a: na3::Point3<f32>, b: na3::Point3<f32>, c: na3::Point3<f32>) -> f32 {
    let ab = b - a;
    let ac = c - a;
    ab.cross(&ac).norm_squared() * 0.25
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() {
        return (source, None);
    }
    if selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<usize> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<usize>().ok()
}

fn triangle_points_2d(
    kind: Triangle2DKind,
    width: f32,
    height: f32,
) -> Option<[na2::Point2<f32>; 3]> {
    let w = width.abs().max(0.0001);
    let mut h = height.abs().max(0.0001);
    let points = match kind {
        Triangle2DKind::Equilateral => {
            h = h.max((3.0f32).sqrt() * 0.5 * w);
            [
                na2::Point2::new(-w * 0.5, -h / 3.0),
                na2::Point2::new(w * 0.5, -h / 3.0),
                na2::Point2::new(0.0, 2.0 * h / 3.0),
            ]
        }
        Triangle2DKind::Right => [
            na2::Point2::new(-w / 3.0, -h / 3.0),
            na2::Point2::new(2.0 * w / 3.0, -h / 3.0),
            na2::Point2::new(-w / 3.0, 2.0 * h / 3.0),
        ],
        Triangle2DKind::Isosceles => [
            na2::Point2::new(-w * 0.5, -h * 0.5),
            na2::Point2::new(w * 0.5, -h * 0.5),
            na2::Point2::new(0.0, h * 0.5),
        ],
    };
    Some(points)
}

fn tri_prism_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(0.0, hh, -hd),
        na3::Point3::new(-hw, -hh, hd),
        na3::Point3::new(hw, -hh, hd),
        na3::Point3::new(0.0, hh, hd),
    ]
}

fn triangular_pyramid_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(0.0, -hh, hd),
        na3::Point3::new(0.0, hh, 0.0),
    ]
}

fn square_pyramid_points(width: f32, height: f32, depth: f32) -> Vec<na3::Point3<f32>> {
    let hw = width.abs().max(0.0001) * 0.5;
    let hh = height.abs().max(0.0001) * 0.5;
    let hd = depth.abs().max(0.0001) * 0.5;
    vec![
        na3::Point3::new(-hw, -hh, -hd),
        na3::Point3::new(hw, -hh, -hd),
        na3::Point3::new(hw, -hh, hd),
        na3::Point3::new(-hw, -hh, hd),
        na3::Point3::new(0.0, hh, 0.0),
    ]
}

fn transform_to_iso2(transform: Transform2D) -> na2::Isometry2<f32> {
    na2::Isometry2::new(
        na2::Vector2::new(transform.position.x, transform.position.y),
        transform.rotation,
    )
}

fn transform_to_iso3(transform: Transform3D) -> na3::Isometry3<f32> {
    let rotation = na3::UnitQuaternion::from_quaternion(na3::Quaternion::new(
        transform.rotation.w,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
    ));
    na3::Isometry3::from_parts(
        na3::Translation3::new(
            transform.position.x,
            transform.position.y,
            transform.position.z,
        ),
        rotation,
    )
}

fn joint_signature_2d(
    body_a: NodeID,
    body_b: NodeID,
    anchor_a: Vector2,
    anchor_b: Vector2,
    enabled: bool,
    collide_connected: bool,
    kind: JointKind2D,
) -> u64 {
    let mut hash = body_a.as_u64().wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ body_b.as_u64();
    hash = hash_u32(hash, anchor_a.x.to_bits());
    hash = hash_u32(hash, anchor_a.y.to_bits());
    hash = hash_u32(hash, anchor_b.x.to_bits());
    hash = hash_u32(hash, anchor_b.y.to_bits());
    hash = hash_u32(hash, enabled as u32);
    hash = hash_u32(hash, collide_connected as u32);
    match kind {
        JointKind2D::Pin => hash_u32(hash, 1),
        JointKind2D::Distance { min, max } => {
            let hash = hash_u32(hash, 2);
            let hash = hash_u32(hash, min.to_bits());
            hash_u32(hash, max.to_bits())
        }
        JointKind2D::Fixed => hash_u32(hash, 3),
    }
}

fn joint_signature_3d(
    body_a: NodeID,
    body_b: NodeID,
    anchor_a: Vector3,
    anchor_b: Vector3,
    enabled: bool,
    collide_connected: bool,
    kind: JointKind3D,
) -> u64 {
    let mut hash = body_a.as_u64().wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ body_b.as_u64();
    hash = hash_u32(hash, anchor_a.x.to_bits());
    hash = hash_u32(hash, anchor_a.y.to_bits());
    hash = hash_u32(hash, anchor_a.z.to_bits());
    hash = hash_u32(hash, anchor_b.x.to_bits());
    hash = hash_u32(hash, anchor_b.y.to_bits());
    hash = hash_u32(hash, anchor_b.z.to_bits());
    hash = hash_u32(hash, enabled as u32);
    hash = hash_u32(hash, collide_connected as u32);
    match kind {
        JointKind3D::Ball => hash_u32(hash, 1),
        JointKind3D::Hinge { axis } => {
            let hash = hash_u32(hash, 2);
            let hash = hash_u32(hash, axis.x.to_bits());
            let hash = hash_u32(hash, axis.y.to_bits());
            hash_u32(hash, axis.z.to_bits())
        }
        JointKind3D::Fixed => hash_u32(hash, 3),
    }
}

fn build_joint_2d(desc: &JointDesc2D) -> r2::GenericJoint {
    let anchor_a = na2::Point2::new(desc.anchor_a.x, desc.anchor_a.y);
    let anchor_b = na2::Point2::new(desc.anchor_b.x, desc.anchor_b.y);
    match desc.kind {
        JointKind2D::Pin => r2::RevoluteJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
        JointKind2D::Distance { min, max } => {
            let min = min.max(0.0);
            let max = max.max(min).max(0.0001);
            r2::GenericJointBuilder::new(r2::JointAxesMask::empty())
                .coupled_axes(r2::JointAxesMask::LIN_AXES)
                .limits(r2::JointAxis::LinX, [min, max])
                .contacts_enabled(desc.collide_connected)
                .local_anchor1(anchor_a)
                .local_anchor2(anchor_b)
                .into()
        }
        JointKind2D::Fixed => r2::FixedJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
    }
}

fn build_joint_3d(desc: &JointDesc3D) -> r3::GenericJoint {
    let anchor_a = na3::Point3::new(desc.anchor_a.x, desc.anchor_a.y, desc.anchor_a.z);
    let anchor_b = na3::Point3::new(desc.anchor_b.x, desc.anchor_b.y, desc.anchor_b.z);
    match desc.kind {
        JointKind3D::Ball => r3::SphericalJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
        JointKind3D::Hinge { axis } => {
            let axis = if axis.x * axis.x + axis.y * axis.y + axis.z * axis.z <= 0.000_001 {
                na3::Vector3::y_axis()
            } else {
                na3::Unit::new_normalize(na3::Vector3::new(axis.x, axis.y, axis.z))
            };
            r3::RevoluteJointBuilder::new(axis)
                .contacts_enabled(desc.collide_connected)
                .local_anchor1(anchor_a)
                .local_anchor2(anchor_b)
                .into()
        }
        JointKind3D::Fixed => r3::FixedJointBuilder::new()
            .contacts_enabled(desc.collide_connected)
            .local_anchor1(anchor_a)
            .local_anchor2(anchor_b)
            .into(),
    }
}

fn remove_joint_2d(world: &mut PhysicsWorld2D, id: NodeID) {
    if let Some(state) = world.joint_map.remove(&id) {
        let _ = world.impulse_joints.remove(state.handle, true);
    }
}

fn remove_joint_3d(world: &mut PhysicsWorld3D, id: NodeID) {
    if let Some(state) = world.joint_map.remove(&id) {
        let _ = world.impulse_joints.remove(state.handle, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::render_2d::{
        ParsedTile2D, ParsedTileCollisionShape2D, ParsedTileset2D, TileSetShape2D,
    };
    use perro_nodes::{
        Area2D, Area3D, CollisionShape2D, CollisionShape3D, FixedJoint2D, FixedJoint3D,
        RigidBody2D, RigidBody3D, StaticBody2D, StaticBody3D,
    };

    #[test]
    fn physics_raycast_3d_hits_static_body() {
        let mut runtime = Runtime::new();
        let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));

        let hit = runtime
            .physics_raycast_3d(
                Vector3::new(0.0, 0.0, -5.0),
                Vector3::new(0.0, 0.0, 1.0),
                10.0,
                false,
            )
            .expect("ray should hit cube");

        assert_eq!(hit.node, body);
        assert!((hit.distance - 4.5).abs() < 0.001);
        assert!((hit.point.z + 0.5).abs() < 0.001);
        assert!(hit.normal.z < -0.9);
    }

    #[test]
    fn physics_raycast_3d_hits_area_with_collision_shape() {
        let mut runtime = Runtime::new();

        let static_body = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let static_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

        let area = NodeAPI::create::<Area3D>(&mut runtime);
        let area_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, area, area_shape));
        let _ = <Runtime as NodeAPI>::set_global_transform_3d(
            &mut runtime,
            area,
            Transform3D::new(
                Vector3::new(0.0, 0.0, -2.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
        );

        let area_hit = runtime
            .physics_raycast_3d(
                Vector3::new(0.0, 0.0, -5.0),
                Vector3::new(0.0, 0.0, 1.0),
                10.0,
                true,
            )
            .expect("ray should hit area first");
        assert_eq!(area_hit.node, area);
        assert!((area_hit.distance - 2.5).abs() < 0.001);

        let no_area_hit = runtime
            .physics_raycast_3d(
                Vector3::new(0.0, 0.0, -5.0),
                Vector3::new(0.0, 0.0, 1.0),
                10.0,
                false,
            )
            .expect("ray should skip area and hit static body");
        assert_eq!(no_area_hit.node, static_body);
    }

    #[test]
    fn physics_raycast_2d_filters_areas_and_nodes() {
        let mut runtime = Runtime::new();

        let static_body = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let static_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

        let area = NodeAPI::create::<Area2D>(&mut runtime);
        let area_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, area, area_shape));
        let _ = <Runtime as NodeAPI>::set_global_transform_2d(
            &mut runtime,
            area,
            Transform2D::new(Vector2::new(-2.0, 0.0), 0.0, Vector2::ONE),
        );

        let hit = runtime
            .physics_raycast_2d(
                Vector2::new(-5.0, 0.0),
                Vector2::new(1.0, 0.0),
                10.0,
                &PhysicsQueryFilter::default(),
            )
            .expect("ray should hit area first");
        assert_eq!(hit.node, area);

        let hit = runtime
            .physics_raycast_2d(
                Vector2::new(-5.0, 0.0),
                Vector2::new(1.0, 0.0),
                10.0,
                &PhysicsQueryFilter {
                    include_areas: false,
                    ..PhysicsQueryFilter::default()
                },
            )
            .expect("ray should skip area");
        assert_eq!(hit.node, static_body);

        let hit = runtime.physics_raycast_2d(
            Vector2::new(-5.0, 0.0),
            Vector2::new(1.0, 0.0),
            10.0,
            &PhysicsQueryFilter {
                include_areas: false,
                exclude_nodes: vec![static_body],
                ..PhysicsQueryFilter::default()
            },
        );
        assert!(hit.is_none());

        if let Some(node) = runtime.nodes.get_mut(static_body)
            && let SceneNodeData::StaticBody2D(body) = &mut node.data
        {
            body.collision_layer = 4;
            body.collision_mask = 0;
        }
        let hit = runtime
            .physics_raycast_2d(
                Vector2::new(-5.0, 0.0),
                Vector2::new(1.0, 0.0),
                10.0,
                &PhysicsQueryFilter {
                    mask: 4,
                    include_areas: false,
                    exclude_nodes: Vec::new(),
                },
            )
            .expect("query mask should use collider layer without collider mask coupling");
        assert_eq!(hit.node, static_body);
    }

    #[test]
    fn physics_shape_cast_2d_and_3d_hit_static_bodies() {
        let mut runtime = Runtime::new();

        let body_2d = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let shape_2d = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body_2d, shape_2d));
        let hit_2d = runtime
            .physics_shape_cast_2d(
                Shape2D::Circle { radius: 0.25 },
                Vector2::new(-5.0, 0.0),
                Vector2::new(1.0, 0.0),
                10.0,
                &PhysicsQueryFilter::default(),
            )
            .expect("2d shape cast should hit");
        assert_eq!(hit_2d.node, body_2d);
        assert!(hit_2d.distance > 3.0 && hit_2d.distance < 5.0);

        let body_3d = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let shape_3d = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body_3d, shape_3d));
        let _ = <Runtime as NodeAPI>::set_global_transform_3d(
            &mut runtime,
            body_3d,
            Transform3D::new(
                Vector3::new(0.0, 0.0, 4.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
        );
        let hit_3d = runtime
            .physics_shape_cast_3d(
                Shape3D::Sphere { radius: 0.25 },
                Vector3::new(0.0, 0.0, -5.0),
                Vector3::new(0.0, 0.0, 1.0),
                20.0,
                &PhysicsQueryFilter::default(),
            )
            .expect("3d shape cast should hit");
        assert_eq!(hit_3d.node, body_3d);
    }

    #[test]
    fn physics_contacts_return_other_node_and_points() {
        let mut runtime = Runtime::new();

        let body_a = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let shape_a = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body_a, shape_a));
        let body_b = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let shape_b = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body_b, shape_b));
        if let Some(node) = runtime.nodes.get_mut(body_a)
            && let SceneNodeData::RigidBody2D(body) = &mut node.data
        {
            body.gravity_scale = 0.0;
        }

        runtime.physics_fixed_step();
        let contacts = runtime.physics_contacts_2d(body_a);
        assert!(contacts.iter().any(|contact| contact.node == body_b));
    }

    #[test]
    fn tilemap_explicit_collision_shapes_do_not_merge_with_auto() {
        let tilemap = TileMap2D {
            width: 2,
            height: 1,
            tiles: vec![1, 2],
            collision_enabled: true,
            ..TileMap2D::new()
        };
        let tiles = vec![
            ParsedTile2D {
                id: 1,
                atlas: [0, 0],
                collision: true,
                collision_shape: ParsedTileCollisionShape2D::Auto,
            },
            ParsedTile2D {
                id: 2,
                atlas: [1, 0],
                collision: true,
                collision_shape: ParsedTileCollisionShape2D::Shape {
                    shape: TileSetShape2D::Circle { radius: 3.0 },
                    offset: [1.0, -1.0],
                },
            },
        ];
        let tileset = ParsedTileset2D {
            texture: "res://tiles.png".into(),
            tile_size: [16.0, 16.0],
            columns: 2,
            rows: 1,
            tiles: tiles.into(),
        };

        let shapes = tilemap_shape_descs_2d(&tilemap, 1, u32::MAX, 0.7, 0.0, Some(&tileset));
        assert_eq!(shapes.len(), 2);
        assert!(matches!(
            shapes[0].shape,
            ShapeKind2D::Primitive(Shape2D::Quad { .. })
        ));
        assert!(matches!(
            shapes[1].shape,
            ShapeKind2D::Primitive(Shape2D::Circle { radius }) if radius == 3.0
        ));
        assert_eq!(shapes[1].local.position, Vector2::new(25.0, -7.0));
    }

    #[test]
    #[ignore]
    fn bench_tilemap_collision_bake_128x128_auto_merge() {
        let tile_count = 128 * 128;
        let tilemap = TileMap2D {
            width: 128,
            height: 128,
            tiles: vec![1; tile_count],
            collision_enabled: true,
            ..TileMap2D::new()
        };
        let tiles = vec![ParsedTile2D {
            id: 1,
            atlas: [0, 0],
            collision: true,
            collision_shape: ParsedTileCollisionShape2D::Auto,
        }];
        let tileset = ParsedTileset2D {
            texture: "res://tiles.png".into(),
            tile_size: [16.0, 16.0],
            columns: 1,
            rows: 1,
            tiles: tiles.into(),
        };

        let start = std::time::Instant::now();
        let mut total = 0usize;
        for _ in 0..250 {
            total += tilemap_shape_descs_2d(&tilemap, 1, u32::MAX, 0.7, 0.0, Some(&tileset)).len();
        }
        let elapsed = start.elapsed();
        assert_eq!(total, 250);
        eprintln!("bench_tilemap_collision_bake_128x128_auto_merge: {elapsed:?}");
    }

    #[test]
    #[ignore]
    fn bench_physics_raycast_2d_query_filter() {
        let mut runtime = Runtime::new();
        for i in 0..256 {
            let body = NodeAPI::create::<StaticBody2D>(&mut runtime);
            let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, shape));
            let _ = <Runtime as NodeAPI>::set_global_transform_2d(
                &mut runtime,
                body,
                Transform2D::new(Vector2::new(i as f32 * 2.0, 0.0), 0.0, Vector2::ONE),
            );
        }

        let filter = PhysicsQueryFilter::default();
        let start = std::time::Instant::now();
        let mut hits = 0usize;
        for _ in 0..10_000 {
            if runtime
                .physics_raycast_2d(
                    Vector2::new(-10.0, 0.0),
                    Vector2::new(1.0, 0.0),
                    1_000.0,
                    &filter,
                )
                .is_some()
            {
                hits += 1;
            }
        }
        let elapsed = start.elapsed();
        assert_eq!(hits, 10_000);
        eprintln!("bench_physics_raycast_2d_query_filter: {elapsed:?}");
    }

    #[test]
    fn physics_2d_layers_and_masks_filter_area_overlaps() {
        let mut runtime = Runtime::new();

        let static_body = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let static_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

        let area = NodeAPI::create::<Area2D>(&mut runtime);
        let area_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, area, area_shape));

        if let Some(node) = runtime.nodes.get_mut(static_body)
            && let SceneNodeData::RigidBody2D(body) = &mut node.data
        {
            body.collision_layer = 1;
            body.collision_mask = 1;
            body.gravity_scale = 0.0;
        }
        if let Some(node) = runtime.nodes.get_mut(area)
            && let SceneNodeData::Area2D(body) = &mut node.data
        {
            body.collision_layer = 2;
            body.collision_mask = 2;
        }

        runtime.physics_fixed_step();
        assert!(runtime.physics.active_area_overlaps_2d.is_empty());

        if let Some(node) = runtime.nodes.get_mut(area)
            && let SceneNodeData::Area2D(body) = &mut node.data
        {
            body.collision_mask = 1;
        }
        if let Some(node) = runtime.nodes.get_mut(static_body)
            && let SceneNodeData::RigidBody2D(body) = &mut node.data
        {
            body.collision_mask = 2;
        }

        runtime.physics_fixed_step();
        assert!(
            runtime
                .physics
                .active_area_overlaps_2d
                .contains(&AreaOverlap {
                    area,
                    other: static_body
                })
        );
    }

    #[test]
    fn physics_3d_layers_and_masks_filter_area_overlaps() {
        let mut runtime = Runtime::new();

        let static_body = NodeAPI::create::<RigidBody3D>(&mut runtime);
        let static_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

        let area = NodeAPI::create::<Area3D>(&mut runtime);
        let area_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, area, area_shape));

        if let Some(node) = runtime.nodes.get_mut(static_body)
            && let SceneNodeData::RigidBody3D(body) = &mut node.data
        {
            body.collision_layer = 1;
            body.collision_mask = 1;
            body.gravity_scale = 0.0;
        }
        if let Some(node) = runtime.nodes.get_mut(area)
            && let SceneNodeData::Area3D(body) = &mut node.data
        {
            body.collision_layer = 4;
            body.collision_mask = 4;
        }

        runtime.physics_fixed_step();
        assert!(runtime.physics.active_area_overlaps_3d.is_empty());

        if let Some(node) = runtime.nodes.get_mut(area)
            && let SceneNodeData::Area3D(body) = &mut node.data
        {
            body.collision_mask = 1;
        }
        if let Some(node) = runtime.nodes.get_mut(static_body)
            && let SceneNodeData::RigidBody3D(body) = &mut node.data
        {
            body.collision_mask = 4;
        }

        runtime.physics_fixed_step();
        assert!(
            runtime
                .physics
                .active_area_overlaps_3d
                .contains(&AreaOverlap {
                    area,
                    other: static_body
                })
        );
    }

    #[test]
    fn physics_2d_fixed_joint_syncs_and_disables() {
        let mut runtime = Runtime::new();

        let body_a = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let body_b = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let joint = NodeAPI::create::<FixedJoint2D>(&mut runtime);

        if let Some(node) = runtime.nodes.get_mut(body_a)
            && let SceneNodeData::RigidBody2D(body) = &mut node.data
        {
            body.gravity_scale = 0.0;
        }
        if let Some(node) = runtime.nodes.get_mut(body_b)
            && let SceneNodeData::RigidBody2D(body) = &mut node.data
        {
            body.gravity_scale = 0.0;
        }
        if let Some(node) = runtime.nodes.get_mut(joint)
            && let SceneNodeData::FixedJoint2D(joint_data) = &mut node.data
        {
            joint_data.body_a = body_a;
            joint_data.body_b = body_b;
        }

        runtime.physics_fixed_step();
        assert!(
            runtime
                .physics
                .world_2d
                .as_ref()
                .is_some_and(|world| world.joint_map.contains_key(&joint))
        );

        if let Some(node) = runtime.nodes.get_mut(joint)
            && let SceneNodeData::FixedJoint2D(joint_data) = &mut node.data
        {
            joint_data.enabled = false;
        }

        runtime.physics_fixed_step();
        assert!(
            runtime
                .physics
                .world_2d
                .as_ref()
                .is_none_or(|world| !world.joint_map.contains_key(&joint))
        );
    }

    #[test]
    fn physics_2d_distance_joint_enforces_min_and_max_limits() {
        let joint = JointDesc2D {
            id: NodeID::new(1),
            body_a: NodeID::new(2),
            body_b: NodeID::new(3),
            anchor_a: Vector2::new(-1.0, 0.0),
            anchor_b: Vector2::new(1.0, 0.0),
            enabled: true,
            collide_connected: false,
            kind: JointKind2D::Distance { min: 2.0, max: 5.0 },
            signature: 0,
        };

        let data = build_joint_2d(&joint);
        let limits = data
            .limits(r2::JointAxis::LinX)
            .expect("distance joint should set linear limits");

        assert_eq!(limits.min, 2.0);
        assert_eq!(limits.max, 5.0);
        assert_eq!(data.coupled_axes, r2::JointAxesMask::LIN_AXES);
    }

    #[test]
    fn physics_3d_fixed_joint_syncs_and_disables() {
        let mut runtime = Runtime::new();

        let body_a = NodeAPI::create::<RigidBody3D>(&mut runtime);
        let body_b = NodeAPI::create::<RigidBody3D>(&mut runtime);
        let joint = NodeAPI::create::<FixedJoint3D>(&mut runtime);

        if let Some(node) = runtime.nodes.get_mut(body_a)
            && let SceneNodeData::RigidBody3D(body) = &mut node.data
        {
            body.gravity_scale = 0.0;
        }
        if let Some(node) = runtime.nodes.get_mut(body_b)
            && let SceneNodeData::RigidBody3D(body) = &mut node.data
        {
            body.gravity_scale = 0.0;
        }
        if let Some(node) = runtime.nodes.get_mut(joint)
            && let SceneNodeData::FixedJoint3D(joint_data) = &mut node.data
        {
            joint_data.body_a = body_a;
            joint_data.body_b = body_b;
        }

        runtime.physics_fixed_step();
        assert!(
            runtime
                .physics
                .world_3d
                .as_ref()
                .is_some_and(|world| world.joint_map.contains_key(&joint))
        );

        if let Some(node) = runtime.nodes.get_mut(joint)
            && let SceneNodeData::FixedJoint3D(joint_data) = &mut node.data
        {
            joint_data.enabled = false;
        }

        runtime.physics_fixed_step();
        assert!(
            runtime
                .physics
                .world_3d
                .as_ref()
                .is_none_or(|world| !world.joint_map.contains_key(&joint))
        );
    }
}
