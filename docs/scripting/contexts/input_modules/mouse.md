# Mouse Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `down` | [`down`](#down) |
| `pressed` | [`pressed`](#pressed) |
| `released` | [`released`](#released) |
| `delta` | [`delta`](#delta) |
| `wheel` | [`wheel`](#wheel) |
| `position` | [`position`](#position) |
| `viewport_size` | [`viewport_size`](#viewport_size) |
| `mode` | [`mode`](#mode) |
| `set_mode` | [`set_mode`](#set_mode) |
| `show` | [`show`](#show) |
| `hide` | [`hide`](#hide) |
| `capture` | [`capture`](#capture) |
| `confine` | [`confine`](#confine) |
| `confine_hidden` | [`confine_hidden`](#confine_hidden) |

## Overview

This input module belongs to `ctx.ipt` and documents mouse calls.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Mouse()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let pos = mouse_position!(ctx.ipt);
        let clicked = mouse_pressed!(ctx.ipt, MouseButton::Left);
        let _ = (pos, clicked);
    }
});
```

## API Reference

### `down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn down(&self, button: MouseButton) -> bool` |
| Params | `&self, button: MouseButton` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().down(MouseButton::Left);
        let _ = value;
    }
});
```

### `pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn pressed(&self, button: MouseButton) -> bool` |
| Params | `&self, button: MouseButton` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().pressed(MouseButton::Left);
        let _ = value;
    }
});
```

### `released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn released(&self, button: MouseButton) -> bool` |
| Params | `&self, button: MouseButton` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().released(MouseButton::Left);
        let _ = value;
    }
});
```

### `delta`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn delta(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().delta();
        let _ = value;
    }
});
```

### `wheel`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn wheel(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().wheel();
        let _ = value;
    }
});
```

### `position`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn position(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().position();
        let _ = value;
    }
});
```

### `viewport_size`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn viewport_size(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().viewport_size();
        let _ = value;
    }
});
```

### `mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn mode(&self) -> MouseMode` |
| Params | `&self` |
| Returns | `MouseMode` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().mode();
        let _ = value;
    }
});
```

### `set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn set_mode(&self, mode: MouseMode)` |
| Params | `&self, mode: MouseMode` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().set_mode(MouseMode::Visible);
        let _ = value;
    }
});
```

### `show`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn show(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().show();
        let _ = value;
    }
});
```

### `hide`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn hide(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().hide();
        let _ = value;
    }
});
```

### `capture`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn capture(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().capture();
        let _ = value;
    }
});
```

### `confine`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn confine(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().confine();
        let _ = value;
    }
});
```

### `confine_hidden`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn confine_hidden(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Mouse().confine_hidden();
        let _ = value;
    }
});
```

### `mouse_capture`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_capture!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_capture!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_confine`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_confine!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_confine!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_confine_hidden`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_confine_hidden!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_confine_hidden!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_delta`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_delta!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `Vector2` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_delta!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_down!(ctx.ipt, MouseButton::Left)` |
| Params | `ctx.ipt, MouseButton::Left` |
| Returns | `bool` |
| Use when | Use when gameplay needs held input state, such as movement, aim, charge, or drag. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_down!(ctx.ipt, MouseButton::Left);
        let _ = value;
    }
});
```

### `mouse_hide`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_hide!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_hide!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_mode!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `MouseMode` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_mode!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_released!(ctx.ipt, MouseButton::Left)` |
| Params | `ctx.ipt, MouseButton::Left` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_released!(ctx.ipt, MouseButton::Left);
        let _ = value;
    }
});
```

### `mouse_set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_set_mode!(ctx.ipt, MouseMode::Captured)` |
| Params | `ctx.ipt, MouseMode::Captured` |
| Returns | `MouseMode` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_set_mode!(ctx.ipt, MouseMode::Captured);
        let _ = value;
    }
});
```

### `mouse_show`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_show!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_show!(ctx.ipt);
        let _ = value;
    }
});
```

### `mouse_wheel`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `mouse_wheel!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `Vector2` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mouse_wheel!(ctx.ipt);
        let _ = value;
    }
});
```
