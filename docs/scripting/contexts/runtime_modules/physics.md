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

Raycast macros:

- `physics_raycast_2d!(ctx, origin, direction, max_distance) -> Option<PhysicsRayHit2D>`
- `physics_raycast_2d!(ctx, origin, direction, max_distance, filter) -> Option<PhysicsRayHit2D>`
- `physics_raycast_3d!(ctx, origin, direction, max_distance) -> Option<PhysicsRayHit3D>`
- `physics_raycast_3d_with_areas!(ctx, origin, direction, max_distance) -> Option<PhysicsRayHit3D>`
- `physics_raycast_3d_without_areas!(ctx, origin, direction, max_distance) -> Option<PhysicsRayHit3D>`

Shape/contact macros:

- `physics_shape_cast_2d!(ctx, shape, origin, direction, max_distance, filter) -> Option<PhysicsShapeHit2D>`
- `physics_shape_cast_3d!(ctx, shape, origin, direction, max_distance, filter) -> Option<PhysicsShapeHit3D>`
- `physics_contacts_2d!(ctx, body_id) -> Vec<PhysicsContact2D>`
- `physics_contacts_3d!(ctx, body_id) -> Vec<PhysicsContact3D>`

Arguments:

- `ctx`: `&mut RuntimeWindow<_>`
- `body_id`: `NodeID` of a `RigidBody2D` or `RigidBody3D`
- `force`/`impulse`: `Vector2` for 2D bodies, `Vector3` for 3D bodies
- `origin`/`direction`: `Vector2` or `Vector3` query data in world space
- `max_distance`: maximum query distance
- `filter`: `PhysicsQueryFilter` for layer masks, area inclusion, and excluded nodes

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
- `physics_raycast_3d!` hits `StaticBody3D`, `RigidBody3D`, and `Area3D` colliders.
- `physics_raycast_3d_with_areas!` is an explicit alias for area-inclusive raycasts.
- `physics_raycast_3d_without_areas!` skips `Area3D` sensor colliders.
- `physics_raycast_2d!` hits `StaticBody2D`, `RigidBody2D`, `Area2D`, and `TileMap2D` colliders.
- Raycast returns `None` for invalid direction, non-positive distance, missing world, or no hit.
- Shape casts use `Shape2D` or primitive `Shape3D` as the moving shape.
- Shape casts return the first hit along the direction.
- 3D `Shape3D::TriMesh` cannot be used as the moving cast shape.
- Contact queries return current active contact points for one body.

## Collision Layers And Masks

2D and 3D body/area nodes expose:

- `collision_layers: BitMask`
- `collision_mask: BitMask`

Default values:

- `collision_layers = [1]`
- `collision_mask_layers = [1, 2, 3, ...]`

Layer/mask behavior:

- A collider belongs to `collision_layers`.
- A collider checks against other layers through `collision_mask`.
- Contacts and area overlaps use layer/mask rules.

Queries use `PhysicsQueryFilter`:

```rust
PhysicsQueryFilter {
    mask: BitMask::ALL,
    include_areas: true,
    exclude_nodes: Vec::new(),
}
```

Use `BitMask::with([1, 2])` in Rust code.
Use `collision_layers = [1, 2]` and `collision_mask_layers = [1, 2]` in scene files.
See [BitMask](../../bitmask.md).

Contact hit data:

- `node`: other body node.
- `point`: world-space contact point.
- `normal`: world-space normal.
- `impulse`: solver impulse when available.

## Joint Nodes

2D:

- `PinJoint2D`
- `DistanceJoint2D`
- `FixedJoint2D`

3D:

- `BallJoint3D`
- `HingeJoint3D`
- `FixedJoint3D`

Common fields:

- `body_a`
- `body_b`
- `anchor_a`
- `anchor_b`
- `enabled`
- `collide_connected`

Extra fields:

- `HingeJoint3D.axis`
- `DistanceJoint2D.min_distance`
- `DistanceJoint2D.max_distance`

Joint nodes sync during fixed step after bodies sync and before the physics world step.
If either body is missing, the joint is skipped.
`body_a` and `body_b` accept scene node refs like `@BodyName` in scene files.
Anchors are local to each connected body.
`collide_connected = false` disables contacts between connected bodies.
`DistanceJoint2D` enforces both `min_distance` and `max_distance`.

`PhysicsRayHit2D` fields:

- `node`: hit body `NodeID`
- `point`: world-space hit point
- `normal`: world-space hit normal
- `distance`: distance from ray origin

`PhysicsRayHit3D` fields:

- `node`: hit body `NodeID`
- `point`: world-space hit point
- `normal`: world-space hit normal
- `distance`: distance from ray origin

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

if let Some(hit) = physics_raycast_3d!(
    ctx,
    Vector3::new(0.0, 2.0, -5.0),
    Vector3::new(0.0, -0.2, 1.0),
    25.0
) {
    log::info!("hit {:?} at {:?}", hit.node, hit.point);
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

