# Physics Nodes

Physics bodies and shapes are separate scene nodes.
`StaticBody2D`, `RigidBody2D`, `CharacterBody2D`, `Area2D`, `StaticBody3D`, `RigidBody3D`, `CharacterBody3D`, and `Area3D` hold body/area behavior.
`CollisionShape2D` and `CollisionShape3D` hold geometry.

In scene files, put collision shapes in separate top-level node blocks.
Set each shape `parent` to the body or area node key.

Inner type blocks are inheritance data, not children.
`[RigidBody3D] ... [Node3D] ... [/Node3D]` means `RigidBody3D` inherits `Node3D` fields.
It does not create a `Node3D` child.

## 2D Body Shape

```text
[Body]
parent = $root
    [RigidBody2D]
        collision_layers = [1]
        collision_mask = []
        [Node2D/]
    [/RigidBody2D]
[/Body]

[BodyShape]
parent = @Body
    [CollisionShape2D]
        shape = { type = quad width = 1.0 height = 1.0 }
    [/CollisionShape2D]
[/BodyShape]
```

## 3D Body Shape

```text
[Body]
parent = $root
    [StaticBody3D]
        collision_layers = [1]
        collision_mask = []
        [Node3D/]
    [/StaticBody3D]
[/Body]

[BodyShape]
parent = @Body
    [CollisionShape3D]
        shape = { type = cube, size = (1, 1, 1) }
    [/CollisionShape3D]
[/BodyShape]
```

## Character Body

`CharacterBody2D` / `CharacterBody3D` are script-driven bodies.
They are not dynamic: no velocity, no forces, no impulses, no physics write-back.
The engine only:

- applies gravity each fixed step with a collision sweep (`apply_gravity`, `gravity_scale`, `max_fall_speed`)
- blocks the gravity sweep on static and rigid bodies (no tunneling into floors)

Move them from scripts by setting the transform, or use the physics move API for a collide-and-stop move.
Fields: `enabled`, `collision_layers`, `collision_mask`, `apply_gravity`, `gravity_scale`, `max_fall_speed`, `friction`, `restitution`, `density`.

```text
[Player]
parent = $root
    [CharacterBody3D]
        collision_layers = [1]
        apply_gravity = true
        [Node3D/]
    [/CharacterBody3D]
[/Player]

[PlayerShape]
parent = @Player
    [CollisionShape3D]
        shape = { type = capsule, radius = 0.4, half_height = 0.6 }
    [/CollisionShape3D]
[/PlayerShape]
```

## Notes

- Areas also need child collision shapes for overlap/query volume.
- Bodies and areas can have more than one child collision shape.
- Shape local transform comes from the shape node's `Node2D` or `Node3D` data.
- Collision shapes only provide geometry.
