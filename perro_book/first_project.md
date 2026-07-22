# First Project

Create a small project and add one controllable node.

## Goal

Make a scene with a player script.

Run it through the dev runner.

## Feature Shape

The scene owns the player node and its starting values. The script owns input
response and per-player state. `ctx.id` identifies that same player, so movement
does not need a name lookup or query.

This is the smallest useful ownership split: authored data stays visible in the
scene, while frame behavior stays typed in Rust.

## Create Project

```powershell
perro new --name MyGame --path D:\GameProjects
perro dev --path D:\GameProjects\MyGame
```

If using the source workspace:

```powershell
cargo run -p perro_cli -- new --name MyGame --path D:\GameProjects
cargo run -p perro_cli -- dev --path D:\GameProjects\MyGame
```

## Add Script

Create a script under `res/scripts`.

```powershell
perro new_script --name Player --res /scripts --path D:\GameProjects\MyGame
```

Core shape:

```rust
use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct PlayerState {
    #[default(240.0)]
    #[expose]
    speed: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let speed = with_state!(ctx.run, PlayerState, ctx.id, |state| state.speed);
        let mut delta = Vector2::ZERO;

        if key_down!(ctx.ipt, KeyCode::KeyD) {
            delta.x += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyA) {
            delta.x -= 1.0;
        }

        if delta.length_squared() > 0.0 {
            let step = delta.normalized() * speed * dt;
            let _ = with_base_node_mut!(ctx.run, SelfNodeType, ctx.id, |node| {
                node.transform.position += step;
            });
        }
    }
});
```

## Attach Script

Attach the script to a `Node2D` or `Sprite2D` in the scene.

Use editor scene files or scene templates.

Run:

```powershell
perro check --path D:\GameProjects\MyGame
perro dev --path D:\GameProjects\MyGame
```

## Dev Loop

Use this loop:

1. edit scene or script
2. run `perro check`
3. run `perro dev`
4. move one feature at a time

Run `check` before `dev`: compile and scene-wiring errors arrive without waiting
for the game window. Use `dev` after the project is structurally valid because
its job is behavior feedback, not first-pass diagnosis.

## Next Choice

- fixed camera or HUD target -> inject `NodeID` through `script_vars`
- child created as part of the player -> use the parent/child relation
- changing set of enemies or pickups -> use a query
- one known target command -> call a method
- event with zero or many listeners -> emit a signal

## Reference

- [Scripting Overview](/docs/scripting/README.md)
- [Script State](/docs/scripting/state.md)
- [Script Lifecycle](/docs/scripting/lifecycle.md)
- [Input API](/docs/scripting/contexts/input_api.md)
- [Node Types](/docs/scripting/nodes.md)
