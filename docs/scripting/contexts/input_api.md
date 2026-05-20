# Input API

## Page Map

| Header | Link |
| --- | --- |
| Input Window | [Input Window](#input-window) |
| Input Modules | [Input Modules](#input-modules) |
| Example | [Example](#example) |

## Input Window

Use `ctx.ipt` for frame input state and queued input device commands. Use pressed/released for one-frame edges and down for held controls.

## Input Modules

| Module | Page | Ctx |
| --- | --- | --- |
| Actions | [actions](input_modules/actions.md) | `ctx.ipt.Actions()` |
| Gamepads | [gamepads](input_modules/gamepads.md) | `ctx.ipt.Gamepads()` |
| Joycons | [joycons](input_modules/joycons.md) | `ctx.ipt.JoyCons()` |
| Keys | [keys](input_modules/keys.md) | `ctx.ipt.Keys()` |
| Mouse | [mouse](input_modules/mouse.md) | `ctx.ipt.Mouse()` |
| Players | [players](input_modules/players.md) | `ctx.ipt.Players()` |

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if key_pressed!(ctx.ipt, KeyCode::Space) {
            // jump
        }
    }
});
```
