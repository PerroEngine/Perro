# Keys Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `down` | [`down`](#down) |
| `pressed` | [`pressed`](#pressed) |
| `released` | [`released`](#released) |

## Overview

This input module belongs to `ctx.ipt` and documents keys calls.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Keys()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if key_pressed!(ctx.ipt, KeyCode::Space) {
            // jump once
        }
    }
});
```

## API Reference

### `down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Keys()` |
| Signature | `pub fn down(&self, key: KeyCode) -> bool` |
| Params | `&self, key: KeyCode` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Keys().down(KeyCode::Space);
        let _ = value;
    }
});
```

### `pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Keys()` |
| Signature | `pub fn pressed(&self, key: KeyCode) -> bool` |
| Params | `&self, key: KeyCode` |
| Returns | `bool` |
| Use when | Use when code branches on current state or a one-frame state edge. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Keys().pressed(KeyCode::Space);
        let _ = value;
    }
});
```

### `released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Keys()` |
| Signature | `pub fn released(&self, key: KeyCode) -> bool` |
| Params | `&self, key: KeyCode` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Keys().released(KeyCode::Space);
        let _ = value;
    }
});
```

### `key_released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `key_released!(ctx.ipt, KeyCode::Space)` |
| Params | `ctx.ipt, KeyCode::Space` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = key_released!(ctx.ipt, KeyCode::Space);
        let _ = value;
    }
});
```
