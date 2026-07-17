use super::RuntimePhysicsStepTiming;
use crate::Runtime;
use ahash::{AHashMap, AHashSet};
#[cfg(test)]
use glam::{Mat3, Mat4};
use perro_ids::{NodeID, SignalID};
#[cfg(test)]
use perro_nodes::TileMap2D;
use perro_nodes::{SceneNodeData, Shape2D, Shape3D, WaterShape};
use perro_physics::*;
use perro_runtime_api::sub_apis::{
    NodeAPI, PhysicsContact2D, PhysicsContact3D, PhysicsMoveResult2D, PhysicsMoveResult3D,
    PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D,
    PhysicsSlideResult2D, PhysicsSlideResult3D, SignalAPI,
};
#[cfg(test)]
use perro_structs::BitMask;
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;
use rayon::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[path = "physics/water.rs"]
mod water;

use water::*;
pub(crate) use water::{
    RuntimeWater2D, RuntimeWater3D, RuntimeWaterBody2D, RuntimeWaterBody3D,
    lookup_water_body_sample, water_physics_sample_for_body_cached, water_target_submerged,
};

pub(crate) type PhysicsState = PhysicsSystem;

/// staged awake-body pose 4 SoA writeback (sync_world_to_nodes_2d).
/// dense scratch lane: rapier reads 1st, arena writes 2nd in slot order.
pub(crate) struct StagedBodyPose2D {
    pub id: NodeID,
    pub position: Vector2,
    pub rotation: f32,
    pub lin: Vector2,
    pub ang: f32,
}

/// staged awake-body pose 4 SoA writeback (sync_world_to_nodes_3d).
pub(crate) struct StagedBodyPose3D {
    pub id: NodeID,
    pub position: Vector3,
    pub rotation: Quaternion,
    pub lin: Vector3,
    pub ang: Vector3,
}

/// gap kp btw body + hit surface on move_and_slide sweep
const CHARACTER_MOVE_MARGIN: f32 = 0.005;
/// max slide iterations per move_and_slide call
const MAX_SLIDE_ITERATIONS: usize = 4;
pub(crate) use perro_physics::{AudioRaycastInput, AudioRaycastResult};

mod forces;
mod queries;
mod signals;
mod step;
mod world_sync;

fn body_sync_same_2d(
    state: &BodyState2D,
    position: Vector2,
    rotation: f32,
    linear_velocity: Vector2,
    angular_velocity: f32,
) -> bool {
    approx_eq_f32(state.last_translation[0], position.x)
        && approx_eq_f32(state.last_translation[1], position.y)
        && approx_eq_f32(state.last_rotation, rotation)
        && approx_eq_f32(state.last_linear_velocity[0], linear_velocity.x)
        && approx_eq_f32(state.last_linear_velocity[1], linear_velocity.y)
        && approx_eq_f32(state.last_angular_velocity, angular_velocity)
}

fn update_body_sync_state_2d(
    state: &mut BodyState2D,
    position: Vector2,
    rotation: f32,
    linear_velocity: Vector2,
    angular_velocity: f32,
    _sleeping: bool,
    same_as_last_sync: bool,
) {
    state.last_translation = [position.x, position.y];
    state.last_rotation = rotation;
    state.last_linear_velocity = [linear_velocity.x, linear_velocity.y];
    state.last_angular_velocity = angular_velocity;
    state.idle_sync_frames = if same_as_last_sync {
        state.idle_sync_frames.saturating_add(1)
    } else {
        0
    };
}

fn body_sync_same_3d(
    state: &BodyState3D,
    position: Vector3,
    rotation: Quaternion,
    linear_velocity: Vector3,
    angular_velocity: Vector3,
) -> bool {
    approx_eq_f32(state.last_translation[0], position.x)
        && approx_eq_f32(state.last_translation[1], position.y)
        && approx_eq_f32(state.last_translation[2], position.z)
        && approx_eq_f32(state.last_rotation[0], rotation.x)
        && approx_eq_f32(state.last_rotation[1], rotation.y)
        && approx_eq_f32(state.last_rotation[2], rotation.z)
        && approx_eq_f32(state.last_rotation[3], rotation.w)
        && approx_eq_f32(state.last_linear_velocity[0], linear_velocity.x)
        && approx_eq_f32(state.last_linear_velocity[1], linear_velocity.y)
        && approx_eq_f32(state.last_linear_velocity[2], linear_velocity.z)
        && approx_eq_f32(state.last_angular_velocity[0], angular_velocity.x)
        && approx_eq_f32(state.last_angular_velocity[1], angular_velocity.y)
        && approx_eq_f32(state.last_angular_velocity[2], angular_velocity.z)
}

