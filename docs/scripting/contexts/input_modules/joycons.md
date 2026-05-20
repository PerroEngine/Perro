# Joycons Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `all` | [`all`](#all) |
| `get` | [`get`](#get) |
| `set_rumble` | [`set_rumble`](#set_rumble) |
| `set_indicator` | [`set_indicator`](#set_indicator) |
| `set_indicator_slot` | [`set_indicator_slot`](#set_indicator_slot) |

## Overview

This input module belongs to `ctx.ipt` and documents joycons calls.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.JoyCons()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `all`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn all(&self) -> &'ipt [JoyConState]` |
| Params | `&self` |
| Returns | `&'ipt [JoyConState]` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.JoyCons().all();
        let _ = value;
    }
});
```

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn get(&self, index: usize) -> Option<&'ipt JoyConState>` |
| Params | `&self, index: usize` |
| Returns | `Option<&'ipt JoyConState>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.JoyCons().get(0);
        let _ = value;
    }
});
```

### `set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32)` |
| Params | `&self, index: usize, low_frequency: f32, high_frequency: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.JoyCons().set_rumble(0, 1.0, 1.0);
        let _ = value;
    }
});
```

### `set_indicator`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_indicator(&self, index: usize, indicator: u8)` |
| Params | `&self, index: usize, indicator: u8` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.JoyCons().set_indicator(0, 0);
        let _ = value;
    }
});
```

### `set_indicator_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_indicator_slot(&self, index: usize, slot: u8)` |
| Params | `&self, index: usize, slot: u8` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.JoyCons().set_indicator_slot(0, 0);
        let _ = value;
    }
});
```

### `joycon_accel`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_accel!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_accel!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_calibrated`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_calibrated!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_calibrated!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_calibrating`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_calibrating!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_calibrating!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_calibration_bias`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_calibration_bias!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_calibration_bias!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_connected`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_connected!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_connected!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_down!(ctx.ipt, 0, JoyConButton::Top)` |
| Params | `ctx.ipt, 0, JoyConButton::Top` |
| Returns | `bool` |
| Use when | Use when gameplay needs held input state, such as movement, aim, charge, or drag. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_down!(ctx.ipt, 0, JoyConButton::Top);
        let _ = value;
    }
});
```

### `joycon_get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_get!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Option` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_get!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_gyro`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_gyro!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_gyro!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_list`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_list!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `slice` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_list!(ctx.ipt);
        let _ = value;
    }
});
```

### `joycon_needs_calibration`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_needs_calibration!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_needs_calibration!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_pressed!(ctx.ipt, 0, JoyConButton::Top)` |
| Params | `ctx.ipt, 0, JoyConButton::Top` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_pressed!(ctx.ipt, 0, JoyConButton::Top);
        let _ = value;
    }
});
```

### `joycon_released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_released!(ctx.ipt, 0, JoyConButton::Top)` |
| Params | `ctx.ipt, 0, JoyConButton::Top` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_released!(ctx.ipt, 0, JoyConButton::Top);
        let _ = value;
    }
});
```

### `joycon_request_calibration`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_request_calibration!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_request_calibration!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_set_indicator`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_set_indicator!(ctx.ipt, 0, 1)` |
| Params | `ctx.ipt, 0, 1` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_set_indicator!(ctx.ipt, 0, 1);
        let _ = value;
    }
});
```

### `joycon_set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_set_rumble!(ctx.ipt, 0, 0.5, 0.5)` |
| Params | `ctx.ipt, 0, 0.5, 0.5` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_set_rumble!(ctx.ipt, 0, 0.5, 0.5);
        let _ = value;
    }
});
```

### `joycon_side`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_side!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Option<JoyConSide>` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_side!(ctx.ipt, 0);
        let _ = value;
    }
});
```

### `joycon_stick`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_stick!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector2` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = joycon_stick!(ctx.ipt, 0);
        let _ = value;
    }
});
```
