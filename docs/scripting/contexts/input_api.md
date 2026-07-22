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

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Character moves while control is held | `down` or stick value | Continuous state matches continuous motion | Runs every frame while held |
| Jump/confirm fires once | `pressed` edge | Held input does not retrigger | Edge exists for one input frame only |
| Charged action fires on release | `released` edge | Release moment is explicit | Charge duration must live in script state |
| Player remaps controls or switches device | action API | Gameplay reads one semantic action across bindings | Action names/config become runtime contracts |
| First-person camera needs unbounded mouse motion | captured mode + delta | Relative motion works without screen-edge limits | UI cursor must restore another mode when leaving gameplay |
| Local multiplayer owns device assignment per player | players API | Separates player identity from physical device slot | Disconnect/reconnect needs an assignment policy |

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