fn update_body_sync_state_3d(
    state: &mut BodyState3D,
    position: Vector3,
    rotation: Quaternion,
    linear_velocity: Vector3,
    angular_velocity: Vector3,
    _sleeping: bool,
    same_as_last_sync: bool,
) {
    state.last_translation = [position.x, position.y, position.z];
    state.last_rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
    state.last_linear_velocity = [linear_velocity.x, linear_velocity.y, linear_velocity.z];
    state.last_angular_velocity = [angular_velocity.x, angular_velocity.y, angular_velocity.z];
    state.idle_sync_frames = if same_as_last_sync {
        state.idle_sync_frames.saturating_add(1)
    } else {
        0
    };
}

fn body_sync_signature_2d(
    kind: BodyKind,
    enabled: bool,
    global: Transform2D,
    rigid: Option<RigidProps2D>,
) -> u64 {
    let mut state = body_signature_seed(kind);
    state = hash_u32(state, enabled as u32);
    state = hash_transform_2d(state, global);
    if let Some(rigid) = rigid {
        state = hash_u32(state, 1);
        state = hash_u32(state, rigid.enabled as u32);
        state = hash_u32(state, rigid.can_sleep as u32);
        state = hash_u32(state, rigid.lock_rotation as u32);
        state = hash_f32(state, rigid.mass.to_bits());
        state = hash_f32(state, rigid.density.to_bits());
        state = hash_u32(state, rigid.continuous_collision_detection as u32);
        state = hash_f32(state, rigid.linear_velocity.x.to_bits());
        state = hash_f32(state, rigid.linear_velocity.y.to_bits());
        state = hash_f32(state, rigid.angular_velocity.to_bits());
        state = hash_f32(state, rigid.gravity_scale.to_bits());
        state = hash_f32(state, rigid.linear_damping.to_bits());
        hash_f32(state, rigid.angular_damping.to_bits())
    } else {
        hash_u32(state, 0)
    }
}

fn body_sync_signature_2d_if_useful(
    kind: BodyKind,
    enabled: bool,
    global: Transform2D,
    rigid: Option<RigidProps2D>,
) -> u64 {
    if rigid.is_some_and(|rigid| !rigid.can_sleep) {
        0
    } else {
        body_sync_signature_2d(kind, enabled, global, rigid)
    }
}

fn body_sync_signature_3d(
    kind: BodyKind,
    enabled: bool,
    global: Transform3D,
    rigid: Option<RigidProps3D>,
) -> u64 {
    let mut state = body_signature_seed(kind);
    state = hash_u32(state, enabled as u32);
    state = hash_transform_3d(state, global);
    if let Some(rigid) = rigid {
        state = hash_u32(state, 1);
        state = hash_u32(state, rigid.enabled as u32);
        state = hash_u32(state, rigid.can_sleep as u32);
        state = hash_f32(state, rigid.mass.to_bits());
        state = hash_f32(state, rigid.density.to_bits());
        state = hash_u32(state, rigid.continuous_collision_detection as u32);
        state = hash_f32(state, rigid.linear_velocity.x.to_bits());
        state = hash_f32(state, rigid.linear_velocity.y.to_bits());
        state = hash_f32(state, rigid.linear_velocity.z.to_bits());
        state = hash_f32(state, rigid.angular_velocity.x.to_bits());
        state = hash_f32(state, rigid.angular_velocity.y.to_bits());
        state = hash_f32(state, rigid.angular_velocity.z.to_bits());
        state = hash_f32(state, rigid.gravity_scale.to_bits());
        state = hash_f32(state, rigid.linear_damping.to_bits());
        hash_f32(state, rigid.angular_damping.to_bits())
    } else {
        hash_u32(state, 0)
    }
}

fn body_sync_signature_3d_if_useful(
    kind: BodyKind,
    enabled: bool,
    global: Transform3D,
    rigid: Option<RigidProps3D>,
) -> u64 {
    if rigid.is_some_and(|rigid| !rigid.can_sleep) {
        0
    } else {
        body_sync_signature_3d(kind, enabled, global, rigid)
    }
}

fn hash_water_shape(state: u64, shape: WaterShape) -> u64 {
    match shape {
        WaterShape::Rect { .. } | WaterShape::Circle { .. } => {
            hash_shape_2d(state, water_shape_2d(shape))
        }
        WaterShape::Box { .. } | WaterShape::Cylinder { .. } => {
            let (shape, _) = water_shape_3d(shape, 0.001);
            hash_shape_3d(state, &shape)
        }
    }
}

