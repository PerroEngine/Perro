use crate::Runtime;
use ahash::{AHashMap, AHashSet};
use perro_ids::{NodeID, SignalID};
use perro_nodes::{
    CollisionShape2D, CollisionShape3D, RigidBody2D, RigidBody3D, SceneNodeData, Shape2D, Shape3D,
    Triangle2DKind,
};
use perro_runtime_context::sub_apis::{NodeAPI, SignalAPI};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;
use rapier2d::{na as na2, prelude as r2};
use rapier3d::{na as na3, prelude as r3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyKind {
    Static,
    Area,
    Rigid,
}

#[derive(Clone, Debug)]
struct ShapeDesc2D {
    local: Transform2D,
    shape: Shape2D,
    sensor: bool,
    friction: f32,
    restitution: f32,
    density: f32,
}

#[derive(Clone, Debug)]
struct ShapeDesc3D {
    local: Transform3D,
    shape: Shape3D,
    sensor: bool,
    friction: f32,
    restitution: f32,
    density: f32,
}

#[derive(Clone, Debug)]
struct BodyDesc2D {
    id: NodeID,
    kind: BodyKind,
    enabled: bool,
    global: Transform2D,
    rigid: Option<RigidBody2D>,
    shapes: Vec<ShapeDesc2D>,
}

#[derive(Clone, Debug)]
struct BodyDesc3D {
    id: NodeID,
    kind: BodyKind,
    enabled: bool,
    global: Transform3D,
    rigid: Option<RigidBody3D>,
    shapes: Vec<ShapeDesc3D>,
}

#[derive(Clone, Debug)]
struct BodyState2D {
    handle: r2::RigidBodyHandle,
    colliders: Vec<r2::ColliderHandle>,
    kind: BodyKind,
    opaque_handle: u64,
}

#[derive(Clone, Debug)]
struct BodyState3D {
    handle: r3::RigidBodyHandle,
    colliders: Vec<r3::ColliderHandle>,
    kind: BodyKind,
    opaque_handle: u64,
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
    direction: Vector2,
    amount: f32,
}

#[derive(Clone, Copy, Debug)]
struct PendingImpulse3D {
    id: NodeID,
    direction: Vector3,
    amount: f32,
}

#[derive(Clone, Copy, Debug)]
struct PendingForce2D {
    id: NodeID,
    direction: Vector2,
    amount: f32,
}

#[derive(Clone, Copy, Debug)]
struct PendingForce3D {
    id: NodeID,
    direction: Vector3,
    amount: f32,
}

pub(crate) struct PhysicsState {
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
}

impl PhysicsState {
    pub(crate) fn new() -> Self {
        Self {
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
        Self {
            pipeline: r2::PhysicsPipeline::new(),
            gravity: na2::Vector2::new(0.0, -9.81),
            integration_parameters: r2::IntegrationParameters::default(),
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
        }
    }
}

impl PhysicsWorld3D {
    fn new() -> Self {
        Self {
            pipeline: r3::PhysicsPipeline::new(),
            gravity: na3::Vector3::new(0.0, -9.81, 0.0),
            integration_parameters: r3::IntegrationParameters::default(),
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
        }
    }
}

impl Runtime {
    pub(crate) fn physics_fixed_step(&mut self) {
        let bodies_2d = self.collect_body_descs_2d();
        let bodies_3d = self.collect_body_descs_3d();
        self.sync_world_2d(&bodies_2d);
        self.sync_world_3d(&bodies_3d);
        self.apply_pending_forces_2d();
        self.apply_pending_forces_3d();
        self.apply_pending_impulses_2d();
        self.apply_pending_impulses_3d();
        self.step_world_2d();
        self.step_world_3d();
        self.sync_world_to_nodes_2d();
        self.sync_world_to_nodes_3d();
        self.emit_collision_signals_2d();
        self.emit_collision_signals_3d();
        self.emit_area_signals_2d();
        self.emit_area_signals_3d();
    }

    pub(crate) fn queue_impulse_2d(&mut self, id: NodeID, direction: Vector2, amount: f32) {
        self.physics.pending_impulses_2d.push(PendingImpulse2D {
            id,
            direction,
            amount,
        });
    }

    pub(crate) fn queue_force_2d(&mut self, id: NodeID, direction: Vector2, amount: f32) {
        self.physics.pending_forces_2d.push(PendingForce2D {
            id,
            direction,
            amount,
        });
    }

    pub(crate) fn queue_impulse_3d(&mut self, id: NodeID, direction: Vector3, amount: f32) {
        self.physics.pending_impulses_3d.push(PendingImpulse3D {
            id,
            direction,
            amount,
        });
    }

    pub(crate) fn queue_force_3d(&mut self, id: NodeID, direction: Vector3, amount: f32) {
        self.physics.pending_forces_3d.push(PendingForce3D {
            id,
            direction,
            amount,
        });
    }

    pub(crate) fn clear_physics(&mut self) {
        self.physics.clear();
    }

    fn collect_body_descs_2d(&mut self) -> Vec<BodyDesc2D> {
        let ids = self.internal_updates.physics_body_nodes_2d.clone();

        let mut out = Vec::new();
        for id in ids {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };

            let (kind, enabled, rigid, children) = match &node.data {
                SceneNodeData::StaticBody2D(body) => (
                    BodyKind::Static,
                    body.enabled,
                    None,
                    node.children_slice().to_vec(),
                ),
                SceneNodeData::Area2D(body) => (
                    BodyKind::Area,
                    body.enabled,
                    None,
                    node.children_slice().to_vec(),
                ),
                SceneNodeData::RigidBody2D(body) => (
                    BodyKind::Rigid,
                    body.enabled,
                    Some(body.clone()),
                    node.children_slice().to_vec(),
                ),
                _ => continue,
            };

            let Some(global) = self.get_global_transform_2d(id) else {
                continue;
            };
            let mut shapes = Vec::new();
            for child_id in children {
                let Some(child) = self.nodes.get(child_id) else {
                    continue;
                };
                if let SceneNodeData::CollisionShape2D(shape) = &child.data {
                    let mut desc = shape_desc_2d(shape);
                    if kind == BodyKind::Area {
                        desc.sensor = true;
                    }
                    shapes.push(desc);
                }
            }

            out.push(BodyDesc2D {
                id,
                kind,
                enabled,
                global,
                rigid,
                shapes,
            });
        }
        out
    }

    fn collect_body_descs_3d(&mut self) -> Vec<BodyDesc3D> {
        let ids = self.internal_updates.physics_body_nodes_3d.clone();

        let mut out = Vec::new();
        for id in ids {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };

            let (kind, enabled, rigid, children) = match &node.data {
                SceneNodeData::StaticBody3D(body) => (
                    BodyKind::Static,
                    body.enabled,
                    None,
                    node.children_slice().to_vec(),
                ),
                SceneNodeData::Area3D(body) => (
                    BodyKind::Area,
                    body.enabled,
                    None,
                    node.children_slice().to_vec(),
                ),
                SceneNodeData::RigidBody3D(body) => (
                    BodyKind::Rigid,
                    body.enabled,
                    Some(body.clone()),
                    node.children_slice().to_vec(),
                ),
                _ => continue,
            };

            let Some(global) = self.get_global_transform_3d(id) else {
                continue;
            };
            let mut shapes = Vec::new();
            for child_id in children {
                let Some(child) = self.nodes.get(child_id) else {
                    continue;
                };
                if let SceneNodeData::CollisionShape3D(shape) = &child.data {
                    let mut desc = shape_desc_3d(shape);
                    if kind == BodyKind::Area {
                        desc.sensor = true;
                    }
                    shapes.push(desc);
                }
            }

            out.push(BodyDesc3D {
                id,
                kind,
                enabled,
                global,
                rigid,
                shapes,
            });
        }
        out
    }

    fn sync_world_2d(&mut self, bodies: &[BodyDesc2D]) {
        if bodies.is_empty() {
            if let Some(world) = self.physics.world_2d.take() {
                let ids: Vec<NodeID> = world.body_map.keys().copied().collect();
                for id in ids {
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
                rb.set_body_type(
                    match body.kind {
                        BodyKind::Static => r2::RigidBodyType::Fixed,
                        BodyKind::Area => r2::RigidBodyType::Fixed,
                        BodyKind::Rigid => r2::RigidBodyType::Dynamic,
                    },
                    true,
                );
                rb.set_position(transform_to_iso2(body.global), true);

                if let Some(rigid) = body.rigid.as_ref() {
                    rb.set_linvel(
                        na2::Vector2::new(rigid.linear_velocity.x, rigid.linear_velocity.y),
                        true,
                    );
                    rb.set_angvel(rigid.angular_velocity, true);
                    rb.set_gravity_scale(rigid.gravity_scale, true);
                    rb.set_linear_damping(rigid.linear_damping);
                    rb.set_angular_damping(rigid.angular_damping);
                }
            }

            for handle in state.colliders.drain(..) {
                let _ = world.colliders.remove(
                    handle,
                    &mut world.islands,
                    &mut world.bodies,
                    true,
                );
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
        }

        let stale: Vec<NodeID> = world
            .body_map
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();

        for id in stale {
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
        self.physics.world_2d = Some(world);
    }

    fn sync_world_3d(&mut self, bodies: &[BodyDesc3D]) {
        if bodies.is_empty() {
            if let Some(world) = self.physics.world_3d.take() {
                let ids: Vec<NodeID> = world.body_map.keys().copied().collect();
                for id in ids {
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
                rb.set_body_type(
                    match body.kind {
                        BodyKind::Static => r3::RigidBodyType::Fixed,
                        BodyKind::Area => r3::RigidBodyType::Fixed,
                        BodyKind::Rigid => r3::RigidBodyType::Dynamic,
                    },
                    true,
                );
                rb.set_position(transform_to_iso3(body.global), true);

                if let Some(rigid) = body.rigid.as_ref() {
                    rb.set_linvel(
                        na3::Vector3::new(
                            rigid.linear_velocity.x,
                            rigid.linear_velocity.y,
                            rigid.linear_velocity.z,
                        ),
                        true,
                    );
                    rb.set_angvel(
                        na3::Vector3::new(
                            rigid.angular_velocity.x,
                            rigid.angular_velocity.y,
                            rigid.angular_velocity.z,
                        ),
                        true,
                    );
                    rb.set_gravity_scale(rigid.gravity_scale, true);
                    rb.set_linear_damping(rigid.linear_damping);
                    rb.set_angular_damping(rigid.angular_damping);
                }
            }

            for handle in state.colliders.drain(..) {
                let _ = world.colliders.remove(
                    handle,
                    &mut world.islands,
                    &mut world.bodies,
                    true,
                );
            }

            for shape in &body.shapes {
                let Some(builder) = collider_builder_3d(shape) else {
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
        }

        let stale: Vec<NodeID> = world
            .body_map
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();

        for id in stale {
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
        self.physics.world_3d = Some(world);
    }

    fn step_world_2d(&mut self) {
        let Some(world) = self.physics.world_2d.as_mut() else {
            return;
        };
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
        let Some(world) = self.physics.world_3d.as_mut() else {
            return;
        };
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
            let len_sq = impulse.direction.x * impulse.direction.x
                + impulse.direction.y * impulse.direction.y;
            if len_sq <= 0.000_001 {
                continue;
            }
            let inv_len = len_sq.sqrt().recip();
            rb.apply_impulse(
                na2::Vector2::new(
                    impulse.direction.x * inv_len * impulse.amount,
                    impulse.direction.y * inv_len * impulse.amount,
                ),
                true,
            );
        }
        self.physics.pending_impulses_2d = pending;
    }

    fn apply_pending_forces_2d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_forces_2d);
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
            let len_sq =
                force.direction.x * force.direction.x + force.direction.y * force.direction.y;
            if len_sq <= 0.000_001 {
                continue;
            }
            let inv_len = len_sq.sqrt().recip();
            let impulse = force.amount * dt;
            rb.apply_impulse(
                na2::Vector2::new(
                    force.direction.x * inv_len * impulse,
                    force.direction.y * inv_len * impulse,
                ),
                true,
            );
        }
        self.physics.pending_forces_2d = pending;
    }

    fn apply_pending_impulses_3d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_impulses_3d);
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
            let len_sq = impulse.direction.x * impulse.direction.x
                + impulse.direction.y * impulse.direction.y
                + impulse.direction.z * impulse.direction.z;
            if len_sq <= 0.000_001 {
                continue;
            }
            let inv_len = len_sq.sqrt().recip();
            rb.apply_impulse(
                na3::Vector3::new(
                    impulse.direction.x * inv_len * impulse.amount,
                    impulse.direction.y * inv_len * impulse.amount,
                    impulse.direction.z * inv_len * impulse.amount,
                ),
                true,
            );
        }
        self.physics.pending_impulses_3d = pending;
    }

    fn apply_pending_forces_3d(&mut self) {
        let mut pending = std::mem::take(&mut self.physics.pending_forces_3d);
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
            let len_sq = force.direction.x * force.direction.x
                + force.direction.y * force.direction.y
                + force.direction.z * force.direction.z;
            if len_sq <= 0.000_001 {
                continue;
            }
            let inv_len = len_sq.sqrt().recip();
            let impulse = force.amount * dt;
            rb.apply_impulse(
                na3::Vector3::new(
                    force.direction.x * inv_len * impulse,
                    force.direction.y * inv_len * impulse,
                    force.direction.z * inv_len * impulse,
                ),
                true,
            );
        }
        self.physics.pending_forces_3d = pending;
    }

    fn sync_world_to_nodes_2d(&mut self) {
        let Some(world) = self.physics.world_2d.as_ref() else {
            return;
        };
        let ids: Vec<NodeID> = world.body_map.keys().copied().collect();
        for id in ids {
            let Some(state) = self
                .physics
                .world_2d
                .as_ref()
                .and_then(|w| w.body_map.get(&id))
                .cloned()
            else {
                continue;
            };
            self.set_body_handle_2d(id, Some(state.opaque_handle));
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(body) = self
                .physics
                .world_2d
                .as_ref()
                .and_then(|w| w.bodies.get(state.handle))
            else {
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
    }

    fn sync_world_to_nodes_3d(&mut self) {
        let Some(world) = self.physics.world_3d.as_ref() else {
            return;
        };
        let ids: Vec<NodeID> = world.body_map.keys().copied().collect();
        for id in ids {
            let Some(state) = self
                .physics
                .world_3d
                .as_ref()
                .and_then(|w| w.body_map.get(&id))
                .cloned()
            else {
                continue;
            };
            self.set_body_handle_3d(id, Some(state.opaque_handle));
            if state.kind != BodyKind::Rigid {
                continue;
            }
            let Some(body) = self
                .physics
                .world_3d
                .as_ref()
                .and_then(|w| w.bodies.get(state.handle))
            else {
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

fn shape_desc_2d(shape: &CollisionShape2D) -> ShapeDesc2D {
    ShapeDesc2D {
        local: shape.base.transform,
        shape: shape.shape,
        sensor: shape.sensor,
        friction: shape.friction,
        restitution: shape.restitution,
        density: shape.density,
    }
}

fn shape_desc_3d(shape: &CollisionShape3D) -> ShapeDesc3D {
    ShapeDesc3D {
        local: shape.base.transform,
        shape: shape.shape,
        sensor: shape.sensor,
        friction: shape.friction,
        restitution: shape.restitution,
        density: shape.density,
    }
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
            .can_sleep(rigid.can_sleep)
            .enabled(rigid.enabled);
    }

    builder.build()
}

fn collider_builder_2d(desc: &ShapeDesc2D) -> Option<r2::Collider> {
    let sx = desc.local.scale.x.abs().max(0.0001);
    let sy = desc.local.scale.y.abs().max(0.0001);
    let shape = match desc.shape {
        Shape2D::Quad { width, height } => r2::ColliderBuilder::cuboid(
            width.abs().max(0.0001) * sx * 0.5,
            height.abs().max(0.0001) * sy * 0.5,
        ),
        Shape2D::Circle { radius } => {
            let scale = sx.max(sy);
            r2::ColliderBuilder::ball(radius.abs().max(0.0001) * scale)
        }
        Shape2D::Triangle {
            kind,
            width,
            height,
        } => {
            let points = triangle_points_2d(kind, width * sx, height * sy)?;
            r2::ColliderBuilder::triangle(points[0], points[1], points[2])
        }
    };

    Some(
        shape
            .position(na2::Isometry2::new(
                na2::Vector2::new(desc.local.position.x, desc.local.position.y),
                desc.local.rotation,
            ))
            .sensor(desc.sensor)
            .friction(desc.friction)
            .restitution(desc.restitution)
            .density(desc.density)
            .build(),
    )
}

fn collider_builder_3d(desc: &ShapeDesc3D) -> Option<r3::Collider> {
    let sx = desc.local.scale.x.abs().max(0.0001);
    let sy = desc.local.scale.y.abs().max(0.0001);
    let sz = desc.local.scale.z.abs().max(0.0001);

    let shape = match desc.shape {
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
    };

    Some(
        shape
            .position(transform_to_iso3(desc.local))
            .sensor(desc.sensor)
            .friction(desc.friction)
            .restitution(desc.restitution)
            .density(desc.density)
            .build(),
    )
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
