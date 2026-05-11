use ahash::{AHashMap, AHashSet};
use perro_ids::NodeID;
use perro_nodes::{Shape2D, Shape3D};
use perro_runtime_context::sub_apis::{
    PhysicsContact2D, PhysicsContact3D, PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D,
    PhysicsShapeHit2D, PhysicsShapeHit3D,
};
use perro_structs::{Vector2, Vector3};
use rayon::prelude::*;

use crate::{
    AreaOverlap, AudioRaycastInput, AudioRaycastResult, BodyPair, PendingForce2D, PendingForce3D,
    PendingImpulse2D, PendingImpulse3D, PhysicsAssetContext, PhysicsWorld2D, PhysicsWorld3D,
    TriMeshData, helpers::*, na2, na3, r2, r3,
};

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

pub struct PhysicsSystem {
    pub paused: bool,
    pub world_2d: Option<PhysicsWorld2D>,
    pub world_3d: Option<PhysicsWorld3D>,
    pub active_collision_pairs_2d: AHashSet<BodyPair>,
    pub active_collision_pairs_3d: AHashSet<BodyPair>,
    pub active_area_overlaps_2d: AHashSet<AreaOverlap>,
    pub active_area_overlaps_3d: AHashSet<AreaOverlap>,
    pub pending_forces_2d: Vec<PendingForce2D>,
    pub pending_forces_3d: Vec<PendingForce3D>,
    pub pending_impulses_2d: Vec<PendingImpulse2D>,
    pub pending_impulses_3d: Vec<PendingImpulse3D>,
    pub stale_ids_2d: Vec<NodeID>,
    pub stale_ids_3d: Vec<NodeID>,
    pub trimesh_cache: AHashMap<u64, TriMeshData>,
    pub next_opaque_handle: u64,
    pub signal_name_scratch: String,
}

