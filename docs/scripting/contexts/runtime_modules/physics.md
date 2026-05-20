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
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().apply_force_2d(ctx.id, Vector2::new(0.0, 0.0));
        let _ = value;
    }
});
```

### `get_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_gravity(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().get_gravity();
        let _ = value;
    }
});
```

### `set_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_gravity(&mut self, gravity: f32)` |
| Params | `&mut self, gravity: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().set_gravity(1.0);
        let _ = value;
    }
});
```

### `get_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn get_coefficient(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().get_coefficient();
        let _ = value;
    }
});
```

### `set_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn set_coefficient(&mut self, coefficient: f32)` |
| Params | `&mut self, coefficient: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().set_coefficient(1.0);
        let _ = value;
    }
});
```

### `apply_force_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force_3d(&mut self, body_id: NodeID, force: Vector3) -> bool` |
| Params | `&mut self, body_id: NodeID, force: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().apply_force_3d(ctx.id, Vector3::new(0.0, 0.0, 0.0));
        let _ = value;
    }
});
```

### `apply_impulse_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse_2d(&mut self, body_id: NodeID, impulse: Vector2) -> bool` |
| Params | `&mut self, body_id: NodeID, impulse: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().apply_impulse_2d(ctx.id, Vector2::new(0.0, 0.0));
        let _ = value;
    }
});
```

### `apply_impulse_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse_3d(&mut self, body_id: NodeID, impulse: Vector3) -> bool` |
| Params | `&mut self, body_id: NodeID, impulse: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().apply_impulse_3d(ctx.id, Vector3::new(0.0, 0.0, 0.0));
        let _ = value;
    }
});
```

### `emit_force_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn emit_force_2d(&mut self, emitter: PhysicsForceEmitter2D) -> bool` |
| Params | `&mut self, emitter: PhysicsForceEmitter2D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().emit_force_2d(0.1);
        let _ = value;
    }
});
```

### `emit_force_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn emit_force_3d(&mut self, emitter: PhysicsForceEmitter3D) -> bool` |
| Params | `&mut self, emitter: PhysicsForceEmitter3D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().emit_force_3d(0.1);
        let _ = value;
    }
});
```

### `apply_force`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_force<D>(&mut self, body_id: NodeID, force: D) -> bool where D: IntoImpulseDirection,` |
| Params | `&mut self, body_id: NodeID, force: D` |
| Returns | `bool where D: IntoImpulseDirection,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().apply_force(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `apply_impulse`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn apply_impulse<D>(&mut self, body_id: NodeID, impulse: D) -> bool where D: IntoImpulseDirection,` |
| Params | `&mut self, body_id: NodeID, impulse: D` |
| Returns | `bool where D: IntoImpulseDirection,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().apply_impulse(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `raycast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().raycast_3d(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0), 1.0);
        let _ = value;
    }
});
```

### `raycast_3d_with_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_with_areas( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().raycast_3d_with_areas(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0), 1.0);
        let _ = value;
    }
});
```

### `raycast_3d_without_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_without_areas( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().raycast_3d_without_areas(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0), 1.0);
        let _ = value;
    }
});
```

### `raycast_3d_filtered`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_3d_filtered( &mut self, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsRayHit3D>` |
| Params | `&mut self, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsRayHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().raycast_3d_filtered(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0), 1.0, 0.1);
        let _ = value;
    }
});
```

### `raycast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_2d( &mut self, origin: Vector2, direction: Vector2, max_distance: f32, ) -> Option<PhysicsRayHit2D>` |
| Params | `&mut self, origin: Vector2, direction: Vector2, max_distance: f32,` |
| Returns | `Option<PhysicsRayHit2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().raycast_2d(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0), 1.0);
        let _ = value;
    }
});
```

### `raycast_2d_filtered`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn raycast_2d_filtered( &mut self, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsRayHit2D>` |
| Params | `&mut self, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsRayHit2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().raycast_2d_filtered(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0), 1.0, 0.1);
        let _ = value;
    }
});
```

