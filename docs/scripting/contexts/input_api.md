# Input API

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Input Window | [Input Window](#input-window) |
| Input Modules | [Input Modules](#input-modules) |
| Example | [Example](#example) |

## Purpose

`ctx.ipt` is the frame's input snapshot plus a queue for device commands. It
answers the two questions gameplay code asks every frame: what is held right now,
and what changed this frame. Use `down` for held controls (movement, aim,
charge), and `pressed`/`released` for one-frame edges (jump, confirm, cancel).
Mutating calls (cursor mode, rumble, rebinds) queue commands that the input
backend applies on the next input frame.

## Use Cases

- Character movement: sample held keys with `key_down!(ctx.ipt, KeyCode::KeyW)`
  or a gamepad stick with `gamepad_left_stick!(ctx.ipt, 0)`.
- Jump and confirm: fire once on the press edge with `key_pressed!` /
  `action_pressed!(ctx.ipt, "jump")` so a held key never re-triggers.
- Abstract, rebindable controls: bind "jump" to keyboard, pad, and Joy-Con at
  once through `ctx.ipt.Actions()`, then let players remap it live.
- First-person mouselook: capture the cursor with `mouse_set_mode!(ctx.ipt,
  MouseMode::Captured)` and read look motion from `mouse_delta!(ctx.ipt)`.
- Couch co-op: assign each player a device slot with `ctx.ipt.Players()` and
  route their input independently.
- Force feedback: pulse a controller on impact with
  `gamepad_set_rumble!(ctx.ipt, 0, 0.6, 0.6)`.

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
        // Held movement on the left stick, jump on the press edge.
        let move_dir = gamepad_left_stick!(ctx.ipt, 0);
        if action_pressed!(ctx.ipt, "jump") {
            // start jump
        }
        let _ = move_dir;
    }
});
```
