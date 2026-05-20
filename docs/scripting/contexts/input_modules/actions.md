# Actions Module

## Page Map

| Header          | Link                              |
| --------------- | --------------------------------- |
| Overview        | [Overview](#overview)             |
| Context         | [Context](#context)               |
| API Reference   | [API Reference](#api-reference)   |
| `down`          | [`down`](#down)                   |
| `pressed`       | [`pressed`](#pressed)             |
| `released`      | [`released`](#released)           |
| `down_hash`     | [`down_hash`](#down_hash)         |
| `pressed_hash`  | [`pressed_hash`](#pressed_hash)   |
| `released_hash` | [`released_hash`](#released_hash) |

## Overview

This input module belongs to `ctx.ipt` and documents actions calls.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Actions()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `down`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn down(&self, name: &str) -> bool`                                                                                                                                                                           |
| Params                     | `&self, name: &str`                                                                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Actions().down("name");
        let _ = value;
    }
});
```

### `pressed`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn pressed(&self, name: &str) -> bool`                                                                                                                                                                        |
| Params                     | `&self, name: &str`                                                                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Actions().pressed("name");
        let _ = value;
    }
});
```

### `released`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn released(&self, name: &str) -> bool`                                                                                                                                                                       |
| Params                     | `&self, name: &str`                                                                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Actions().released("name");
        let _ = value;
    }
});
```

### `down_hash`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn down_hash(&self, name_hash: u64) -> bool`                                                                                                                                                                  |
| Params                     | `&self, name_hash: u64`                                                                                                                                                                                            |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Actions().down_hash(0);
        let _ = value;
    }
});
```

### `pressed_hash`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn pressed_hash(&self, name_hash: u64) -> bool`                                                                                                                                                               |
| Params                     | `&self, name_hash: u64`                                                                                                                                                                                            |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Actions().pressed_hash(0);
        let _ = value;
    }
});
```

### `released_hash`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn released_hash(&self, name_hash: u64) -> bool`                                                                                                                                                              |
| Params                     | `&self, name_hash: u64`                                                                                                                                                                                            |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.ipt.Actions().released_hash(0);
        let _ = value;
    }
});
```

### `action_down`

| Field                      | Detail                                                                                                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.ipt`                                                                                                                                                        |
| Signature                  | `action_down!(ctx.ipt, "jump")`                                                                                                                                  |
| Params                     | `ctx.ipt, "jump"`                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                           |
| Use when                   | Use when gameplay needs held input state, such as movement, aim, charge, or drag.                                                                                |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = action_down!(ctx.ipt, "jump");
        let _ = value;
    }
});
```

### `action_pressed`

| Field                      | Detail                                                                                                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.ipt`                                                                                                                                                        |
| Signature                  | `action_pressed!(ctx.ipt, "jump")`                                                                                                                               |
| Params                     | `ctx.ipt, "jump"`                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                           |
| Use when                   | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release.                                                                       |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = action_pressed!(ctx.ipt, "jump");
        let _ = value;
    }
});
```

### `action_released`

| Field                      | Detail                                                                                                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.ipt`                                                                                                                                                        |
| Signature                  | `action_released!(ctx.ipt, "jump")`                                                                                                                              |
| Params                     | `ctx.ipt, "jump"`                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                           |
| Use when                   | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release.                                                                       |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = action_released!(ctx.ipt, "jump");
        let _ = value;
    }
});
```
