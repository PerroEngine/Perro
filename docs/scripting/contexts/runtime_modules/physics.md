# Physics Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `apply_force_2d` | [`apply_force_2d`](#apply_force_2d) |
| `get_gravity` | [`get_gravity`](#get_gravity) |
| `set_gravity` | [`set_gravity`](#set_gravity) |
| `get_coefficient` | [`get_coefficient`](#get_coefficient) |
| `set_coefficient` | [`set_coefficient`](#set_coefficient) |
| `apply_force_3d` | [`apply_force_3d`](#apply_force_3d) |
| `apply_impulse_2d` | [`apply_impulse_2d`](#apply_impulse_2d) |
| `apply_impulse_3d` | [`apply_impulse_3d`](#apply_impulse_3d) |
| `emit_force_2d` | [`emit_force_2d`](#emit_force_2d) |
| `emit_force_3d` | [`emit_force_3d`](#emit_force_3d) |
| `apply_force` | [`apply_force`](#apply_force) |
| `apply_impulse` | [`apply_impulse`](#apply_impulse) |
| `raycast_3d` | [`raycast_3d`](#raycast_3d) |
| `raycast_3d_with_areas` | [`raycast_3d_with_areas`](#raycast_3d_with_areas) |
| `raycast_3d_without_areas` | [`raycast_3d_without_areas`](#raycast_3d_without_areas) |
| `raycast_3d_filtered` | [`raycast_3d_filtered`](#raycast_3d_filtered) |
| `raycast_2d` | [`raycast_2d`](#raycast_2d) |
| `raycast_2d_filtered` | [`raycast_2d_filtered`](#raycast_2d_filtered) |
| `shape_cast_2d` | [`shape_cast_2d`](#shape_cast_2d) |
| `shape_cast_3d` | [`shape_cast_3d`](#shape_cast_3d) |
| `move_body_2d` | [`move_body_2d`](#move_body_2d) |
| `move_body_3d` | [`move_body_3d`](#move_body_3d) |
| `contacts_2d` | [`contacts_2d`](#contacts_2d) |
| `contacts_3d` | [`contacts_3d`](#contacts_3d) |
| `solve_velocity_to_target_2d` | [`solve_velocity_to_target_2d`](#solve_velocity_to_target_2d) |
| `solve_velocity_to_target_3d` | [`solve_velocity_to_target_3d`](#solve_velocity_to_target_3d) |
| `solve_launch_velocity_2d` | [`solve_launch_velocity_2d`](#solve_launch_velocity_2d) |
| `solve_launch_velocity_3d` | [`solve_launch_velocity_3d`](#solve_launch_velocity_3d) |
| `predict_body_2d` | [`predict_body_2d`](#predict_body_2d) |
| `predict_body_3d` | [`predict_body_3d`](#predict_body_3d) |
| `pause` | [`pause`](#pause) |
| `is_paused` | [`is_paused`](#is_paused) |
| `apply_force` | [`apply_force`](#apply_force) |
| `physics_get_gravity` | [`physics_get_gravity`](#physics_get_gravity) |
| `physics_set_gravity` | [`physics_set_gravity`](#physics_set_gravity) |
| `physics_get_coefficient` | [`physics_get_coefficient`](#physics_get_coefficient) |
| `physics_set_coefficient` | [`physics_set_coefficient`](#physics_set_coefficient) |
| `physics_solve_velocity_to_target_2d` | [`physics_solve_velocity_to_target_2d`](#physics_solve_velocity_to_target_2d) |
| `physics_solve_velocity_to_target_3d` | [`physics_solve_velocity_to_target_3d`](#physics_solve_velocity_to_target_3d) |
| `physics_solve_launch_velocity_2d` | [`physics_solve_launch_velocity_2d`](#physics_solve_launch_velocity_2d) |
| `physics_solve_launch_velocity_3d` | [`physics_solve_launch_velocity_3d`](#physics_solve_launch_velocity_3d) |
| `physics_predict_body_2d` | [`physics_predict_body_2d`](#physics_predict_body_2d) |
| `physics_predict_body_3d` | [`physics_predict_body_3d`](#physics_predict_body_3d) |
| `apply_impulse` | [`apply_impulse`](#apply_impulse) |
| `physics_raycast_3d` | [`physics_raycast_3d`](#physics_raycast_3d) |
| `physics_raycast_3d_with_areas` | [`physics_raycast_3d_with_areas`](#physics_raycast_3d_with_areas) |
| `physics_raycast_3d_without_areas` | [`physics_raycast_3d_without_areas`](#physics_raycast_3d_without_areas) |
| `physics_raycast_2d` | [`physics_raycast_2d`](#physics_raycast_2d) |
| `physics_shape_cast_2d` | [`physics_shape_cast_2d`](#physics_shape_cast_2d) |
| `physics_shape_cast_3d` | [`physics_shape_cast_3d`](#physics_shape_cast_3d) |
| `physics_move_body_2d` | [`physics_move_body_2d`](#physics_move_body_2d) |
| `physics_move_body_3d` | [`physics_move_body_3d`](#physics_move_body_3d) |
| `physics_contacts_2d` | [`physics_contacts_2d`](#physics_contacts_2d) |
| `physics_contacts_3d` | [`physics_contacts_3d`](#physics_contacts_3d) |
| `physics_pause` | [`physics_pause`](#physics_pause) |
| `physics_is_paused` | [`physics_is_paused`](#physics_is_paused) |

## Overview

This runtime module belongs to `ctx.run` and documents physics calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Physics()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `apply_force_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force_2d(&mut self, body_id: NodeID, force: Vector2) -> bool` |
| Params | `&mut self, body_id: NodeID, force: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_gravity(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_gravity(&mut self, gravity: f32)` |
| Params | `&mut self, gravity: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_coefficient(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_coefficient(&mut self, coefficient: f32)` |
| Params | `&mut self, coefficient: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_force_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool` |
| Params | `&mut self, body_id: NodeID, force: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_impulse_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool` |
| Params | `&mut self, body_id: NodeID, impulse: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_impulse_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool` |
| Params | `&mut self, body_id: NodeID, impulse: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `emit_force_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn emit_force_2d(&mut self, emitter: PhysicsForceEmitter2D) -> bool` |
| Params | `&mut self, emitter: PhysicsForceEmitter2D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `emit_force_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn emit_force_3d(&mut self, emitter: PhysicsForceEmitter3D) -> bool` |
| Params | `&mut self, emitter: PhysicsForceEmitter3D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_force`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force<D>(&mut self, body_id: NodeID, force: D) -> bool where D: IntoImpulseDirection,` |
| Params | `&mut self, body_id: NodeID, force: D` |
| Returns | `bool where D: IntoImpulseDirection,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_impulse`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse<D>(&mut self, body_id: NodeID, impulse: D) -> bool where D: IntoImpulseDirection,` |
| Params | `&mut self, body_id: NodeID, impulse: D` |
| Returns | `bool where D: IntoImpulseDirection,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `raycast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `raycast_3d_with_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_with_areas( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `raycast_3d_without_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_without_areas( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `raycast_3d_filtered`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_filtered( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `raycast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_2d( &mut self, origin: Vector2, direction: Vector2, max_distance: f32, ) -> Option<PhysicsRayHit2D>` |
| Params | `&mut self, origin: Vector2, direction: Vector2, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `raycast_2d_filtered`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_2d_filtered( &mut self, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsRayHit2D>` |
| Params | `&mut self, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsRayHit2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `shape_cast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn shape_cast_2d( &mut self, shape: Shape2D, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsShapeHit2D>` |
| Params | `&mut self, shape: Shape2D, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsShapeHit2D>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `shape_cast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn shape_cast_3d( &mut self, shape: Shape3D, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsShapeHit3D>` |
| Params | `&mut self, shape: Shape3D, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsShapeHit3D>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `move_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn move_body_2d(&mut self, body_id: NodeID, target: Vector2, margin: f32, filter: PhysicsQueryFilter) -> Option<PhysicsMoveResult2D>` |
| Params | `&mut self, body_id: NodeID, target: Vector2, margin: f32, filter: PhysicsQueryFilter` |
| Returns | `Option<PhysicsMoveResult2D>` |
| Use when | Move a physics body toward a target position without clipping through blocking colliders. |
| Fails when / edge behavior | Syncs current physics bodies, sweeps attached body colliders, excludes the moving body, writes the safe global position, and returns hit/clipped state. Does not clear velocity. |

### `move_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn move_body_3d(&mut self, body_id: NodeID, target: Vector3, margin: f32, filter: PhysicsQueryFilter) -> Option<PhysicsMoveResult3D>` |
| Params | `&mut self, body_id: NodeID, target: Vector3, margin: f32, filter: PhysicsQueryFilter` |
| Returns | `Option<PhysicsMoveResult3D>` |
| Use when | Move a physics body toward a target position without clipping through blocking colliders. |
| Fails when / edge behavior | Syncs current physics bodies, sweeps attached body colliders, excludes the moving body, writes the safe global position, and returns hit/clipped state. Does not clear velocity. |

### `contacts_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Vec<PhysicsContact2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `contacts_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Vec<PhysicsContact3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `solve_velocity_to_target_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_velocity_to_target_2d( &mut self, origin: Vector2, target: Vector2, time: f32, drift: Vector2, ) -> Option<Vector2>` |
| Params | `&mut self, origin: Vector2, target: Vector2, time: f32, drift: Vector2,` |
| Returns | `Option<Vector2>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `solve_velocity_to_target_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_velocity_to_target_3d( &mut self, origin: Vector3, target: Vector3, time: f32, drift: Vector3, ) -> Option<Vector3>` |
| Params | `&mut self, origin: Vector3, target: Vector3, time: f32, drift: Vector3,` |
| Returns | `Option<Vector3>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `solve_launch_velocity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_launch_velocity_2d( &mut self, origin: Vector2, target: Vector2, speed: f32, max_time: f32, drift: Vector2, ) -> Option<PhysicsLaunchSolution2D>` |
| Params | `&mut self, origin: Vector2, target: Vector2, speed: f32, max_time: f32, drift: Vector2,` |
| Returns | `Option<PhysicsLaunchSolution2D>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `solve_launch_velocity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_launch_velocity_3d( &mut self, origin: Vector3, target: Vector3, speed: f32, max_time: f32, drift: Vector3, ) -> Option<PhysicsLaunchSolution3D>` |
| Params | `&mut self, origin: Vector3, target: Vector3, speed: f32, max_time: f32, drift: Vector3,` |
| Returns | `Option<PhysicsLaunchSolution3D>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `predict_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn predict_body_2d( &mut self, body_id: NodeID, time: f32, drift: Vector2, ) -> Option<PhysicsBodyPrediction2D>` |
| Params | `&mut self, body_id: NodeID, time: f32, drift: Vector2,` |
| Returns | `Option<PhysicsBodyPrediction2D>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `predict_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn predict_body_3d( &mut self, body_id: NodeID, time: f32, drift: Vector3, ) -> Option<PhysicsBodyPrediction3D>` |
| Params | `&mut self, body_id: NodeID, time: f32, drift: Vector3,` |
| Returns | `Option<PhysicsBodyPrediction3D>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn pause(&mut self, paused: bool)` |
| Params | `&mut self, paused: bool` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `is_paused`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn is_paused(&mut self) -> bool` |
| Params | `&mut self` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_force`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `apply_force!(ctx.run, body_id, force)` |
| Params | `ctx, body_id, force` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_get_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_gravity!(ctx.run)` |
| Params | `ctx` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_set_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_gravity!(ctx.run, gravity)` |
| Params | `ctx, gravity` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_get_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_coefficient!(ctx.run)` |
| Params | `ctx` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_set_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_coefficient!(ctx.run, coefficient)` |
| Params | `ctx, coefficient` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_solve_velocity_to_target_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_velocity_to_target_2d!(ctx.run, origin, target, time)` |
| Params | `ctx, origin, target, time` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_solve_velocity_to_target_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_velocity_to_target_3d!(ctx.run, origin, target, time)` |
| Params | `ctx, origin, target, time` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_solve_launch_velocity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_launch_velocity_2d!(ctx.run, origin, target, speed, max_time)` |
| Params | `ctx, origin, target, speed, max_time` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_solve_launch_velocity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_launch_velocity_3d!(ctx.run, origin, target, speed, max_time)` |
| Params | `ctx, origin, target, speed, max_time` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_predict_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_predict_body_2d!(ctx.run, body_id, time)` |
| Params | `ctx, body_id, time` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_predict_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_predict_body_3d!(ctx.run, body_id, time)` |
| Params | `ctx, body_id, time` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `apply_impulse`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `apply_impulse!(ctx.run, body_id, impulse)` |
| Params | `ctx, body_id, impulse` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_raycast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_raycast_3d_with_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d_with_areas!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_raycast_3d_without_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d_without_areas!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_raycast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_2d!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_shape_cast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_shape_cast_2d!(ctx.run, shape, origin, direction, max_distance, filter)` |
| Params | `ctx, shape, origin, direction, max_distance, filter` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_shape_cast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_shape_cast_3d!(ctx.run, shape, origin, direction, max_distance, filter)` |
| Params | `ctx, shape, origin, direction, max_distance, filter` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_move_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_move_body_2d!(ctx.run, body_id, target)` |
| Params | `ctx, body_id, target` |
| Returns | `same as backing method` |
| Use when | Use when script wants clipped manual physics-body movement. |
| Fails when / edge behavior | Default margin is `0.001`; full macro form accepts `margin` and `filter`. |

### `physics_move_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_move_body_3d!(ctx.run, body_id, target)` |
| Params | `ctx, body_id, target` |
| Returns | `same as backing method` |
| Use when | Use when script wants clipped manual physics-body movement. |
| Fails when / edge behavior | Default margin is `0.001`; full macro form accepts `margin` and `filter`. |

### `physics_contacts_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_contacts_2d!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_contacts_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_contacts_3d!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_pause!(ctx.run, paused)` |
| Params | `ctx, paused` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `physics_is_paused`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_is_paused!(ctx.run)` |
| Params | `ctx` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