### `shape_cast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn shape_cast_2d( &mut self, shape: Shape2D, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsShapeHit2D>` |
| Params | `&mut self, shape: Shape2D, origin: Vector2, direction: Vector2, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsShapeHit2D>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().shape_cast_2d(Default::default(), Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0), 1.0, 0.1);
        let _ = value;
    }
});
```

### `shape_cast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn shape_cast_3d( &mut self, shape: Shape3D, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter, ) -> Option<PhysicsShapeHit3D>` |
| Params | `&mut self, shape: Shape3D, origin: Vector3, direction: Vector3, max_distance: f32, filter: PhysicsQueryFilter,` |
| Returns | `Option<PhysicsShapeHit3D>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().shape_cast_3d(Default::default(), Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0), 1.0, 0.1);
        let _ = value;
    }
});
```

### `contacts_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn contacts_2d(&mut self, body_id: NodeID) -> Vec<PhysicsContact2D>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Vec<PhysicsContact2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().contacts_2d(ctx.id);
        let _ = value;
    }
});
```

### `contacts_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn contacts_3d(&mut self, body_id: NodeID) -> Vec<PhysicsContact3D>` |
| Params | `&mut self, body_id: NodeID` |
| Returns | `Vec<PhysicsContact3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().contacts_3d(ctx.id);
        let _ = value;
    }
});
```

### `solve_velocity_to_target_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_velocity_to_target_2d( &mut self, origin: Vector2, target: Vector2, time: f32, drift: Vector2, ) -> Option<Vector2>` |
| Params | `&mut self, origin: Vector2, target: Vector2, time: f32, drift: Vector2,` |
| Returns | `Option<Vector2>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().solve_velocity_to_target_2d(Vector2::new(0.0, 0.0), ctx.id, 1.0, Vector2::new(0.0, 0.0));
        let _ = value;
    }
});
```

### `solve_velocity_to_target_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_velocity_to_target_3d( &mut self, origin: Vector3, target: Vector3, time: f32, drift: Vector3, ) -> Option<Vector3>` |
| Params | `&mut self, origin: Vector3, target: Vector3, time: f32, drift: Vector3,` |
| Returns | `Option<Vector3>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().solve_velocity_to_target_3d(Vector3::new(0.0, 0.0, 0.0), ctx.id, 1.0, Vector3::new(0.0, 0.0, 0.0));
        let _ = value;
    }
});
```

### `solve_launch_velocity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_launch_velocity_2d( &mut self, origin: Vector2, target: Vector2, speed: f32, max_time: f32, drift: Vector2, ) -> Option<PhysicsLaunchSolution2D>` |
| Params | `&mut self, origin: Vector2, target: Vector2, speed: f32, max_time: f32, drift: Vector2,` |
| Returns | `Option<PhysicsLaunchSolution2D>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().solve_launch_velocity_2d(Vector2::new(0.0, 0.0), ctx.id, 1.0, 1.0, Vector2::new(0.0, 0.0));
        let _ = value;
    }
});
```

### `solve_launch_velocity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn solve_launch_velocity_3d( &mut self, origin: Vector3, target: Vector3, speed: f32, max_time: f32, drift: Vector3, ) -> Option<PhysicsLaunchSolution3D>` |
| Params | `&mut self, origin: Vector3, target: Vector3, speed: f32, max_time: f32, drift: Vector3,` |
| Returns | `Option<PhysicsLaunchSolution3D>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().solve_launch_velocity_3d(Vector3::new(0.0, 0.0, 0.0), ctx.id, 1.0, 1.0, Vector3::new(0.0, 0.0, 0.0));
        let _ = value;
    }
});
```

### `predict_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn predict_body_2d( &mut self, body_id: NodeID, time: f32, drift: Vector2, ) -> Option<PhysicsBodyPrediction2D>` |
| Params | `&mut self, body_id: NodeID, time: f32, drift: Vector2,` |
| Returns | `Option<PhysicsBodyPrediction2D>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().predict_body_2d(ctx.id, 1.0, Vector2::new(0.0, 0.0));
        let _ = value;
    }
});
```

### `predict_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn predict_body_3d( &mut self, body_id: NodeID, time: f32, drift: Vector3, ) -> Option<PhysicsBodyPrediction3D>` |
| Params | `&mut self, body_id: NodeID, time: f32, drift: Vector3,` |
| Returns | `Option<PhysicsBodyPrediction3D>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().predict_body_3d(ctx.id, 1.0, Vector3::new(0.0, 0.0, 0.0));
        let _ = value;
    }
});
```

### `pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn pause(&mut self, paused: bool)` |
| Params | `&mut self, paused: bool` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().pause(true);
        let _ = value;
    }
});
```

