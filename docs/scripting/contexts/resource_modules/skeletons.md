# Skeletons Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `load_bones_2d` | [`load_bones_2d`](#load_bones_2d) |
| `load_bones_3d` | [`load_bones_3d`](#load_bones_3d) |
| `load_bones` | [`load_bones`](#load_bones) |
| `skeleton_load_bones` | [`skeleton_load_bones`](#skeleton_load_bones) |

## Overview

This resource module belongs to `ctx.res` and documents skeletons calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Skeletons()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `load_bones_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones_2d<S: ResPathSource>(&self, source: S) -> Vec<Bone2D>` |
| Params | `&self, source: S` |
| Returns | `Vec<Bone2D>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Skeletons().load_bones_2d("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_bones_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones_3d<S: ResPathSource>(&self, source: S) -> Vec<Bone3D>` |
| Params | `&self, source: S` |
| Returns | `Vec<Bone3D>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Skeletons().load_bones_3d("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_bones`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones<S: ResPathSource>(&self, source: S) -> Vec<Bone3D>` |
| Params | `&self, source: S` |
| Returns | `Vec<Bone3D>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Skeletons().load_bones("res://path/to/resource");
        let _ = value;
    }
});
```

### `skeleton_load_bones`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `skeleton_load_bones!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = skeleton_load_bones!(ctx.res, 0.1);
        let _ = value;
    }
});
```
