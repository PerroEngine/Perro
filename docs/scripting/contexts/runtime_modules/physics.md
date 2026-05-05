# Physics Module

Purpose:

- Apply directional forces or impulses to rigidbodies through `NodeID`.

Force macro:

- `apply_force!(ctx, body_id, force) -> bool`

Impulse macro:

- `apply_impulse!(ctx, body_id, impulse) -> bool`

Pause macros:

- `physics_pause!(ctx, paused)`
- `physics_is_paused!(ctx) -> bool`

Arguments:

- `ctx`: `&mut RuntimeWindow<_>`
- `body_id`: `NodeID` of a `RigidBody2D` or `RigidBody3D`
- `force`/`impulse`: `Vector2` for 2D bodies, `Vector3` for 3D bodies

Behavior:

- Force: integrates with fixed-step `dt` using `impulse = force * fixed_dt`.
- Impulse: applies the `impulse` vector immediately.
- Returns `false` if `body_id` is invalid or not a rigidbody node of the matching dimension.
- Calls are queued and applied in fixed-step physics before the world simulation step.
- Use `apply_impulse!` for one-shot burst/knockback.
- Use repeated `apply_force!` calls (for example every fixed-update) for sustained acceleration.
- `physics_pause!(ctx, true)` pauses physics simulation step.
- While paused, gravity/velocity integration + collision/area signal propagation do not advance.
- `physics_pause!(ctx, false)` resumes from current physics world state.
- Queued force/impulse calls made during pause stay queued and apply after resume.

Example:

```rust
if ipt.Keys().is_pressed(KeyCode::W) {
    apply_force!(ctx, player_body_id, Vector3::new(0.0, 0.0, -0.35));
}

if take_hit {
    apply_impulse!(ctx, player_body_id, Vector3::new(2.0, 0.0, 0.0));
}

if menu_open {
    physics_pause!(ctx, true);
}
if menu_closed {
    physics_pause!(ctx, false);
}
```

Collision signals:

- On first contact between two bodies, runtime emits a global signal per body:
  - `"{BodyNodeName}_Collided"`
- Signal params:
  - `params[0]`: source body `NodeID`
  - `params[1]`: other body `NodeID`
- Emitted for `RigidBody2D/StaticBody2D` and `RigidBody3D/StaticBody3D` contacts.

Area signals:

- Areas emit overlap lifecycle signals using exact action suffixes:
  - `"{AreaNodeName}_Entered"`
  - `"{AreaNodeName}_Occupied"`
  - `"{AreaNodeName}_Exited"`
- Emitted for `Area2D` and `Area3D` when their overlap set changes.
- Signal params:
  - `params[0]`: area `NodeID`
  - `params[1]`: other overlapped body `NodeID`

