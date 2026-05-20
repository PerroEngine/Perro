# Post Processing Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `post_processing_set` | [`post_processing_set`](#post_processing_set) |
| `post_processing_add` | [`post_processing_add`](#post_processing_add) |
| `post_processing_remove` | [`post_processing_remove`](#post_processing_remove) |
| `post_processing_clear` | [`post_processing_clear`](#post_processing_clear) |

## Overview

This resource module belongs to `ctx.res` and documents post processing calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `post_processing_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_set!(ctx.res.res, set)` |
| Params | `ctx.res, set` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = post_processing_set!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `post_processing_add`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_add!(ctx.res.res, effect)` |
| Params | `ctx.res, effect` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = post_processing_add!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `post_processing_remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_remove!(ctx.res.res, name = name)` |
| Params | `ctx.res, name = name` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = post_processing_remove!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `post_processing_clear`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_clear!(ctx.res.res)` |
| Params | `ctx.res` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = post_processing_clear!(ctx.res);
        let _ = value;
    }
});
```
