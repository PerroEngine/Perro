# Physics Module

Purpose:

- Apply directional impulses to rigidbodies through `NodeID`.
- Use one API for both one-shot impulses and sustained acceleration by controlling call frequency.

Impulse macro:

- `apply_force!(ctx, body_id, direction, amount) -> bool`

Arguments:

- `ctx`: `&mut RuntimeContext<_>`
- `body_id`: `NodeID` of a `RigidBody2D` or `RigidBody3D`
- `direction`: `Vector2` for 2D bodies, `Vector3` for 3D bodies
- `amount`: scalar magnitude

Behavior:

- The engine normalizes `direction` and computes `impulse = direction * amount`.
- Returns `false` if `body_id` is invalid or not a rigidbody of the matching dimension.
- Calls are queued and applied in fixed-step physics before the world simulation step.
- Call once for burst/knockback behavior; call repeatedly (for example every update/fixed-update) for constant behavior.

Example:

```rust
if ipt.Keys().is_pressed(KeyCode::W) {
    apply_force!(ctx, player_body_id, Vector3::new(0.0, 0.0, -1.0), 0.35);
}
```

Collision signals:

- On first contact between two bodies, runtime emits a global signal per body:
  - `"{BodyNodeName}_Collision"`
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
