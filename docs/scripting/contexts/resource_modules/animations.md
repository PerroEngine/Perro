# Animations Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `reserve` | [`reserve`](#reserve) |
| `reserve_hashed` | [`reserve_hashed`](#reserve_hashed) |
| `reserve_hashed_with_source` | [`reserve_hashed_with_source`](#reserve_hashed_with_source) |
| `drop` | [`drop`](#drop) |
| `get` | [`get`](#get) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `animation_load` | [`animation_load`](#animation_load) |
| `animation_reserve` | [`animation_reserve`](#animation_reserve) |
| `animation_drop` | [`animation_drop`](#animation_drop) |
| `animation_is_loaded` | [`animation_is_loaded`](#animation_is_loaded) |
| `load` | [`load`](#load) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `get` | [`get`](#get) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `animation_tree_load` | [`animation_tree_load`](#animation_tree_load) |
| `animation_tree_is_loaded` | [`animation_tree_is_loaded`](#animation_tree_is_loaded) |

## Overview

This resource module belongs to `ctx.res` and documents animations calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Animations() / ctx.res.AnimationTrees()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> AnimationID` |
| Params | `&self, source: S` |
| Returns | `AnimationID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().load("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> AnimationID` |
| Params | `&self, source_hash: u64` |
| Returns | `AnimationID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().load_hashed(0);
        let _ = value;
    }
});
```

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> AnimationID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `AnimationID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().load_hashed_with_source(0, "res://path/to/resource");
        let _ = value;
    }
});
```

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn reserve<S: ResPathSource>(&self, source: S) -> AnimationID` |
| Params | `&self, source: S` |
| Returns | `AnimationID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().reserve("res://path/to/resource");
        let _ = value;
    }
});
```

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> AnimationID` |
| Params | `&self, source_hash: u64` |
| Returns | `AnimationID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().reserve_hashed(0);
        let _ = value;
    }
});
```

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> AnimationID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `AnimationID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().reserve_hashed_with_source(0, "res://path/to/resource");
        let _ = value;
    }
});
```

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn drop(&self, id: AnimationID) -> bool` |
| Params | `&self, id: AnimationID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().drop(0.1);
        let _ = value;
    }
});
```

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn get(&self, id: AnimationID) -> Option<Arc<AnimationClip>>` |
| Params | `&self, id: AnimationID` |
| Returns | `Option<Arc<AnimationClip>>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().get(0.1);
        let _ = value;
    }
});
```

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn is_loaded(&self, id: AnimationID) -> bool` |
| Params | `&self, id: AnimationID` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Animations().is_loaded(0.1);
        let _ = value;
    }
});
```

### `animation_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Signature | `animation_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = animation_load!(ctx.res, "res://anim/run.panim");
        let _ = value;
    }
});
```

### `animation_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Signature | `animation_reserve!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = animation_reserve!(ctx.res, "res://anim/run.panim");
        let _ = value;
    }
});
```

### `animation_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Signature | `animation_drop!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = animation_drop!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `animation_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Signature | `animation_is_loaded!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = animation_is_loaded!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> AnimationTreeID` |
| Params | `&self, source: S` |
| Returns | `AnimationTreeID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.AnimationTrees().load("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> AnimationTreeID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `AnimationTreeID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.AnimationTrees().load_hashed_with_source(0, "res://path/to/resource");
        let _ = value;
    }
});
```

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn get(&self, id: AnimationTreeID) -> Option<Arc<AnimationTreeAsset>>` |
| Params | `&self, id: AnimationTreeID` |
| Returns | `Option<Arc<AnimationTreeAsset>>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.AnimationTrees().get(0.1);
        let _ = value;
    }
});
```

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn is_loaded(&self, id: AnimationTreeID) -> bool` |
| Params | `&self, id: AnimationTreeID` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.AnimationTrees().is_loaded(0.1);
        let _ = value;
    }
});
```

### `animation_tree_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Signature | `animation_tree_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = animation_tree_load!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `animation_tree_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Signature | `animation_tree_is_loaded!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = animation_tree_is_loaded!(ctx.res, 0.1);
        let _ = value;
    }
});
```