### `is_paused`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `pub fn is_paused(&mut self) -> bool` |
| Params | `&mut self` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Physics().is_paused();
        let _ = value;
    }
});
```

### `apply_force`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `apply_force!(ctx.run, body_id, force)` |
| Params | `ctx, body_id, force` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = apply_force!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `physics_get_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_gravity!(ctx.run)` |
| Params | `ctx` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_get_gravity!(ctx.run);
        let _ = value;
    }
});
```

### `physics_set_gravity`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_gravity!(ctx.run, gravity)` |
| Params | `ctx, gravity` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_set_gravity!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `physics_get_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_get_coefficient!(ctx.run)` |
| Params | `ctx` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_get_coefficient!(ctx.run);
        let _ = value;
    }
});
```

### `physics_set_coefficient`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_set_coefficient!(ctx.run, coefficient)` |
| Params | `ctx, coefficient` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_set_coefficient!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `physics_solve_velocity_to_target_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_velocity_to_target_2d!(ctx.run, origin, target, time)` |
| Params | `ctx, origin, target, time` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_solve_velocity_to_target_2d!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_solve_velocity_to_target_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_velocity_to_target_3d!(ctx.run, origin, target, time)` |
| Params | `ctx, origin, target, time` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_solve_velocity_to_target_3d!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_solve_launch_velocity_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_launch_velocity_2d!(ctx.run, origin, target, speed, max_time)` |
| Params | `ctx, origin, target, speed, max_time` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_solve_launch_velocity_2d!(ctx.run, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `physics_solve_launch_velocity_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_solve_launch_velocity_3d!(ctx.run, origin, target, speed, max_time)` |
| Params | `ctx, origin, target, speed, max_time` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_solve_launch_velocity_3d!(ctx.run, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `physics_predict_body_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_predict_body_2d!(ctx.run, body_id, time)` |
| Params | `ctx, body_id, time` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_predict_body_2d!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `physics_predict_body_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_predict_body_3d!(ctx.run, body_id, time)` |
| Params | `ctx, body_id, time` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_predict_body_3d!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `apply_impulse`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `apply_impulse!(ctx.run, body_id, impulse)` |
| Params | `ctx, body_id, impulse` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = apply_impulse!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `physics_raycast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_raycast_3d!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_raycast_3d_with_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d_with_areas!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_raycast_3d_with_areas!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_raycast_3d_without_areas`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_3d_without_areas!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_raycast_3d_without_areas!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_raycast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_raycast_2d!(ctx.run, origin, direction, max_distance)` |
| Params | `ctx, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_raycast_2d!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_shape_cast_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_shape_cast_2d!(ctx.run, shape, origin, direction, max_distance, filter)` |
| Params | `ctx, shape, origin, direction, max_distance, filter` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_shape_cast_2d!(ctx.run, 0.0, 0.1, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_shape_cast_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_shape_cast_3d!(ctx.run, shape, origin, direction, max_distance, filter)` |
| Params | `ctx, shape, origin, direction, max_distance, filter` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_shape_cast_3d!(ctx.run, 0.0, 0.1, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `physics_contacts_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_contacts_2d!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_contacts_2d!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `physics_contacts_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_contacts_3d!(ctx.run, body_id)` |
| Params | `ctx, body_id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_contacts_3d!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `physics_pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_pause!(ctx.run, paused)` |
| Params | `ctx, paused` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_pause!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `physics_is_paused`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Physics()` |
| Signature | `physics_is_paused!(ctx.run)` |
| Params | `ctx` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = physics_is_paused!(ctx.run);
        let _ = value;
    }
});
```
