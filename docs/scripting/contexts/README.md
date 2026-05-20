# Script Contexts

## Page Map

| Header | Link |
| --- | --- |
| Context Fields | [Context Fields](#context-fields) |
| API Areas | [API Areas](#api-areas) |
| Example Shape | [Example Shape](#example-shape) |

## Context Fields

Every script lifecycle and method receives one context value.

| Field | Meaning | Use for |
| --- | --- | --- |
| `ctx.run` | Runtime API window | nodes, scenes, time, window, physics, signals, runtime audio |
| `ctx.res` | Resource API window | textures, meshes, materials, audio assets, CSV, localization, draw helpers |
| `ctx.ipt` | Input API window | keys, mouse, gamepads, Joy-Cons, players, action map |
| `ctx.id` | Current script node ID | self node lookup, state access, node transforms |

## API Areas

| Area | Page | Ctx |
| --- | --- | --- |
| Runtime | [Runtime API](runtime_api.md) | `ctx.run` |
| Resource | [Resource API](resource_api.md) | `ctx.res` |
| Input | [Input API](input_api.md) | `ctx.ipt` |

## Example Shape

Lifecycle hooks live inside `lifecycle!`. The macro supplies the `impl<API>` wrapper, so hooks use `API` in `ScriptContext` but do not declare their own generic.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let jump = key_pressed!(ctx.ipt, KeyCode::Space);
        let tex = texture_load!(ctx.res, "res://textures/player.png");
        let _ = (dt, jump, tex);
    }
});
```
