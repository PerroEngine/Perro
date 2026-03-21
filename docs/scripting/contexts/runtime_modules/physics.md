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