impl PhysicsSystem {
    pub fn new() -> Self {
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

    pub fn clear(&mut self) {
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

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn queue_impulse_2d(&mut self, id: NodeID, impulse: Vector2) {
        self.pending_impulses_2d
            .push(PendingImpulse2D { id, impulse });
    }

    pub fn queue_force_2d(&mut self, id: NodeID, force: Vector2) {
        self.pending_forces_2d.push(PendingForce2D { id, force });
    }

    pub fn queue_impulse_3d(&mut self, id: NodeID, impulse: Vector3) {
        self.pending_impulses_3d
            .push(PendingImpulse3D { id, impulse });
    }

    pub fn queue_force_3d(&mut self, id: NodeID, force: Vector3) {
        self.pending_forces_3d.push(PendingForce3D { id, force });
    }

    pub fn alloc_opaque_handle(&mut self) -> u64 {
        let handle = self.next_opaque_handle;
        self.next_opaque_handle = self.next_opaque_handle.saturating_add(1);
        handle
    }

    pub fn sync_world_2d(
        &mut self,
        bodies: &[crate::BodyDesc2D],
        mut set_body_handle: impl FnMut(NodeID, Option<u64>),
    ) {
        if bodies.is_empty() {
            if let Some(world) = self.world_2d.take() {
                for id in world.body_map.keys().copied() {
                    set_body_handle(id, None);
                }
            }
            return;
        }

        let mut world = self.world_2d.take().unwrap_or_default();
        let mut alive = AHashSet::default();
        for body in bodies {
            alive.insert(body.id);
            if !world.body_map.contains_key(&body.id) {
                let rb_handle = world.bodies.insert(build_rigid_body_2d(body));
                let opaque = self.alloc_opaque_handle();
                world.body_map.insert(
                    body.id,
                    crate::BodyState2D {
                        handle: rb_handle,
                        colliders: Vec::new(),
                        kind: body.kind,
                        shape_signature: 0,
                        opaque_handle: opaque,
                    },
                );
                set_body_handle(body.id, Some(opaque));
            }

            let Some(state) = world.body_map.get_mut(&body.id) else {
                continue;
            };

            state.kind = body.kind;
            if let Some(rb) = world.bodies.get_mut(state.handle) {
                rb.set_enabled(body.enabled);
                let target_body_type = match body.kind {
                    crate::BodyKind::Static | crate::BodyKind::Area => r2::RigidBodyType::Fixed,
                    crate::BodyKind::Rigid => r2::RigidBodyType::Dynamic,
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
                } else if rb.is_ccd_enabled() {
                    rb.enable_ccd(false);
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

        let mut stale = std::mem::take(&mut self.stale_ids_2d);
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
            set_body_handle(id, None);
        }
        stale.clear();
        self.stale_ids_2d = stale;
        self.world_2d = Some(world);
    }

    pub fn sync_world_3d(
        &mut self,
        bodies: &[crate::BodyDesc3D],
        assets: PhysicsAssetContext,
        mut set_body_handle: impl FnMut(NodeID, Option<u64>),
    ) {
        if bodies.is_empty() {
            if let Some(world) = self.world_3d.take() {
                for id in world.body_map.keys().copied() {
                    set_body_handle(id, None);
                }
            }
            return;
        }

        let mut world = self.world_3d.take().unwrap_or_default();
        let mut alive = AHashSet::default();
        for body in bodies {
            alive.insert(body.id);
            if !world.body_map.contains_key(&body.id) {
                let rb_handle = world.bodies.insert(build_rigid_body_3d(body));
                let opaque = self.alloc_opaque_handle();
                world.body_map.insert(
                    body.id,
                    crate::BodyState3D {
                        handle: rb_handle,
                        colliders: Vec::new(),
                        kind: body.kind,
                        shape_signature: 0,
                        opaque_handle: opaque,
                    },
                );
                set_body_handle(body.id, Some(opaque));
            }

            let Some(state) = world.body_map.get_mut(&body.id) else {
                continue;
            };

            state.kind = body.kind;
            if let Some(rb) = world.bodies.get_mut(state.handle) {
                rb.set_enabled(body.enabled);
                let target_body_type = match body.kind {
                    crate::BodyKind::Static | crate::BodyKind::Area => r3::RigidBodyType::Fixed,
                    crate::BodyKind::Rigid => r3::RigidBodyType::Dynamic,
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
                } else if rb.is_ccd_enabled() {
                    rb.enable_ccd(false);
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
                        assets.provider_mode,
                        assets.static_mesh_lookup,
                        assets.static_collision_trimesh_lookup,
                        &mut self.trimesh_cache,
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

        let mut stale = std::mem::take(&mut self.stale_ids_3d);
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
            set_body_handle(id, None);
        }
        stale.clear();
        self.stale_ids_3d = stale;
        self.world_3d = Some(world);
    }

    pub fn sync_joints_2d(&mut self, joints: &[crate::JointDesc2D]) {
        let Some(world) = self.world_2d.as_mut() else {
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
                crate::JointState2D {
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

    pub fn sync_joints_3d(&mut self, joints: &[crate::JointDesc3D]) {
        let Some(world) = self.world_3d.as_mut() else {
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
                crate::JointState3D {
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

    pub fn step_world_2d(&mut self, gravity_y: f32, fixed_delta: f32) {
        let Some(world) = self.world_2d.as_mut() else {
            return;
        };
        world.gravity.y = gravity_y;
        world.integration_parameters.dt = fixed_delta.max(0.000_1);
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

    pub fn step_world_3d(&mut self, gravity_y: f32, fixed_delta: f32) {
        let Some(world) = self.world_3d.as_mut() else {
            return;
        };
        world.gravity.y = gravity_y;
        world.integration_parameters.dt = fixed_delta.max(0.000_1);
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

    pub fn apply_pending_impulses_2d(&mut self, coef: f32) {
        let mut pending = std::mem::take(&mut self.pending_impulses_2d);
        let Some(world) = self.world_2d.as_mut() else {
            return;
        };
        for impulse in pending.drain(..) {
            let Some(state) = world.body_map.get(&impulse.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
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
        self.pending_impulses_2d = pending;
    }

    pub fn apply_pending_forces_2d(&mut self, coef: f32, fixed_delta: f32) {
        let mut pending = std::mem::take(&mut self.pending_forces_2d);
        let Some(world) = self.world_2d.as_mut() else {
            return;
        };
        let dt = fixed_delta.max(0.000_1);
        for force in pending.drain(..) {
            let Some(state) = world.body_map.get(&force.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
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
        self.pending_forces_2d = pending;
    }

    pub fn apply_pending_impulses_3d(&mut self, coef: f32) {
        let mut pending = std::mem::take(&mut self.pending_impulses_3d);
        let Some(world) = self.world_3d.as_mut() else {
            return;
        };
        for impulse in pending.drain(..) {
            let Some(state) = world.body_map.get(&impulse.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
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
        self.pending_impulses_3d = pending;
    }

    pub fn apply_pending_forces_3d(&mut self, coef: f32, fixed_delta: f32) {
        let mut pending = std::mem::take(&mut self.pending_forces_3d);
        let Some(world) = self.world_3d.as_mut() else {
            return;
        };
        let dt = fixed_delta.max(0.000_1);
        for force in pending.drain(..) {
            let Some(state) = world.body_map.get(&force.id) else {
                continue;
            };
            if state.kind != crate::BodyKind::Rigid {
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
        self.pending_forces_3d = pending;
    }

    pub fn raycast_2d(
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

        let world = self.world_2d.as_mut()?;
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

    pub fn raycast_3d(
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

        let world = self.world_3d.as_mut()?;
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

    pub fn shape_cast_2d(
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

        let world = self.world_2d.as_mut()?;
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

    pub fn shape_cast_3d(
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

        let world = self.world_3d.as_mut()?;
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

    pub fn contacts_2d(&self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        let Some(world) = self.world_2d.as_ref() else {
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

    pub fn contacts_3d(&self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        let Some(world) = self.world_3d.as_ref() else {
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

    pub fn update_query_pipeline_2d(&mut self) {
        if let Some(world) = self.world_2d.as_mut() {
            world.query_pipeline.update(&world.colliders);
        }
    }

    pub fn update_query_pipeline_3d(&mut self) {
        if let Some(world) = self.world_3d.as_mut() {
            world.query_pipeline.update(&world.colliders);
        }
    }

    pub fn prepared_audio_raycast_2d(
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

        let world = self.world_2d.as_ref()?;
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

    pub fn prepared_audio_raycast_3d(
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

        let world = self.world_3d.as_ref()?;
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

    pub fn cast_prepared_audio_rays(
        &self,
        inputs: &[AudioRaycastInput],
        outputs: &mut [AudioRaycastResult],
        parallel: bool,
    ) {
        let world_2d = self.world_2d.as_ref();
        let world_3d = self.world_3d.as_ref();
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
}

impl Default for PhysicsSystem {
    fn default() -> Self {
        Self::new()
    }
}
