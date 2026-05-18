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

World config macros:

- `physics_get_gravity!(ctx) -> f32`
- `physics_set_gravity!(ctx, gravity)`
- `physics_get_coefficient!(ctx) -> f32`
- `physics_set_coefficient!(ctx, coefficient)`

Trajectory solver macros:

- `physics_solve_velocity_to_target_2d!(ctx, origin, target, time) -> Option<Vector2>`
- `physics_solve_velocity_to_target_2d!(ctx, origin, target, time, drift) -> Option<Vector2>`
- `physics_solve_velocity_to_target_3d!(ctx, origin, target, time) -> Option<Vector3>`
- `physics_solve_velocity_to_target_3d!(ctx, origin, target, time, drift) -> Option<Vector3>`
- `physics_solve_launch_velocity_2d!(ctx, origin, target, speed, max_time) -> Option<PhysicsLaunchSolution2D>`
- `physics_solve_launch_velocity_2d!(ctx, origin, target, speed, max_time, drift) -> Option<PhysicsLaunchSolution2D>`
- `physics_solve_launch_velocity_3d!(ctx, origin, target, speed, max_time) -> Option<PhysicsLaunchSolution3D>`
- `physics_solve_launch_velocity_3d!(ctx, origin, target, speed, max_time, drift) -> Option<PhysicsLaunchSolution3D>`
- `physics_predict_body_2d!(ctx, body_id, time) -> Option<PhysicsBodyPrediction2D>`
- `physics_predict_body_2d!(ctx, body_id, time, drift) -> Option<PhysicsBodyPrediction2D>`
- `physics_predict_body_3d!(ctx, body_id, time) -> Option<PhysicsBodyPrediction3D>`
- `physics_predict_body_3d!(ctx, body_id, time, drift) -> Option<PhysicsBodyPrediction3D>`

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
- Runtime gravity and coefficient changes override `project.toml` values.
- Coefficient must be finite and greater than zero.
- Trajectory solvers use effective gravity: `physics_get_gravity!(ctx) * physics_get_coefficient!(ctx)`.
- Solver model: `target = origin + (velocity + drift) * time + 0.5 * gravity * time * time`.
- Drift is constant velocity from wind, water flow, or gameplay current.
- Omitted drift uses zero vector.
- Fixed-time solvers return the needed velocity vector.
- Fixed-speed solvers return `low` and `high` launch arcs.
- Fixed-speed solvers use a fast analytic path when drift is omitted or zero.
- Fixed-speed solvers with drift scan forward in time to find valid arcs.
- Fixed-speed solvers return `None` when target is unreachable before `max_time`.
- Solvers return `None` for invalid inputs, non-positive time/speed/max-time, or same origin and target.
- Body prediction reads current rigidbody transform, linear/angular velocity, and gravity scale.
- Body prediction does not mutate the runtime or step the physics world.
- Body prediction ignores collisions, joints, damping, sleeping, and queued forces/impulses.
- Body prediction returns predicted `position`, `rotation`, `velocity`, and `angular_velocity`.
- `physics_raycast_3d!` hits `StaticBody3D`, `RigidBody3D`, and `Area3D` colliders.
- `physics_raycast_3d_with_areas!` is an explicit alias for area-inclusive raycasts.
- `physics_raycast_3d_without_areas!` skips `Area3D` sensor colliders.
- `physics_raycast_2d!` hits `StaticBody2D`, `RigidBody2D`, `Area2D`, and `TileMap2D` colliders.
- Raycast returns `None` for invalid direction, non-positive distance, missing world, or no hit.
- Shape casts use `Shape2D` or primitive `Shape3D` as the moving shape.
- Shape casts return the first hit along the direction.
- 3D `Shape3D::TriMesh` cannot be used as the moving cast shape.
- Contact queries return current active contact points for one body.

## Physics Force Emitters

`PhysicsForceEmitter2D` and `PhysicsForceEmitter3D` apply radius-based force fields during fixed physics.

Fields:

- `enabled`
- `profile`: `"lift"`, `"explosion"`, `"current"`, `"vortex"`, or `"custom"`
- `radius`
- `strength`
- `duration`
- `pulse`
- `falloff`
- `affect_bodies`
- `affect_water`
- `collision_layers`
- `collision_mask`
- `vectors`

Custom profile:

```text
[LiftPad]
    [PhysicsForceEmitter2D]
        profile = "custom"
        radius = 8
        strength = 1
        vectors = [(0, 20), (4, 15), (8, 0)]
        [Node2D]
            position = (0, 0)
        [/Node2D]
    [/PhysicsForceEmitter2D]
[/LiftPad]
```

`vectors` stores force vectors.
Runtime samples the array by normalized distance across `radius` and interpolates between entries.
`strength` multiplies the sampled vector.

Presets:

- `lift`: world-up force.
- `explosion`: outward impulse.
- `current`: first vector as steady directional force.
- `vortex`: tangent force plus small inward pull.
- `custom`: sampled vector array.

Water:

- Any emitter with `affect_water = true` sends its force event to nearby water.
- Water converts force strength into wake/foam and cavitation.
- There is no separate underwater explosion preset.

## Collision Layers And Masks

2D and 3D body/area nodes expose:

- `collision_layers: BitMask`
- `collision_mask: BitMask`

Default values:

- `collision_layers = [1, 2, 3, ...]`
- `collision_mask = []`

Layer/mask behavior:

- A collider is tagged with `collision_layers`.
- A collider ignores other tagged layers through `collision_mask`.
- Collision requires neither side to mask the other:
  - A mask does not intersect B layers.
  - B mask does not intersect A layers.
- Contacts and area overlaps use layer/mask rules.
- `collision_mask = []` means ignore nothing.
- `collision_layers = []` means belong to no layer.
- Put all colliders on default layers and leave masks default for normal everything-collides behavior.
- Use a narrow mask to ignore same-team bodies, sensors, editor-only physics, or query-only layers.

Queries use `PhysicsQueryFilter`:

```rust
PhysicsQueryFilter {
    mask: BitMask::ALL,
    include_areas: true,
    exclude_nodes: Vec::new(),
}
```

Use `BitMask::with([1, 2])` in Rust code.
Use `collision_layers = [1, 2]` and `collision_mask = [3]` in scene files.
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

if let Some(velocity) = physics_solve_velocity_to_target_3d!(
    ctx,
    cannon_pos,
    target_pos,
    1.2,
    Vector3::new(0.8, 0.0, 0.0)
) {
    apply_impulse!(ctx, cannon_ball_id, velocity * mass);
}

if let Some(arcs) = physics_solve_launch_velocity_3d!(ctx, origin, target, 22.0, 4.0) {
    let grenade_velocity = arcs.high;
    apply_impulse!(ctx, grenade_id, grenade_velocity * mass);
}

if let Some(predicted) = physics_predict_body_3d!(ctx, ball_id, 0.75) {
    let expected_ball_pos = predicted.position;
    let expected_ball_rot = predicted.rotation;
    let expected_ball_velocity = predicted.velocity;
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

