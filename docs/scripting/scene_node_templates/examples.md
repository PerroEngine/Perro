# Extra `.scn` Node Examples

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use this page when editing `.scn` scene files.

Use [Node Collections](../node_collections.md) for runtime Rust-built node trees.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# Extra `.scn` Node Examples

[Back to index](index.md)

## Parent And Root

Every scene needs one root node.
Set it with `$root = @NodeKey`.

Every non-root node needs `parent`.
Use `$root` for root children or `@OtherNode` for any other parent.

```text
$root = @Level

[Level]
    [Node2D/]
[/Level]

[Player]
parent = $root
script = "res://scripts/player.rs"
    [Node2D]
        position = (0, 0)
    [/Node2D]
[/Player]

[Muzzle]
parent = @Player
    [Node2D]
        position = (12, 0)
    [/Node2D]
[/Muzzle]
```

Parent sets transform inheritance.
`Muzzle` moves with `Player`.

## Security Camera Stream

```text
[SecurityCamera]
parent = $root
    [Camera3D]
        active = false
        post_processing = [
            { type = "bloom", intensity = 0.15 }
        ]
        [Node3D]
            position = (0, 3, -6)
            rotation_deg = (-15, 0, 0)
        [/Node3D]
    [/Camera3D]
[/SecurityCamera]

[Monitor]
parent = $root
    [CameraStream3D]
        camera = @SecurityCamera
        resolution = (640, 360)
        aspect_mode = "fit"
        size = (1.6, 0.9)
        post_processing = [
            { type = "crt", scanline_strength = 0.35, vignette = 0.2 }
        ]
        [Node3D]
            position = (2, 1.2, -2)
        [/Node3D]
    [/CameraStream3D]
[/Monitor]
```

Camera post-processing runs first.
Stream post-processing runs after.

## Script Vars

`script_vars` seeds the attached script state when the node is created.
Keys must match fields in the script `#[State]` struct.

Scene:

```text
[Player]
parent = $root
script = "res://scripts/player.rs"
script_vars = {
    speed = 8.0
    health = 120
    target = @Enemy
}
    [Node2D/]
[/Player]

[Enemy]
parent = $root
    [Node2D/]
[/Enemy]
```

Script:

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default = 6.0]
    pub speed: f32,
    #[default = 100]
    pub health: i32,
    pub target: Option<NodeID>,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state!(ctx, PlayerState, |state| {
            // scene script_vars already applied here
            let _speed = state.speed;
            let _target = state.target;
        });
    }
});
```

Values omitted from `script_vars` use `#[default = ...]` or type default.
Node refs use `@NodeKey`.

## Animation Bindings

`.panim` files store object names.
Scene `bindings` map those object names to scene nodes.

If animation tracks target object `Hero`, bind `Hero = @PlayerRoot`.

```text
[PlayerRoot]
parent = $root
    [Node2D/]
[/PlayerRoot]

[PlayerAnim]
parent = @PlayerRoot
    [AnimationPlayer]
        animation = "res://animations/player_idle.panim"
        bindings = { Hero = @PlayerRoot }
        speed = 1.0
        paused = false
        playback = loop
    [/AnimationPlayer]
[/PlayerAnim]
```

`AnimationTree` uses per-clip bindings.
Each entry can bind same object name to same or different nodes.

```text
[PlayerTree]
parent = @PlayerRoot
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang, speed = 1.0, paused = false }
        ]
        speed = 1.0
        paused = false
    [/AnimationTree]
[/PlayerTree]
```

Binding side rule:
left side = animation object name  
right side = scene node ref

## Render Layers

`render_mask` belongs to cameras.
`render_layers` belongs to renderable nodes.
Camera mask hides node layers when they intersect.
Default camera mask is no layers.
Default node render layers is all layers.
Add layers to a camera mask to hide them.

2D example:

```text
$root = @Scene2D

[Scene2D]
    [Node2D/]
[/Scene2D]

[GameplayCamera]
parent = $root
    [Camera2D]
        active = true
        render_mask = [2]
    [/Camera2D]
[/GameplayCamera]

[PlayerSprite]
parent = $root
    [Sprite2D]
        texture = "res://textures/player.png"
        [Node2D]
            render_layers = [1]
        [/Node2D]
    [/Sprite2D]
[/PlayerSprite]

[EditorOnlySprite]
parent = $root
    [Sprite2D]
        texture = "res://textures/gizmo.png"
        [Node2D]
            render_layers = [2]
        [/Node2D]
    [/Sprite2D]
[/EditorOnlySprite]
```

`GameplayCamera` sees `PlayerSprite`.
It skips `EditorOnlySprite`.

3D example:

```text
$root = @Scene3D

[Scene3D]
    [Node3D/]
[/Scene3D]

[MainCamera]
parent = $root
    [Camera3D]
        active = true
        render_mask = [2]
    [/Camera3D]
[/MainCamera]

[LevelMesh]
parent = $root
    [MeshInstance3D]
        mesh = "res://models/level.glb:mesh[0]"
        material = "res://materials/level.pmat"
        [Node3D]
            render_layers = [1]
        [/Node3D]
    [/MeshInstance3D]
[/LevelMesh]

[ReflectionOnlyMesh]
parent = $root
    [MeshInstance3D]
        mesh = "res://models/reflection_proxy.glb:mesh[0]"
        material = "res://materials/proxy.pmat"
        [Node3D]
            render_layers = [2]
        [/Node3D]
    [/MeshInstance3D]
[/ReflectionOnlyMesh]
```

`MainCamera` sees layer 1 and 3.
It skips layer 2.

## Physics Parity Templates

Current body layer/mask fields:

`collision_layers` tags a body/area.
`collision_mask` says which tagged layers it ignores.
Default body/area layers is all layers.
Default body/area mask is no layers.
Collision requires neither side to ignore the other.
Empty mask (`[]`) means ignore nothing.

```text
$root = @Physics2D

[Physics2D]
    [Node2D/]
[/Physics2D]

[Body]
parent = $root
    [RigidBody2D]
        collision_layers = [1]
        collision_mask = []
    [/RigidBody2D]
[/Body]

[BodyShape]
parent = @Body
    [CollisionShape2D]
        shape = { type = "quad" width = 1 height = 1 }
    [/CollisionShape2D]
[/BodyShape]
```

2D joints:

```text
[AnchorBody]
parent = $root
    [StaticBody2D]
        collision_layers = [1]
        collision_mask = []
    [/StaticBody2D]
[/AnchorBody]

[AnchorBodyShape]
parent = @AnchorBody
    [CollisionShape2D]
        shape = { type = "quad" width = 1 height = 1 }
    [/CollisionShape2D]
[/AnchorBodyShape]

[SwingBody]
parent = $root
    [RigidBody2D]
        collision_layers = [1]
        collision_mask = []
    [/RigidBody2D]
[/SwingBody]

[SwingBodyShape]
parent = @SwingBody
    [CollisionShape2D]
        shape = { type = "quad" width = 1 height = 1 }
    [/CollisionShape2D]
[/SwingBodyShape]

[rope_pin]
parent = $root
    [PinJoint2D]
        body_a = @AnchorBody
        body_b = @SwingBody
        anchor_a = (0, 0)
        anchor_b = (0, 0.5)
        enabled = true
        collide_connected = false
    [/PinJoint2D]
[/rope_pin]

[distance_link]
parent = $root
    [DistanceJoint2D]
        body_a = @AnchorBody
        body_b = @SwingBody
        anchor_a = (0, 0)
        anchor_b = (0, 0)
        min_distance = 0
        max_distance = 2
        enabled = true
        collide_connected = false
    [/DistanceJoint2D]
[/distance_link]

[fixed_link_2d]
parent = $root
    [FixedJoint2D]
        body_a = @AnchorBody
        body_b = @SwingBody
        anchor_a = (0, 0)
        anchor_b = (0, 0)
        enabled = true
        collide_connected = false
    [/FixedJoint2D]
[/fixed_link_2d]
```

3D joints:

```text
$root = @Physics3D

[Physics3D]
    [Node3D/]
[/Physics3D]

[FrameBody]
parent = $root
    [StaticBody3D]
        collision_layers = [1]
        collision_mask = []
    [/StaticBody3D]
[/FrameBody]

[FrameBodyShape]
parent = @FrameBody
    [CollisionShape3D]
        shape = { type = cube, size = (1, 1, 1) }
    [/CollisionShape3D]
[/FrameBodyShape]

[DoorBody]
parent = $root
    [RigidBody3D]
        collision_layers = [1]
        collision_mask = []
    [/RigidBody3D]
[/DoorBody]

[DoorBodyShape]
parent = @DoorBody
    [CollisionShape3D]
        shape = { type = cube, size = (1, 2, 0.2) }
    [/CollisionShape3D]
[/DoorBodyShape]

[ball_socket]
parent = $root
    [BallJoint3D]
        body_a = @FrameBody
        body_b = @DoorBody
        anchor_a = (0, 1, 0)
        anchor_b = (-0.5, 1, 0)
        enabled = true
        collide_connected = false
    [/BallJoint3D]
[/ball_socket]

[door_hinge]
parent = $root
    [HingeJoint3D]
        body_a = @FrameBody
        body_b = @DoorBody
        anchor_a = (0, 1, 0)
        anchor_b = (-0.5, 1, 0)
        axis = (0, 1, 0)
        enabled = true
        collide_connected = false
    [/HingeJoint3D]
[/door_hinge]

[fixed_link_3d]
parent = $root
    [FixedJoint3D]
        body_a = @FrameBody
        body_b = @DoorBody
        anchor_a = (0, 0, 0)
        anchor_b = (0, 0, 0)
        enabled = true
        collide_connected = false
    [/FixedJoint3D]
[/fixed_link_3d]
```
