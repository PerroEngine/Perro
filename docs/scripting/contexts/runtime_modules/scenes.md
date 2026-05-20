# Scenes Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `preload` | [`preload`](#preload) |
| `preload_hashed` | [`preload_hashed`](#preload_hashed) |
| `load_preloaded` | [`load_preloaded`](#load_preloaded) |
| `free_preloaded` | [`free_preloaded`](#free_preloaded) |
| `drop_preloaded` | [`drop_preloaded`](#drop_preloaded) |
| `drop_preloaded_hashed` | [`drop_preloaded_hashed`](#drop_preloaded_hashed) |
| `scene_load` | [`scene_load`](#scene_load) |
| `scene_preload` | [`scene_preload`](#scene_preload) |
| `scene_free_preloaded` | [`scene_free_preloaded`](#scene_free_preloaded) |
| `scene_drop_preloaded` | [`scene_drop_preloaded`](#scene_drop_preloaded) |

## Overview

This runtime module belongs to `ctx.run` and documents scenes calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Scene()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load<S: IntoSceneLoadSource>(&mut self, source: S) -> Result<NodeID, String>` |
| Params | `&mut self, source: S` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().load("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String>` |
| Params | `&mut self, path_hash: u64, path: &str` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().load_hashed(0, "name");
        let _ = value;
    }
});
```

### `preload`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload<P: IntoScenePath>(&mut self, path: P) -> Result<PreloadedSceneID, String>` |
| Params | `&mut self, path: P` |
| Returns | `Result<PreloadedSceneID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().preload("res://path/to/resource");
        let _ = value;
    }
});
```

### `preload_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload_hashed( &mut self, path_hash: u64, path: &str, ) -> Result<PreloadedSceneID, String>` |
| Params | `&mut self, path_hash: u64, path: &str,` |
| Returns | `Result<PreloadedSceneID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().preload_hashed(0, "name");
        let _ = value;
    }
});
```

### `load_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> Result<NodeID, String>` |
| Params | `&mut self, id: I` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().load_preloaded(0.1);
        let _ = value;
    }
});
```

### `free_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn free_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> bool` |
| Params | `&mut self, id: I` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().free_preloaded(0.1);
        let _ = value;
    }
});
```

### `drop_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn drop_preloaded<T: IntoPreloadedSceneTarget>(&mut self, target: T) -> bool` |
| Params | `&mut self, target: T` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().drop_preloaded(ctx.id);
        let _ = value;
    }
});
```

### `drop_preloaded_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn drop_preloaded_hashed(&mut self, path_hash: u64, path: &str) -> bool` |
| Params | `&mut self, path_hash: u64, path: &str` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scene().drop_preloaded_hashed(0, "name");
        let _ = value;
    }
});
```

### `scene_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_load!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = scene_load!(ctx.run, "res://scenes/level.scn");
        let _ = value;
    }
});
```

### `scene_preload`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_preload!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = scene_preload!(ctx.run, "res://scenes/level.scn");
        let _ = value;
    }
});
```

### `scene_free_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_free_preloaded!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = scene_free_preloaded!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `scene_drop_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_drop_preloaded!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = scene_drop_preloaded!(ctx.run, 0.1);
        let _ = value;
    }
});
```
