# Physics Nodes

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| 2D Body Shape | [2D Body Shape](#2d-body-shape) |
| 3D Body Shape | [3D Body Shape](#3d-body-shape) |
| Rigid Body Gravity Scale | [Rigid Body Gravity Scale](#rigid-body-gravity-scale) |
| Character Body | [Character Body](#character-body) |
| Player Movement | [Player Movement](#player-movement) |
| Notes | [Notes](#notes) |

## Purpose

Perro splits physics into two node kinds: body/area nodes carry the behavior (`StaticBody`, `RigidBody`, `CharacterBody`, `Area`), and `CollisionShape` nodes carry the geometry. This page shows how to wire the two together in scenes and drive them from scripts, so you get dynamic props, script-controlled characters, trigger volumes, and world queries. Bodies and shapes exist in both 2D and 3D variants.

## Use Cases

- A prop that reacts to gravity, forces, and collisions (a rolling boulder, a stack of crates): `RigidBody3D` / `RigidBody2D` with a child `CollisionShape`, driven by `apply_force!` / `apply_impulse!` and tuned with `gravity_scale`.
- A script-controlled player or NPC that never tunnels through walls: `CharacterBody3D` / `CharacterBody2D` moved with `physics_move_and_slide_3d!` (slides along walls) or `physics_move_body_3d!` (collide and stop).
- Jumping and custom gravity: keep `y_vel` in `#[State]`, ground-check with `physics_contacts_3d!`, or let `physics_apply_gravity_3d!` handle falling for you.
- Trigger volumes — pickups, damage zones, checkpoints: `Area2D` / `Area3D` with a child shape, reacting to their overlap signals.
- Immovable level geometry (floors, walls, platforms): `StaticBody2D` / `StaticBody3D` with a `CollisionShape`.
- World queries for AI and weapons — line of sight, ground checks, projectile arcs: `physics_raycast_3d!`, `physics_shape_cast_3d!`, `physics_predict_body_3d!`.

Physics bodies and shapes are separate scene nodes.
`StaticBody2D`, `RigidBody2D`, `CharacterBody2D`, `Area2D`, `StaticBody3D`, `RigidBody3D`, `CharacterBody3D`, and `Area3D` hold body/area behavior.
`CollisionShape2D` and `CollisionShape3D` hold geometry.

In scene files, put collision shapes in separate top-level node blocks.
Set each shape `parent` to the body or area node key.

Inner type blocks are inheritance data, not children.
`[RigidBody3D] ... [Node3D] ... [/Node3D]` means `RigidBody3D` inherits `Node3D` fields.
It does not create a `Node3D` child.

## Ownership And Choice

The physics node owns motion mode, mass, layers, and velocity; child collision shapes own geometry. A controller script reads input and asks its known body to move. Choose a character body for authored movement, a rigid body for force-driven motion, a static body for immovable collision, and an area for detection only. Do not write transforms around the physics step to imitate a body type; that bypasses its collision contract.

## 2D Body Shape

```text
[Body]
parent = $root
    [RigidBody2D]
        collision_layers = [1]
        collision_mask = []
        gravity_scale = 0.5
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
    [RigidBody3D]
        collision_layers = [1]
        collision_mask = []
        gravity_scale = 0.5
        [Node3D/]
    [/RigidBody3D]
[/Body]

[BodyShape]
parent = @Body
    [CollisionShape3D]
        shape = { type = cube, size = (1, 1, 1) }
    [/CollisionShape3D]
[/BodyShape]
```

## Rigid Body Gravity Scale

`RigidBody2D` and `RigidBody3D` use world gravity times local `gravity_scale`.
Default is `1.0`.
Set `0.5` for half gravity, `0.0` for no gravity, or a negative value to invert it.

Scene file:

```text
[Ball]
parent = $root
    [RigidBody3D]
        gravity_scale = 0.5
        [Node3D/]
    [/RigidBody3D]
[/Ball]
```

Script:

```rust
physics_set_body_gravity_scale!(ctx.run, body_id, 0.5);
let scale = physics_get_body_gravity_scale!(ctx.run, body_id);
```

## Character Body

`CharacterBody2D` / `CharacterBody3D` are fully script-driven bodies.
They are not dynamic: no velocity, no forces, no impulses, no gravity, no physics write-back.
The engine never moves them — it only reports collisions:

- `physics_move_body_*` / `physics_move_and_slide_*` sweep against static and rigid bodies (no tunneling)
- sweep hits feed `physics_contacts_*` and emit the `<Name>_Collided` signal
- raycasts and shape casts answer questions like "is there ground below me"

Move them from scripts by setting the transform, with the collide-and-stop move API, or with the slide API.
Fields: `enabled`, `collision_layers`, `collision_mask`, `friction`, `restitution`, `density`.

Gravity is opt-in and script-invoked. Two ways:

- `physics_apply_gravity_3d!(ctx.run, ctx.id, dt)` — engine helper. Integrates an internal fall speed from world gravity, sweeps the body down, resets the fall speed on landing. Call it each update; stop calling it and the body stops falling. Hooked into the world gravity setting (`physics_set_gravity`).
- Custom — keep a `y_vel` in script state, integrate your own gravity, and feed it into the motion you pass to `physics_move_and_slide_3d!`. Full control (jump arcs, variable gravity, water).

Do not mix both on the same body: the helper owns its own fall speed and knows nothing about your `y_vel`.

```text
[Player]
parent = $root
    [CharacterBody3D]
        collision_layers = [1]
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

## Player Movement

The body node id is `ctx.id` on the attached script.
Read input, build a motion vector, move with the slide sweep.
`physics_move_and_slide_3d!` sweeps, slides along hit planes (walls, floors), and writes the safe global position.
`physics_move_body_3d!` is the raw collide-and-stop variant (no sliding).
Never use `apply_force` / `apply_impulse`: character bodies reject them.

Attach the script in the scene:

```text
[Player]
parent = $root
script = "res://scripts/player.rs"
script_vars = { speed = 6.0 }
    [CharacterBody3D]
        collision_layers = [1]
        collision_mask = []
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

### Walker (engine gravity helper)

Simplest form: let `physics_apply_gravity_3d!` handle falling, script only drives horizontal motion.

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default = 6.0]
    pub speed: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);

        let mut dir = Vector3::ZERO;
        if ctx.ipt.Actions().down("move_forward") { dir.z -= 1.0; }
        if ctx.ipt.Actions().down("move_back")    { dir.z += 1.0; }
        if ctx.ipt.Actions().down("move_left")    { dir.x -= 1.0; }
        if ctx.ipt.Actions().down("move_right")   { dir.x += 1.0; }

        let speed = with_state!(ctx.run, PlayerState, ctx.id, |s| s.speed);
        physics_move_and_slide_3d!(ctx.run, ctx.id, dir.normalized() * speed * dt);
        physics_apply_gravity_3d!(ctx.run, ctx.id, dt);
    }
});
```

The gravity call returns the down-sweep `PhysicsMoveResult3D`: `clipped == true` means grounded this frame.
The full macro form takes a terminal fall speed and filter: `physics_apply_gravity_3d!(ctx.run, ctx.id, dt, 30.0, filter)`.

### Walker (custom gravity)

The script owns all motion, gravity included.
Feed the full step into `physics_move_and_slide_3d!` and the floor stops the fall while walls slide.

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default = 6.0]
    pub speed: f32,
    #[default = -20.0]
    pub gravity: f32,
    pub y_vel: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);

        let mut dir = Vector3::ZERO;
        if ctx.ipt.Actions().down("move_forward") { dir.z -= 1.0; }
        if ctx.ipt.Actions().down("move_back")    { dir.z += 1.0; }
        if ctx.ipt.Actions().down("move_left")    { dir.x -= 1.0; }
        if ctx.ipt.Actions().down("move_right")   { dir.x += 1.0; }

        let (speed, gravity, mut y_vel) = with_state!(
            ctx.run, PlayerState, ctx.id,
            |s| (s.speed, s.gravity, s.y_vel)
        );
        y_vel += gravity * dt;

        let motion = dir.normalized() * speed * dt + Vector3::new(0.0, y_vel * dt, 0.0);
        if let Some(res) = physics_move_and_slide_3d!(ctx.run, ctx.id, motion) {
            // floor hit -> stop falling
            if res.hits.iter().any(|h| h.normal.y > 0.5) { y_vel = 0.0; }
        }

        with_state_mut!(ctx.run, PlayerState, ctx.id, |s| s.y_vel = y_vel);
    }
});
```

Action names (`move_forward`, ...) come from the project input map.
`dir.normalized()` is zero-safe: no input means no move.

### Jumper

A character body stores no velocity, and the engine gravity helper only falls — it cannot jump.
Own the vertical velocity in script state, integrate gravity yourself,
ground-check with `contacts_3d!` (or a downward raycast).

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default = 6.0]
    pub speed: f32,
    #[default = 9.0]
    pub jump: f32,
    #[default = -20.0]
    pub gravity: f32,
    pub y_vel: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);

        let mut dir = Vector3::ZERO;
        if ctx.ipt.Actions().down("move_forward") { dir.z -= 1.0; }
        if ctx.ipt.Actions().down("move_back")    { dir.z += 1.0; }
        if ctx.ipt.Actions().down("move_left")    { dir.x -= 1.0; }
        if ctx.ipt.Actions().down("move_right")   { dir.x += 1.0; }
        let jump = ctx.ipt.Actions().pressed("jump");

        // pull tuning + current vertical velocity out of state first
        let (speed, jump_v, gravity, mut y_vel) = with_state!(
            ctx.run, PlayerState, ctx.id,
            |s| (s.speed, s.jump, s.gravity, s.y_vel)
        );

        // grounded if any contact pushes up
        let grounded = physics_contacts_3d!(ctx.run, ctx.id)
            .iter()
            .any(|c| c.normal.y > 0.5);

        if grounded && y_vel < 0.0 { y_vel = 0.0; }
        if grounded && jump { y_vel = jump_v; }
        y_vel += gravity * dt;

        let step = dir.normalized() * speed * dt + Vector3::new(0.0, y_vel * dt, 0.0);
        physics_move_and_slide_3d!(ctx.run, ctx.id, step);

        // write velocity back
        with_state_mut!(ctx.run, PlayerState, ctx.id, |s| s.y_vel = y_vel);
    }
});
```

Read state and run the physics macros separately.
A `with_state` closure holds `ctx.run`, so no `ctx.run` macro nests inside it.
The slide sweep stops the body on the floor and slides along walls.
When a downward move clips, `contacts_3d` reports the up-normal and grounds the player.

## Notes

- Areas also need child collision shapes for overlap/query volume.
- Bodies and areas can have more than one child collision shape.
- Shape local transform comes from the shape node's `Node2D` or `Node3D` data.
- Collision shapes only provide geometry.
