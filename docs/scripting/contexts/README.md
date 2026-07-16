# Script Contexts

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Context Fields | [Context Fields](#context-fields) |
| Use Cases | [Use Cases](#use-cases) |
| API Areas | [API Areas](#api-areas) |
| Example Shape | [Example Shape](#example-shape) |

## Purpose

Every script lifecycle hook and method receives one `ScriptContext`. It is the
single door from your game logic to the engine: read the frame's input, mutate
nodes and physics, load scenes and assets, and emit signals. Instead of holding
global handles, you reach the engine through the context passed into each call,
so hot-reloaded scripts always talk to the live runtime.

## Context Fields

Every script lifecycle and method receives one context value.

| Field | Meaning | Use for |
| --- | --- | --- |
| `ctx.run` | Runtime API window | nodes, scenes, time, window, physics, signals, runtime audio |
| `ctx.res` | Resource API window | textures, meshes, materials, audio assets, CSV, localization, draw helpers |
| `ctx.ipt` | Input API window | keys, mouse, gamepads, Joy-Cons, players, action map |
| `ctx.id` | Current script node ID | self node lookup, state access, node transforms |

## Use Cases

- Player controller: read a jump edge with `key_pressed!(ctx.ipt, KeyCode::Space)`,
  move the body with `ctx.run`, and step physics each frame.
- Scene flow: preload a level with `scene_preload!(ctx.run, ...)` in `on_init`,
  then swap to it with `scene_load!(ctx.run, ...)` when the player reaches the exit.
- HUD update: pull `delta_time!(ctx.run)` and the mouse position from `ctx.ipt`
  to drive an aim reticle, and load its texture through `ctx.res`.
- Event wiring: connect a button's `pressed` signal in `on_all_init` and react in
  a `methods!` handler that mutates state via `with_state_mut!`.
- Per-node identity: use `ctx.id` to read and write this script's own `#[State]`
  block and to look up the node's transform.

## API Areas

| Area | Page | Ctx |
| --- | --- | --- |
| Runtime | [Runtime API](runtime_api.md) | `ctx.run` |
| Resource | [Resource API](resource_api.md) | `ctx.res` |
| Input | [Input API](input_api.md) | `ctx.ipt` |

## Example Shape

Lifecycle hooks live inside `lifecycle!`. The macro supplies the `impl<API>` wrapper, so hooks use `API` in `ScriptContext` but do not declare their own generic. Reusable state lives in a `#[State]` struct, and signal handlers or button callbacks live in `methods!`.

```rust
#[State]
struct PlayerState {
    #[default = 0]
    coins: i64,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let jump = key_pressed!(ctx.ipt, KeyCode::Space);
        let tex = texture_load!(ctx.res, "res://textures/player.png");
        let _ = (dt, jump, tex);
    }
});

methods!({
    fn on_coin_pickup(&self, ctx: &mut ScriptContext<'_, API>, _coin: NodeID) {
        with_state_mut!(ctx.run, PlayerState, ctx.id, |state| state.coins += 1);
    }
});
```
