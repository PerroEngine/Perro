# Physics Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `apply_force_2d` | [`apply_force_2d`](#apply_force_2d) |
| `get_gravity` | [`get_gravity`](#get_gravity) |
| `set_gravity` | [`set_gravity`](#set_gravity) |
| `get_body_gravity_scale` | [`get_body_gravity_scale`](#get_body_gravity_scale) |
| `set_body_gravity_scale` | [`set_body_gravity_scale`](#set_body_gravity_scale) |
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
| `move_and_slide_2d` | [`move_and_slide_2d`](#move_and_slide_2d) |
| `move_and_slide_3d` | [`move_and_slide_3d`](#move_and_slide_3d) |
| `apply_gravity_2d` | [`apply_gravity_2d`](#apply_gravity_2d) |
| `apply_gravity_3d` | [`apply_gravity_3d`](#apply_gravity_3d) |
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
| `physics_get_body_gravity_scale` | [`physics_get_body_gravity_scale`](#physics_get_body_gravity_scale) |
| `physics_set_body_gravity_scale` | [`physics_set_body_gravity_scale`](#physics_set_body_gravity_scale) |
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
| `physics_move_and_slide_2d` | [`physics_move_and_slide_2d`](#physics_move_and_slide_2d) |
| `physics_move_and_slide_3d` | [`physics_move_and_slide_3d`](#physics_move_and_slide_3d) |
| `physics_apply_gravity_2d` | [`physics_apply_gravity_2d`](#physics_apply_gravity_2d) |
| `physics_apply_gravity_3d` | [`physics_apply_gravity_3d`](#physics_apply_gravity_3d) |
| `physics_contacts_2d` | [`physics_contacts_2d`](#physics_contacts_2d) |
| `physics_contacts_3d` | [`physics_contacts_3d`](#physics_contacts_3d) |
| `physics_pause` | [`physics_pause`](#physics_pause) |
| `physics_is_paused` | [`physics_is_paused`](#physics_is_paused) |

## Purpose

The physics module is how gameplay both drives and interrogates the physics
world. On the driving side it moves character bodies (`move_and_slide`,
`apply_gravity`) and pushes rigid bodies with forces and impulses. On the query
side it casts rays and shapes for line-of-sight, ground checks, and hit-scan
weapons, and reports contacts. It also carries trajectory solvers for aiming
lobbed projectiles and global/per-body gravity controls. Both 2D and 3D
variants exist for every core operation.

Character-body helpers keep you out of the raw solver: `move_and_slide` sweeps a
motion vector and slides along walls, while `apply_gravity` integrates fall
speed and reports grounding — you supply intent, the engine handles the sweep.

## Use Cases

- Platformer / character controller: slide along walls with `physics_move_and_slide_3d!(ctx.run, body, motion)` and fall with grounding via `physics_apply_gravity_3d!(ctx.run, body, dt)`.
- Hit-scan weapon or AI line-of-sight: `ctx.run.Physics().raycast_3d(origin, dir, max_distance)` returns the first `PhysicsRayHit3D`.
- Knockback, jump impulse, explosion push: `apply_impulse!(ctx.run, body, impulse)` for an instant kick, `apply_force!` for sustained force (both dispatch to 2D or 3D by the vector type).
- Ground / ledge / wall probe: a short raycast or `shape_cast_3d` in the desired direction.
- Aim a grenade or basketball arc: `ctx.run.Physics().solve_launch_velocity_3d(...)` (or `solve_velocity_to_target_3d`) computes the throw velocity; `predict_body_3d` previews the path.
- Floaty jumps or a low-gravity zone: `ctx.run.Physics().set_body_gravity_scale(body, 0.5)`.
- Pause the simulation for a menu or cutscene: `physics_pause!(ctx.run, true)`.
- React to collisions: read `physics_contacts_3d!(ctx.run, body)` and respond (damage, bounce, stick).

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Physics()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

A side-scroller character controller: read held movement input, slide the body
horizontally, and let engine gravity handle falling and landing. `on_jump` fires
an upward impulse when the jump action is pressed.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Clamp so a frame spike cannot fling the body through a wall.
        let dt = delta_time_capped!(ctx.run, 0.1);

        let mut motion = Vector3::ZERO;
        if action_down!(ctx.ipt, "move_right") {
            motion.x += 6.0 * dt;
        }
        if action_down!(ctx.ipt, "move_left") {
            motion.x -= 6.0 * dt;
        }

        // Slide along walls instead of stopping dead on contact.
        physics_move_and_slide_3d!(ctx.run, ctx.id, motion);

        // Engine gravity + ground detection, run separately from the slide.
        let _ = physics_apply_gravity_3d!(ctx.run, ctx.id, dt);

        if action_pressed!(ctx.ipt, "jump") {
            apply_impulse!(ctx.run, ctx.id, Vector3::new(0.0, 8.0, 0.0));
        }
    }
});
```

## API Reference

### `apply_force_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force_2d(&mut self, body_id: NodeID, force: Vector2) -> bool` |
| Params | `&mut self, body_id: NodeID, force: Vector2` |
| Returns | `bool` |
| Use when | Use `apply_force_2d` to apply force 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `apply_force_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_gravity(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use `get_gravity` to get gravity in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Has no optional/error return; `get_gravity` returns the documented value directly. |

### `set_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_gravity(&mut self, gravity: f32)` |
| Params | `&mut self, gravity: f32` |
| Returns | `()` |
| Use when | Use `set_gravity` to set gravity in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Has no failure return; `set_gravity` sends the command through the runtime module and the caller receives no acknowledgement. |

### `get_body_gravity_scale`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_body_gravity_scale(&mut self, body_id: NodeID) -> Option<f32>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Option<f32>` |
| Use when | Read local gravity multiplier for `RigidBody2D` or `RigidBody3D`. |
| Fails when / edge behavior | Returns `None` when node is missing or not a rigid body. |

### `set_body_gravity_scale`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_body_gravity_scale(&mut self, body_id: NodeID, scale: f32) -> bool` |
| Params | `&mut self, body_id: NodeID, scale: f32` |
| Returns | `bool` |
| Use when | Set local gravity multiplier for `RigidBody2D` or `RigidBody3D`. |
| Fails when / edge behavior | Returns `false` when node is missing, not a rigid body, or `scale` is not finite. Effective gravity is `world gravity * physics coefficient * gravity_scale`. |

### `get_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_coefficient(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use `get_coefficient` to get coefficient in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Has no optional/error return; `get_coefficient` returns the documented value directly. |

### `set_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_coefficient(&mut self, coefficient: f32)` |
| Params | `&mut self, coefficient: f32` |
| Returns | `()` |
| Use when | Use `set_coefficient` to set coefficient in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Has no failure return; `set_coefficient` sends the command through the runtime module and the caller receives no acknowledgement. |

### `apply_force_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool` |
| Params | `&mut self, body_id: NodeID, force: Vector3` |
| Returns | `bool` |
| Use when | Use `apply_force_3d` to apply force 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `apply_force_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `apply_impulse_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool` |
| Params | `&mut self, body_id: NodeID, impulse: Vector2` |
| Returns | `bool` |
| Use when | Use `apply_impulse_2d` to apply impulse 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `apply_impulse_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `apply_impulse_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool` |
| Params | `&mut self, body_id: NodeID, impulse: Vector3` |
| Returns | `bool` |
| Use when | Use `apply_impulse_3d` to apply impulse 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `apply_impulse_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `emit_force_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn emit_force_2d(&mut self, emitter: PhysicsForceEmitter2D) -> bool` |
| Params | `&mut self, emitter: PhysicsForceEmitter2D` |
| Returns | `bool` |
| Use when | Use `emit_force_2d` to emit force 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `emit_force_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `emit_force_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn emit_force_3d(&mut self, emitter: PhysicsForceEmitter3D) -> bool` |
| Params | `&mut self, emitter: PhysicsForceEmitter3D` |
| Returns | `bool` |
| Use when | Use `emit_force_3d` to emit force 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `emit_force_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `apply_force`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force<D>(&mut self, body_id: NodeID, force: D) -> bool where D: IntoImpulseDirection,` |
| Params | `&mut self, body_id: NodeID, force: D` |
| Returns | `bool where D: IntoImpulseDirection,` |
| Use when | Use `apply_force` to apply force in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `apply_force` cannot apply to the supplied target or inputs; `true` confirms success. |

### `apply_impulse`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse<D>(&mut self, body_id: NodeID, impulse: D) -> bool where D: IntoImpulseDirection,` |
| Params | `&mut self, body_id: NodeID, impulse: D` |
| Returns | `bool where D: IntoImpulseDirection,` |
| Use when | Use `apply_impulse` to apply impulse in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `false` when `apply_impulse` cannot apply to the supplied target or inputs; `true` confirms success. |

### `raycast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use `raycast_3d` to raycast 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `raycast_3d` cannot produce a value for the supplied target or inputs. |

### `raycast_3d_with_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_with_areas( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use `raycast_3d_with_areas` to raycast 3d with areas in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `raycast_3d_with_areas` cannot produce a value for the supplied target or inputs. |

### `raycast_3d_without_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_without_areas( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use `raycast_3d_without_areas` to raycast 3d without areas in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `raycast_3d_without_areas` cannot produce a value for the supplied target or inputs. |

### `raycast_3d_filtered`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_filtered( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use `raycast_3d_filtered` to raycast 3d filtered in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `raycast_3d_filtered` cannot produce a value for the supplied target or inputs. |

### `raycast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_2d( &mut self, origin: Vector2, direction: Vector2, max_distance: f32, ) -> Option<PhysicsRayHit2D>` |
| Params | `&mut self, origin: Vector2, direction: Vector2, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit2D>` |
| Use when | Use `raycast_2d` to raycast 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `raycast_2d` cannot produce a value for the supplied target or inputs. |

### `raycast_2d_filtered`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_2d_filtered( &mut self, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsRayHit2D>` |
| Params | `&mut self, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsRayHit2D>` |
| Use when | Use `raycast_2d_filtered` to raycast 2d filtered in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `raycast_2d_filtered` cannot produce a value for the supplied target or inputs. |

### `shape_cast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn shape_cast_2d( &mut self, shape: Shape2D, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsShapeHit2D>` |
| Params | `&mut self, shape: Shape2D, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsShapeHit2D>` |
| Use when | Use `shape_cast_2d` to shape cast 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `shape_cast_2d` cannot produce a value for the supplied target or inputs. |

### `shape_cast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn shape_cast_3d( &mut self, shape: Shape3D, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsShapeHit3D>` |
| Params | `&mut self, shape: Shape3D, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsShapeHit3D>` |
| Use when | Use `shape_cast_3d` to shape cast 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `shape_cast_3d` cannot produce a value for the supplied target or inputs. |

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

### `move_and_slide_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn move_and_slide_2d(&mut self, body_id: NodeID, motion: Vector2, filter: PhysicsQueryFilter) -> Option<PhysicsSlideResult2D>` |
| Params | `&mut self, body_id: NodeID, motion: Vector2, filter: PhysicsQueryFilter` |
| Returns | `Option<PhysicsSlideResult2D>` |
| Use when | Move a character-style body by a motion vector, sliding along hit surfaces instead of stopping. |
| Fails when / edge behavior | Sweeps up to 4 slide iterations, projecting unconsumed motion onto each hit plane. Writes the safe global position. `remainder` holds motion still blocked (e.g. cornered). `hits` lists each clipped iteration in order. |

### `move_and_slide_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn move_and_slide_3d(&mut self, body_id: NodeID, motion: Vector3, filter: PhysicsQueryFilter) -> Option<PhysicsSlideResult3D>` |
| Params | `&mut self, body_id: NodeID, motion: Vector3, filter: PhysicsQueryFilter` |
| Returns | `Option<PhysicsSlideResult3D>` |
| Use when | Move a character-style body by a motion vector, sliding along hit surfaces instead of stopping. |
| Fails when / edge behavior | Sweeps up to 4 slide iterations, projecting unconsumed motion onto each hit plane. Writes the safe global position. `remainder` holds motion still blocked (e.g. cornered). `hits` lists each clipped iteration in order. |

### `apply_gravity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_gravity_2d(&mut self, body_id: NodeID, dt: f32, max_fall_speed: f32, filter: PhysicsQueryFilter) -> Option<PhysicsMoveResult2D>` |
| Params | `&mut self, body_id: NodeID, dt: f32, max_fall_speed: f32, filter: PhysicsQueryFilter` |
| Returns | `Option<PhysicsMoveResult2D>` |
| Use when | Script wants engine gravity on a character body without owning the fall-speed integration. Call once per update; separate from `move_and_slide`. |
| Fails when / edge behavior | Character bodies only — returns `None` for other body types or non-positive `dt`. Integrates an internal fall speed from world gravity (`physics_set_gravity` respected), clamps to `max_fall_speed`, sweeps down, resets fall speed on landing. `clipped == true` in the result means grounded. |

### `apply_gravity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_gravity_3d(&mut self, body_id: NodeID, dt: f32, max_fall_speed: f32, filter: PhysicsQueryFilter) -> Option<PhysicsMoveResult3D>` |
| Params | `&mut self, body_id: NodeID, dt: f32, max_fall_speed: f32, filter: PhysicsQueryFilter` |
| Returns | `Option<PhysicsMoveResult3D>` |
| Use when | Script wants engine gravity on a character body without owning the fall-speed integration. Call once per update; separate from `move_and_slide`. |
| Fails when / edge behavior | Character bodies only — returns `None` for other body types or non-positive `dt`. Integrates an internal fall speed from world gravity (`physics_set_gravity` respected), clamps to `max_fall_speed`, sweeps down, resets fall speed on landing. `clipped == true` in the result means grounded. |

### `contacts_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Vec<PhysicsContact2D>` |
| Use when | Use `contacts_2d` to contacts 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Returns an empty vector when `contacts_2d` finds no values; callers must treat zero results as normal. |

### `contacts_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Vec<PhysicsContact3D>` |
| Use when | Use `contacts_3d` to contacts 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Returns an empty vector when `contacts_3d` finds no values; callers must treat zero results as normal. |

### `solve_velocity_to_target_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_velocity_to_target_2d( &mut self, origin: Vector2, target: Vector2, time: f32, drift: Vector2, ) -> Option<Vector2>` |
| Params | `&mut self, origin: Vector2, target: Vector2, time: f32, drift: Vector2,` |
| Returns | `Option<Vector2>` |
| Use when | Use `solve_velocity_to_target_2d` to solve velocity to target 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `solve_velocity_to_target_2d` cannot produce a value for the supplied target or inputs. |

### `solve_velocity_to_target_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_velocity_to_target_3d( &mut self, origin: Vector3, target: Vector3, time: f32, drift: Vector3, ) -> Option<Vector3>` |
| Params | `&mut self, origin: Vector3, target: Vector3, time: f32, drift: Vector3,` |
| Returns | `Option<Vector3>` |
| Use when | Use `solve_velocity_to_target_3d` to solve velocity to target 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `solve_velocity_to_target_3d` cannot produce a value for the supplied target or inputs. |

### `solve_launch_velocity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_launch_velocity_2d( &mut self, origin: Vector2, target: Vector2, speed: f32, max_time: f32, drift: Vector2, ) -> Option<PhysicsLaunchSolution2D>` |
| Params | `&mut self, origin: Vector2, target: Vector2, speed: f32, max_time: f32, drift: Vector2,` |
| Returns | `Option<PhysicsLaunchSolution2D>` |
| Use when | Use `solve_launch_velocity_2d` to solve launch velocity 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `solve_launch_velocity_2d` cannot produce a value for the supplied target or inputs. |

### `solve_launch_velocity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_launch_velocity_3d( &mut self, origin: Vector3, target: Vector3, speed: f32, max_time: f32, drift: Vector3, ) -> Option<PhysicsLaunchSolution3D>` |
| Params | `&mut self, origin: Vector3, target: Vector3, speed: f32, max_time: f32, drift: Vector3,` |
| Returns | `Option<PhysicsLaunchSolution3D>` |
| Use when | Use `solve_launch_velocity_3d` to solve launch velocity 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `solve_launch_velocity_3d` cannot produce a value for the supplied target or inputs. |

### `predict_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn predict_body_2d( &mut self, body_id: NodeID, time: f32, drift: Vector2, ) -> Option<PhysicsBodyPrediction2D>` |
| Params | `&mut self, body_id: NodeID, time: f32, drift: Vector2,` |
| Returns | `Option<PhysicsBodyPrediction2D>` |
| Use when | Use `predict_body_2d` to predict body 2d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `predict_body_2d` cannot produce a value for the supplied target or inputs. |

### `predict_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn predict_body_3d( &mut self, body_id: NodeID, time: f32, drift: Vector3, ) -> Option<PhysicsBodyPrediction3D>` |
| Params | `&mut self, body_id: NodeID, time: f32, drift: Vector3,` |
| Returns | `Option<PhysicsBodyPrediction3D>` |
| Use when | Use `predict_body_3d` to predict body 3d in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Returns `None` when `predict_body_3d` cannot produce a value for the supplied target or inputs. |

### `pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn pause(&mut self, paused: bool)` |
| Params | `&mut self, paused: bool` |
| Returns | `()` |
| Use when | Use `pause` to pause in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Has no failure return; `pause` sends the command through the runtime module and the caller receives no acknowledgement. |

### `is_paused`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn is_paused(&mut self) -> bool` |
| Params | `&mut self` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | Returns `false` when `is_paused` cannot apply to the supplied target or inputs; `true` confirms success. |

### `apply_force`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `apply_force!(ctx.run, body_id, force)` |
| Params | `ctx, body_id, force` |
| Returns | `same as backing method` |
| Use when | Use `apply_force` to apply force in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Uses the backing `apply_force` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_get_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_gravity!(ctx.run)` |
| Params | `ctx` |
| Returns | `same as backing method` |
| Use when | Use `physics_get_gravity` to physics get gravity in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_get_gravity` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_set_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_gravity!(ctx.run, gravity)` |
| Params | `ctx, gravity` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `physics_set_gravity` to physics set gravity in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Returns `false` when `physics_set_gravity` cannot apply to the supplied target or inputs; `true` confirms success. |

### `physics_get_body_gravity_scale`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_body_gravity_scale!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `Option<f32>` |
| Use when | Read local gravity multiplier for a rigid body. |
| Fails when / edge behavior | Returns `None` when node is missing or not a rigid body. |

### `physics_set_body_gravity_scale`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_body_gravity_scale!(ctx.run, body_id, scale)` |
| Params | `ctx, body_id, scale` |
| Returns | `bool` |
| Use when | Set local gravity multiplier for a rigid body. |
| Fails when / edge behavior | Returns `false` when node is missing, not a rigid body, or `scale` is not finite. |

### `physics_get_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_coefficient!(ctx.run)` |
| Params | `ctx` |
| Returns | `same as backing method` |
| Use when | Use `physics_get_coefficient` to physics get coefficient in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_get_coefficient` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_set_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_coefficient!(ctx.run, coefficient)` |
| Params | `ctx, coefficient` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `physics_set_coefficient` to physics set coefficient in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Returns `false` when `physics_set_coefficient` cannot apply to the supplied target or inputs; `true` confirms success. |

### `physics_solve_velocity_to_target_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_velocity_to_target_2d!(ctx.run, origin, target, time)` |
| Params | `ctx, origin, target, time` |
| Returns | `same as backing method` |
| Use when | Use `physics_solve_velocity_to_target_2d` to physics solve velocity to target 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_solve_velocity_to_target_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_solve_velocity_to_target_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_velocity_to_target_3d!(ctx.run, origin, target, time)` |
| Params | `ctx, origin, target, time` |
| Returns | `same as backing method` |
| Use when | Use `physics_solve_velocity_to_target_3d` to physics solve velocity to target 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_solve_velocity_to_target_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_solve_launch_velocity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_launch_velocity_2d!(ctx.run, origin, target, speed, max_time)` |
| Params | `ctx, origin, target, speed, max_time` |
| Returns | `same as backing method` |
| Use when | Use `physics_solve_launch_velocity_2d` to physics solve launch velocity 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_solve_launch_velocity_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_solve_launch_velocity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_launch_velocity_3d!(ctx.run, origin, target, speed, max_time)` |
| Params | `ctx, origin, target, speed, max_time` |
| Returns | `same as backing method` |
| Use when | Use `physics_solve_launch_velocity_3d` to physics solve launch velocity 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_solve_launch_velocity_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_predict_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_predict_body_2d!(ctx.run, body_id, time)` |
| Params | `ctx, body_id, time` |
| Returns | `same as backing method` |
| Use when | Use `physics_predict_body_2d` to physics predict body 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_predict_body_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_predict_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_predict_body_3d!(ctx.run, body_id, time)` |
| Params | `ctx, body_id, time` |
| Returns | `same as backing method` |
| Use when | Use `physics_predict_body_3d` to physics predict body 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_predict_body_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `apply_impulse`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `apply_impulse!(ctx.run, body_id, impulse)` |
| Params | `ctx, body_id, impulse` |
| Returns | `same as backing method` |
| Use when | Use `apply_impulse` to apply impulse in the physics world; reads are snapshots and force/state calls affect runtime bodies. |
| Fails when / edge behavior | Uses the backing `apply_impulse` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_raycast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use `physics_raycast_3d` to physics raycast 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_raycast_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_raycast_3d_with_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d_with_areas!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use `physics_raycast_3d_with_areas` to physics raycast 3d with areas in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_raycast_3d_with_areas` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_raycast_3d_without_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d_without_areas!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use `physics_raycast_3d_without_areas` to physics raycast 3d without areas in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_raycast_3d_without_areas` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_raycast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_2d!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use `physics_raycast_2d` to physics raycast 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_raycast_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_shape_cast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_shape_cast_2d!(ctx.run, shape, origin, direction, max_distance, filter)` |
| Params | `ctx, shape, origin, direction, max_distance, filter` |
| Returns | `same as backing method` |
| Use when | Use `physics_shape_cast_2d` to physics shape cast 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_shape_cast_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_shape_cast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_shape_cast_3d!(ctx.run, shape, origin, direction, max_distance, filter)` |
| Params | `ctx, shape, origin, direction, max_distance, filter` |
| Returns | `same as backing method` |
| Use when | Use `physics_shape_cast_3d` to physics shape cast 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_shape_cast_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

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

### `physics_move_and_slide_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_move_and_slide_2d!(ctx.run, body_id, motion)` |
| Params | `ctx, body_id, motion` |
| Returns | `same as backing method` |
| Use when | Use when script wants character-style movement that slides along hit surfaces. |
| Fails when / edge behavior | Short form uses `PhysicsQueryFilter::default()` (all layers); full macro form accepts `filter`. Sensor/area colliders never block the sweep. |

### `physics_move_and_slide_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_move_and_slide_3d!(ctx.run, body_id, motion)` |
| Params | `ctx, body_id, motion` |
| Returns | `same as backing method` |
| Use when | Use when script wants character-style movement that slides along hit surfaces. |
| Fails when / edge behavior | Short form uses `PhysicsQueryFilter::default()` (all layers); full macro form accepts `filter`. Sensor/area colliders never block the sweep. |

### `physics_apply_gravity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_apply_gravity_2d!(ctx.run, body_id, dt)` |
| Params | `ctx, body_id, dt` |
| Returns | `same as backing method` |
| Use when | Use when script wants engine gravity on a character body each update, separate from move/slide calls. |
| Fails when / edge behavior | Short form uses terminal fall speed `64.0` and `PhysicsQueryFilter::default()`; full form is `(ctx, body_id, dt, max_fall_speed, filter)`. |

### `physics_apply_gravity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_apply_gravity_3d!(ctx.run, body_id, dt)` |
| Params | `ctx, body_id, dt` |
| Returns | `same as backing method` |
| Use when | Use when script wants engine gravity on a character body each update, separate from move/slide calls. |
| Fails when / edge behavior | Short form uses terminal fall speed `64.0` and `PhysicsQueryFilter::default()`; full form is `(ctx, body_id, dt, max_fall_speed, filter)`. |

### `physics_contacts_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_contacts_2d!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `same as backing method` |
| Use when | Use `physics_contacts_2d` to physics contacts 2d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_contacts_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_contacts_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_contacts_3d!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `same as backing method` |
| Use when | Use `physics_contacts_3d` to physics contacts 3d in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Uses the backing `physics_contacts_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `physics_pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_pause!(ctx.run, paused)` |
| Params | `ctx, paused` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `physics_pause` to physics pause in the physics world; queries return a snapshot while setters/forces mutate runtime body state. |
| Fails when / edge behavior | Returns `false` when `physics_pause` cannot apply to the supplied target or inputs; `true` confirms success. |

### `physics_is_paused`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_is_paused!(ctx.run)` |
| Params | `ctx` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | Returns `false` when `physics_is_paused` cannot apply to the supplied target or inputs; `true` confirms success. |

