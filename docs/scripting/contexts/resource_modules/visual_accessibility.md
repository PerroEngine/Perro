# Visual Accessibility Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `enable_colorblind_filter` | [`enable_colorblind_filter`](#enable_colorblind_filter) |
| `disable_colorblind_filter` | [`disable_colorblind_filter`](#disable_colorblind_filter) |

## Overview

This resource module belongs to `ctx.res` and documents visual accessibility calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `enable_colorblind_filter`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `enable_colorblind_filter!(ctx.res.res, mode, strength)` |
| Params | `ctx.res, mode, strength` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = enable_colorblind_filter!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```

### `disable_colorblind_filter`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `disable_colorblind_filter!(ctx.res.res)` |
| Params | `ctx.res` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = disable_colorblind_filter!(ctx.res);
        let _ = value;
    }
});
```
