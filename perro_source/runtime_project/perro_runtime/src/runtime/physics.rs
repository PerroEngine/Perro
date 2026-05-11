use super::RuntimePhysicsStepTiming;
use crate::Runtime;
use ahash::{AHashMap, AHashSet};
use perro_asset_formats::pmesh::{
    FLAG_INDEX_U16 as PMESH_FLAG_INDEX_U16, FLAG_PAYLOAD_RAW as PMESH_FLAG_PAYLOAD_RAW,
    VERSION as PMESH_VERSION,
};
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
use rayon::prelude::*;

const MAX_CCD_SUBSTEPS: usize = 1;
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum AudioRaycastInput {
    TwoD {
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        mask: u32,
    },
    ThreeD {
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum AudioRaycastResult {
    #[default]
    None,
    TwoD(Option<PhysicsRayHit2D>),
    ThreeD(Option<PhysicsRayHit3D>),
}

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
    signal_name_scratch: String,
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
    query_pipeline: r2::QueryPipeline,
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
    query_pipeline: r3::QueryPipeline,
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
            signal_name_scratch: String::new(),
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
            query_pipeline: r2::QueryPipeline::new(),
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
            query_pipeline: r3::QueryPipeline::new(),
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

        let world = self.physics.world_3d.as_mut()?;
        world.query_pipeline.update(&world.colliders);

        let ray = r3::Ray::new(na3::Point3::new(origin.x, origin.y, origin.z), dir);
        let filter = if include_areas {
            r3::QueryFilter::new()
        } else {
            r3::QueryFilter::new().exclude_sensors()
        };
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
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

        let world = self.physics.world_2d.as_mut()?;
        world.query_pipeline.update(&world.colliders);

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
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
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

    pub(crate) fn prepare_audio_raycast_2d(&mut self) {
        let bodies_2d = self.collect_body_descs_2d();
        self.sync_world_2d(&bodies_2d);
        if let Some(world) = self.physics.world_2d.as_mut() {
            world.query_pipeline.update(&world.colliders);
        }
    }

    pub(crate) fn prepare_audio_raycast_3d(&mut self) {
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_3d(&bodies_3d);
        if let Some(world) = self.physics.world_3d.as_mut() {
            world.query_pipeline.update(&world.colliders);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn prepared_audio_raycast_2d(
        &self,
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

        let world = self.physics.world_2d.as_ref()?;
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
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
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

    #[allow(dead_code)]
    pub(crate) fn prepared_audio_raycast_3d(
        &self,
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

        let world = self.physics.world_3d.as_ref()?;
        let ray = r3::Ray::new(na3::Point3::new(origin.x, origin.y, origin.z), dir);
        let filter = if include_areas {
            r3::QueryFilter::new()
        } else {
            r3::QueryFilter::new().exclude_sensors()
        };
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
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

    pub(crate) fn cast_prepared_audio_rays(
        &self,
        inputs: &[AudioRaycastInput],
        outputs: &mut [AudioRaycastResult],
        parallel: bool,
    ) {
        let world_2d = self.physics.world_2d.as_ref();
        let world_3d = self.physics.world_3d.as_ref();
        let cast = |input: &AudioRaycastInput| match *input {
            AudioRaycastInput::TwoD {
                origin,
                direction,
                max_distance,
                mask,
            } => AudioRaycastResult::TwoD(world_2d.and_then(|world| {
                prepared_audio_raycast_2d_in_world(world, origin, direction, max_distance, mask)
            })),
            AudioRaycastInput::ThreeD {
                origin,
                direction,
                max_distance,
                include_areas,
            } => AudioRaycastResult::ThreeD(world_3d.and_then(|world| {
                prepared_audio_raycast_3d_in_world(
                    world,
                    origin,
                    direction,
                    max_distance,
                    include_areas,
                )
            })),
        };

        if parallel {
            outputs
                .par_iter_mut()
                .zip(inputs.par_iter())
                .for_each(|(out, input)| *out = cast(input));
        } else {
            for (out, input) in outputs.iter_mut().zip(inputs.iter()) {
                *out = cast(input);
            }
        }
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

        let world = self.physics.world_2d.as_mut()?;
        let shape = shared_shape_2d(shape)?;
        world.query_pipeline.update(&world.colliders);

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
        let (collider, hit) = world.query_pipeline.cast_shape(
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

        let world = self.physics.world_3d.as_mut()?;
        let shape = shared_shape_3d(shape)?;
        world.query_pipeline.update(&world.colliders);

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
        let (collider, hit) = world.query_pipeline.cast_shape(
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
            self.physics.signal_name_scratch.clear();
            self.physics
                .signal_name_scratch
                .push_str(node.name.as_ref());
            self.physics.signal_name_scratch.push_str("_Collided");
            SignalID::from_string(&self.physics.signal_name_scratch)
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
            self.physics.signal_name_scratch.clear();
            self.physics
                .signal_name_scratch
                .push_str(node.name.as_ref());
            self.physics.signal_name_scratch.push('_');
            self.physics.signal_name_scratch.push_str(action);
            SignalID::from_string(&self.physics.signal_name_scratch)
        };

        let params = [Variant::from(area), Variant::from(other)];
        let _ = SignalAPI::signal_emit(self, signal_id, &params);
    }
}

#[path = "physics/helpers.rs"]
mod helpers;

use helpers::*;

#[cfg(test)]
mod tests;
