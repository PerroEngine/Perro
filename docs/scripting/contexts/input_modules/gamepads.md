# Gamepads Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `all` | [`all`](#all) |
| `get` | [`get`](#get) |
| `set_rumble` | [`set_rumble`](#set_rumble) |

## Overview

This input module belongs to `ctx.ipt` and documents gamepads calls.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Gamepads()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `all`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Gamepads()` |
| Signature | `pub fn all(&self) -> &'ipt [GamepadState]` |
| Params | `&self` |
| Returns | `&'ipt [GamepadState]` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Gamepads().all();
        let _ = value;
    }
});
```

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Gamepads()` |
| Signature | `pub fn get(&self, index: usize) -> Option<&'ipt GamepadState>` |
| Params | `&self, index: usize` |
| Returns | `Option<&'ipt GamepadState>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Gamepads().get(0);
        let _ = value;
    }
});
```

### `set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Gamepads()` |
| Signature | `pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32)` |
| Params | `&self, index: usize, low_frequency: f32, high_frequency: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Gamepads().set_rumble(0, 1.0, 1.0);
        let _ = value;
    }
});
```

### `gamepad_accel`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_accel!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_accel!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `gamepad_down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_down!(ctx.ipt, 0, GamepadButton::Bottom)` |
| Params | `ctx.ipt, 0, GamepadButton::Bottom` |
| Returns | `bool` |
| Use when | Use when gameplay needs held input state, such as movement, aim, charge, or drag. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_down!(ctx.ipt, 0, GamepadButton::Bottom);
        let _ = value;
    }
});
```

### `gamepad_get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_get!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Option` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_get!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `gamepad_gyro`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_gyro!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_gyro!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `gamepad_left_stick`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_left_stick!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector2` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_left_stick!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `gamepad_list`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_list!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `slice` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_list!(ctx.ipt);
        let _ = value;
    }
});
```

### `gamepad_pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_pressed!(ctx.ipt, 0, GamepadButton::Bottom)` |
| Params | `ctx.ipt, 0, GamepadButton::Bottom` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_pressed!(ctx.ipt, 0, GamepadButton::Bottom);
        let _ = value;
    }
});
```

### `gamepad_released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_released!(ctx.ipt, 0, GamepadButton::Bottom)` |
| Params | `ctx.ipt, 0, GamepadButton::Bottom` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_released!(ctx.ipt, 0, GamepadButton::Bottom);
        let _ = value;
    }
});
```

### `gamepad_right_stick`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_right_stick!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector2` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_right_stick!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `gamepad_set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `gamepad_set_rumble!(ctx.ipt, 0, 0.5, 0.5)` |
| Params | `ctx.ipt, 0, 0.5, 0.5` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = gamepad_set_rumble!(ctx.ipt, 0, 0.5, 0.5);
        let _ = value;
    }
});
```
