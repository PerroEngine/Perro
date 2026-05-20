# Runtime API

## Page Map

| Header | Link |
| --- | --- |
| Runtime Window | [Runtime Window](#runtime-window) |
| Runtime Modules | [Runtime Modules](#runtime-modules) |
| Example | [Example](#example) |

## Runtime Window

Use `ctx.run` for runtime state: time, window commands, node mutation, scene loading, script calls, signals, physics, animation playback, and runtime audio.

## Runtime Modules

| Module | Page | Ctx |
| --- | --- | --- |
| Animations | [animations](runtime_modules/animations.md) | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Audio | [audio](runtime_modules/audio.md) | `ctx.run.Audio()` |
| Helpers | [helpers](runtime_modules/helpers.md) | `helper macros` |
| Mesh Query | [mesh_query](runtime_modules/mesh_query.md) | `ctx.run.MeshQuery()` |
| Node Query | [node_query](runtime_modules/node_query.md) | `ctx.run.NodeQuery()` |
| Nodes | [nodes](runtime_modules/nodes.md) | `ctx.run.Nodes()` |
| Physics | [physics](runtime_modules/physics.md) | `ctx.run.Physics()` |
| Scenes | [scenes](runtime_modules/scenes.md) | `ctx.run.Scene()` |
| Scripts | [scripts](runtime_modules/scripts.md) | `ctx.run.Scripts()` |
| Signals | [signals](runtime_modules/signals.md) | `ctx.run.Signals()` |
| Time | [time](runtime_modules/time.md) | `ctx.run.Time()` |
| Window | [window](runtime_modules/window.md) | `ctx.run.Window()` |

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        if dt > 0.0 {
            window_set_title!(ctx.run, "Perro");
        }
    }
});
```