fn force_emitter_active(enabled: bool, pulse: bool, duration: f32, age: f32) -> bool {
    enabled && !(pulse && age > 0.0) && (duration <= 0.0 || age <= duration)
}

fn falloff_scale(dist: f32, radius: f32, falloff: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let t = (1.0 - dist / radius).clamp(0.0, 1.0);
    if falloff <= 0.0 {
        1.0
    } else if (falloff - 1.0).abs() <= f32::EPSILON {
        t
    } else {
        t.powf(falloff)
    }
}

fn force_emitter_force_2d(
    emitter: &perro_nodes::PhysicsForceEmitter2D,
    offset: Vector2,
    dist: f32,
) -> Vector2 {
    let scale = emitter.strength * falloff_scale(dist, emitter.radius, emitter.falloff);
    match emitter.profile {
        perro_nodes::PhysicsForceProfile::Lift => Vector2::new(0.0, 1.0) * scale,
        perro_nodes::PhysicsForceProfile::Explosion => {
            if dist <= 0.000_1 {
                Vector2::new(0.0, 1.0) * scale
            } else {
                offset.normalized() * scale
            }
        }
        perro_nodes::PhysicsForceProfile::Current => {
            emitter
                .vectors
                .first()
                .copied()
                .unwrap_or(Vector2::new(1.0, 0.0))
                * scale
        }
        perro_nodes::PhysicsForceProfile::Vortex => {
            let dir = if dist <= 0.000_1 {
                Vector2::new(1.0, 0.0)
            } else {
                offset.normalized()
            };
            Vector2::new(-dir.y, dir.x) * scale + dir * (-0.35 * scale)
        }
        perro_nodes::PhysicsForceProfile::Custom => {
            sample_force_vectors_2d(
                &emitter.vectors,
                if emitter.radius > 0.0 {
                    dist / emitter.radius
                } else {
                    0.0
                },
            ) * emitter.strength
        }
    }
}

fn force_emitter_force_3d(
    emitter: &perro_nodes::PhysicsForceEmitter3D,
    offset: Vector3,
    dist: f32,
) -> Vector3 {
    let scale = emitter.strength * falloff_scale(dist, emitter.radius, emitter.falloff);
    match emitter.profile {
        perro_nodes::PhysicsForceProfile::Lift => Vector3::new(0.0, 1.0, 0.0) * scale,
        perro_nodes::PhysicsForceProfile::Explosion => {
            if offset.length_squared() <= 0.000_1 {
                Vector3::new(0.0, 1.0, 0.0) * scale
            } else {
                offset.normalized() * scale
            }
        }
        perro_nodes::PhysicsForceProfile::Current => {
            emitter
                .vectors
                .first()
                .copied()
                .unwrap_or(Vector3::new(1.0, 0.0, 0.0))
                * scale
        }
        perro_nodes::PhysicsForceProfile::Vortex => {
            let flat = Vector2::new(offset.x, offset.z);
            let dir = if flat.length_squared() <= 0.000_1 {
                Vector2::new(1.0, 0.0)
            } else {
                flat.normalized()
            };
            Vector3::new(-dir.y * scale, 0.0, dir.x * scale)
                + Vector3::new(dir.x, 0.0, dir.y) * (-0.35 * scale)
        }
        perro_nodes::PhysicsForceProfile::Custom => {
            sample_force_vectors_3d(
                &emitter.vectors,
                if emitter.radius > 0.0 {
                    dist / emitter.radius
                } else {
                    0.0
                },
            ) * emitter.strength
        }
    }
}

fn sample_force_vectors_2d(vectors: &[Vector2], t: f32) -> Vector2 {
    if vectors.is_empty() {
        return Vector2::ZERO;
    }
    if vectors.len() == 1 {
        return vectors[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (vectors.len() - 1) as f32;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(vectors.len() - 1);
    let frac = scaled - idx as f32;
    vectors[idx] * (1.0 - frac) + vectors[next] * frac
}

fn sample_force_vectors_3d(vectors: &[Vector3], t: f32) -> Vector3 {
    if vectors.is_empty() {
        return Vector3::ZERO;
    }
    if vectors.len() == 1 {
        return vectors[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (vectors.len() - 1) as f32;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(vectors.len() - 1);
    let frac = scaled - idx as f32;
    vectors[idx] * (1.0 - frac) + vectors[next] * frac
}

#[cfg(test)]
mod tests;
