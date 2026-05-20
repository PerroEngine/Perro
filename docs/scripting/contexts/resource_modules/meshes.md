# Meshes Module

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
| `create` | [`create`](#create) |
| `get_data` | [`get_data`](#get_data) |
| `write` | [`write`](#write) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `mesh_load` | [`mesh_load`](#mesh_load) |
| `mesh_reserve` | [`mesh_reserve`](#mesh_reserve) |
| `mesh_drop` | [`mesh_drop`](#mesh_drop) |
| `mesh_create` | [`mesh_create`](#mesh_create) |
| `mesh_get_data` | [`mesh_get_data`](#mesh_get_data) |
| `mesh_write` | [`mesh_write`](#mesh_write) |
| `mesh_is_loaded` | [`mesh_is_loaded`](#mesh_is_loaded) |

## Overview

This resource module belongs to `ctx.res` and documents meshes calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Meshes()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let mesh = mesh_load!(ctx.res, "res://meshes/player.glb");
        let ready = mesh_is_loaded!(ctx.res, mesh);
        let _ = ready;
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> MeshID` |
| Params | `&self, source: S` |
| Returns | `MeshID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().load("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> MeshID` |
| Params | `&self, source_hash: u64` |
| Returns | `MeshID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().load_hashed(0);
        let _ = value;
    }
});
```

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> MeshID` |
| Params | `&self, source_hash: u64, source: S` |
| Returns | `MeshID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().load_hashed_with_source(0, "res://path/to/resource");
        let _ = value;
    }
});
```

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn reserve<S: ResPathSource>(&self, source: S) -> MeshID` |
| Params | `&self, source: S` |
| Returns | `MeshID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().reserve("res://path/to/resource");
        let _ = value;
    }
});
```

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> MeshID` |
| Params | `&self, source_hash: u64` |
| Returns | `MeshID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().reserve_hashed(0);
        let _ = value;
    }
});
```

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> MeshID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `MeshID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().reserve_hashed_with_source(0, "res://path/to/resource");
        let _ = value;
    }
});
```

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn drop(&self, id: MeshID) -> bool` |
| Params | `&self, id: MeshID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().drop(0.1);
        let _ = value;
    }
});
```

### `create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn create(&self, data: Mesh3D) -> MeshID` |
| Params | `&self, data: Mesh3D` |
| Returns | `MeshID` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().create(0.1);
        let _ = value;
    }
});
```

### `get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn get_data(&self, id: MeshID) -> Option<Mesh3D>` |
| Params | `&self, id: MeshID` |
| Returns | `Option<Mesh3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().get_data(0.1);
        let _ = value;
    }
});
```

### `write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn write(&self, id: MeshID, data: Mesh3D) -> bool` |
| Params | `&self, id: MeshID, data: Mesh3D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().write(0.0, 0.1);
        let _ = value;
    }
});
```

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn is_loaded(&self, id: MeshID) -> bool` |
| Params | `&self, id: MeshID` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Meshes().is_loaded(0.1);
        let _ = value;
    }
});
```

### `mesh_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_load!(ctx.res, "res://meshes/hero.glb");
        let _ = value;
    }
});
```

### `mesh_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_reserve!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_reserve!(ctx.res, "res://meshes/hero.glb");
        let _ = value;
    }
});
```

### `mesh_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_drop!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_drop!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `mesh_create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_create!(ctx.res.res, data)` |
| Params | `ctx.res, data` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_create!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `mesh_get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_get_data!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_get_data!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `mesh_write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_write!(ctx.res.res, id, data)` |
| Params | `ctx.res, id, data` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_write!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```

### `mesh_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_is_loaded!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = mesh_is_loaded!(ctx.res, 0.1);
        let _ = value;
    }
});
```
