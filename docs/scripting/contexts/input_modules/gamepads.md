# Gamepads Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `all` | [`all`](#all) |
| `get` | [`get`](#get) |
| `set_rumble` | [`set_rumble`](#set_rumble) |
| Macros | [Macros](#macros) |

## Purpose

The gamepads module exposes every connected controller by slot index. Each
`GamepadState` carries buttons (with held/pressed/released edges), two analog
sticks, and gyro/accel motion; the module also queues rumble. Slots are stable
indices, so you can support hot-plugged pads and multiple controllers by reading
slot `0`, `1`, and up.

## Use Cases

- Twin-stick movement and aim: move with `gamepad_left_stick!(ctx.ipt, 0)` and
  aim with `gamepad_right_stick!(ctx.ipt, 0)`.
- Jump / confirm on the face button: fire once with
  `gamepad_pressed!(ctx.ipt, 0, GamepadButton::Bottom)`.
- Rumble on impact: pulse both motors with
  `gamepad_set_rumble!(ctx.ipt, 0, 0.7, 0.7)`, then stop with `0.0, 0.0`.
- Motion control: read `gamepad_gyro!(ctx.ipt, 0)` for tilt-steering or
  gyro-aim, and `gamepad_accel!` for shake gestures.
- Local multiplayer: iterate `gamepad_list!(ctx.ipt)` (or `all()`) to detect how
  many pads are connected and assign each to a player slot.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Gamepads()`
- `GamepadButton` is the button enum (face `Bottom`/`Right`/`Left`/`Top`, d-pad, `Start`/`Select`/`Home`/`Capture`, `L1`/`R1`/`L2`/`R2`/`L3`/`R3`).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Move with the left stick.
        let move_dir = gamepad_left_stick!(ctx.ipt, 0);

        // Jump on the A/cross button press edge.
        if gamepad_pressed!(ctx.ipt, 0, GamepadButton::Bottom) {
            // start jump
        }

        // Rumble while the right trigger is held.
        if gamepad_down!(ctx.ipt, 0, GamepadButton::R2) {
            gamepad_set_rumble!(ctx.ipt, 0, 0.6, 0.6);
        } else {
            gamepad_set_rumble!(ctx.ipt, 0, 0.0, 0.0);
        }
        let _ = move_dir;
    }
});
```

## API Reference

### `all`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Gamepads()` |
| Signature | `pub fn all(&self) -> &'ipt [GamepadState]` |
| Params | `&self` |
| Returns | `&'ipt [GamepadState]` |
| Use when | Enumerate connected pads, such as counting players or scanning all sticks. |
| Edge behavior | Slice covers the current device slots; may be empty when no pad is connected. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Gamepads()` |
| Signature | `pub fn get(&self, index: usize) -> Option<&'ipt GamepadState>` |
| Params | `&self, index: usize` |
| Returns | `Option<&'ipt GamepadState>` |
| Use when | Read one pad's buttons, sticks, gyro, and accel by slot. |
| Edge behavior | Returns `None` when the slot is empty. |

### `set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Gamepads()` |
| Signature | `pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32)` |
| Params | `&self, index: usize, low_frequency: f32, high_frequency: f32` |
| Returns | `()` |
| Use when | Add force feedback; set both to `0.0` to stop. |
| Edge behavior | Queues a rumble command when an input command buffer exists; missing slots are ignored. |

## Macros

For a missing slot, button reads return `false`, state reads return `None`, and
stick/motion reads return zero vectors. Command macros queue work only when an
input command buffer exists.

| Macro | Signature | Returns |
| --- | --- | --- |
| `gamepad_list!` | `gamepad_list!(ctx.ipt)` | `&[GamepadState]` |
| `gamepad_get!` | `gamepad_get!(ctx.ipt, 0)` | `Option<&GamepadState>` |
| `gamepad_down!` | `gamepad_down!(ctx.ipt, 0, GamepadButton::Bottom)` | `bool` |
| `gamepad_pressed!` | `gamepad_pressed!(ctx.ipt, 0, GamepadButton::Bottom)` | `bool` |
| `gamepad_released!` | `gamepad_released!(ctx.ipt, 0, GamepadButton::Bottom)` | `bool` |
| `gamepad_left_stick!` | `gamepad_left_stick!(ctx.ipt, 0)` | `Vector2` |
| `gamepad_right_stick!` | `gamepad_right_stick!(ctx.ipt, 0)` | `Vector2` |
| `gamepad_gyro!` | `gamepad_gyro!(ctx.ipt, 0)` | `Vector3` |
| `gamepad_accel!` | `gamepad_accel!(ctx.ipt, 0)` | `Vector3` |
| `gamepad_set_rumble!` | `gamepad_set_rumble!(ctx.ipt, 0, 0.5, 0.5)` | `()` |
