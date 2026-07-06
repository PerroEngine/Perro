# Textures Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `reserve` | [`reserve`](#reserve) |
| `reserve_hashed` | [`reserve_hashed`](#reserve_hashed) |
| `reserve_hashed_with_source` | [`reserve_hashed_with_source`](#reserve_hashed_with_source) |
| `drop` | [`drop`](#drop) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `texture_load` | [`texture_load`](#texture_load) |
| `texture_reserve` | [`texture_reserve`](#texture_reserve) |
| `texture_drop` | [`texture_drop`](#texture_drop) |
| `texture_is_loaded` | [`texture_is_loaded`](#texture_is_loaded) |

## Overview

This resource module belongs to `ctx.res` and documents textures calls.
Texture loads return a `TextureID` immediately and do not block the frame.
Renderer uses the texture once async decode/upload completes.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Textures()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when texture data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Textures().create_from_rgba(w, h, rgba)` | `TextureID` | Raw RGBA8 bytes; length must be `w * h * 4`. |
| `ctx.res.Textures().create_from_bytes(bytes)` | `TextureID` | Decodes image bytes or `PTEX`. |
| `texture_create_from_rgba!(ctx.res, w, h, rgba)` | `TextureID` | Macro form. |
| `texture_create_from_bytes!(ctx.res, bytes)` | `TextureID` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let texture = texture_load!(ctx.res, "res://textures/player.png");
        // assign texture now; renderer uses it once ready
        let _ = texture;
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> TextureID` |
| Params | `&self, source: S` |
| Returns | `TextureID` |
| Use when | Use when code needs an ID now; renderer can use it once async load finishes. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> TextureID` |
| Params | `&self, source_hash: u64` |
| Returns | `TextureID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> TextureID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `TextureID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn reserve<A: TextureReserveArg>(&self, arg: A) -> TextureID` |
| Params | `&self, source_or_id` |
| Returns | `TextureID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it, or when an existing `TextureID` should be promoted to reserved. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> TextureID` |
| Params | `&self, source_hash: u64` |
| Returns | `TextureID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> TextureID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `TextureID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn drop(&self, id: TextureID) -> bool` |
| Params | `&self, id: TextureID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn is_loaded(&self, id: TextureID) -> bool` |
| Params | `&self, id: TextureID` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `texture_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `texture_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_reserve!(ctx.res, source_or_id)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `texture_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_drop!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `texture_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_is_loaded!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

