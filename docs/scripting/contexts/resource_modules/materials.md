# Materials Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `create` | [`create`](#create) |
| `get_data` | [`get_data`](#get_data) |
| `write` | [`write`](#write) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `reserve` | [`reserve`](#reserve) |
| `reserve_hashed` | [`reserve_hashed`](#reserve_hashed) |
| `reserve_hashed_with_source` | [`reserve_hashed_with_source`](#reserve_hashed_with_source) |
| `drop` | [`drop`](#drop) |
| `material_load` | [`material_load`](#material_load) |
| `material_reserve` | [`material_reserve`](#material_reserve) |
| `material_drop` | [`material_drop`](#material_drop) |
| `material_create` | [`material_create`](#material_create) |
| `material_get_data` | [`material_get_data`](#material_get_data) |
| `material_write` | [`material_write`](#material_write) |
| `material_is_loaded` | [`material_is_loaded`](#material_is_loaded) |

## Overview

This resource module belongs to `ctx.res` and documents materials calls.
Material loads return a `MaterialID` immediately and do not block the frame.
Renderer uses the material once async load/upload completes.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Materials()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let material = material_load!(ctx.res, "res://materials/player.pmat");
        let ready = material_is_loaded!(ctx.res, material);
        let _ = ready;
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> MaterialID` |
| Params | `&self, source: S` |
| Returns | `MaterialID` |
| Use when | Use when code needs an ID now; renderer can use it once async load finishes. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> MaterialID` |
| Params | `&self, source_hash: u64` |
| Returns | `MaterialID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> MaterialID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `MaterialID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn create(&self, material: Material3D) -> MaterialID` |
| Params | `&self, material: Material3D` |
| Returns | `MaterialID` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn get_data(&self, id: MaterialID) -> Option<Material3D>` |
| Params | `&self, id: MaterialID` |
| Returns | `Option<Material3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn write(&self, id: MaterialID, material: Material3D) -> bool` |
| Params | `&self, id: MaterialID, material: Material3D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn is_loaded(&self, id: MaterialID) -> bool` |
| Params | `&self, id: MaterialID` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn reserve<A: MaterialReserveArg>(&self, arg: A) -> MaterialID` |
| Params | `&self, source_or_id` |
| Returns | `MaterialID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it, or when an existing `MaterialID` should be promoted to reserved. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> MaterialID` |
| Params | `&self, source_hash: u64` |
| Returns | `MaterialID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> MaterialID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `MaterialID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn drop(&self, id: MaterialID) -> bool` |
| Params | `&self, id: MaterialID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_reserve!(ctx.res, source_or_id)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_drop!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_create!(ctx.res.res, material)` |
| Params | `ctx.res, material` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_get_data!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_write!(ctx.res.res, id, material)` |
| Params | `ctx.res, id, material` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `material_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_is_loaded!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

