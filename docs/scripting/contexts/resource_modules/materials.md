# Materials Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| Practical Example | [Practical Example](#practical-example) |
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

## Purpose

`ctx.res.Materials()` turns a material path or a `Material3D` definition into a `MaterialID` that 3D surfaces render with. Loads return the ID the same frame and never block. Because a material can be built in code, read back, and overwritten, a script can recolor a surface, swap a shader, or tint an object at runtime without editing the source asset.

## Use Cases

- Team or faction colors: `material_get_data!` an object's material, change its base color, and `material_write!` it back to recolor per player.
- Damage or status tint: overwrite the emissive/base color while a status effect is active, then restore it.
- Building materials in code: `material_create!(ctx.res, Material3D::default())` for a fully runtime-defined surface (unlit, toon, standard, or custom shader).
- Swapping a skin: `material_load!(ctx.res, "res://materials/gold.pmat")` and assign the returned `MaterialID`.
- Decoding downloaded materials: `create_from_bytes` for engine `PMAT` text or glTF/GLB material index 0.
- Preloading and memory control: `material_reserve!` to pin, `material_is_loaded!` to poll, `material_drop!` to free.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Materials()`
- Material loads return a `MaterialID` immediately and do not block the frame; the renderer uses the material once async load/upload completes.
- Material type: `perro_render_bridge::Material3D` (`Standard`, `Unlit`, `Toon`, or `Custom`).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when material data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Materials().create_from_bytes(bytes)` | `MaterialID` | Decodes `.pmat` text or glTF/GLB material index `0`. |
| `material_create_from_bytes!(ctx.res, bytes)` | `MaterialID` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Load a material at init, then read it back and rewrite it to recolor the surface.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let material = material_load!(ctx.res, "res://materials/player.pmat");
        self.recolor(ctx, material);
    }
});

methods!({
    fn recolor(&self, ctx: &mut ScriptContext<'_, API>, material: MaterialID) {
        if let Some(data) = material_get_data!(ctx.res, material) {
            // Adjust `data` (base color, emissive, shader) then write it back.
            let ok = material_write!(ctx.res, material, data);
            let _ = ok;
        }
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> MaterialID` |
| Params | `source: S` |
| Returns | `MaterialID` |
| Use when | Getting an ID now; the renderer uses it once the async load finishes. |
| Fails when / edge behavior | Returns a nil `MaterialID` when the path is empty; a missing file resolves to nil after the async load fails. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> MaterialID` |
| Params | `source_hash: u64` |
| Returns | `MaterialID` |
| Use when | A precomputed path hash is available and the source string is not needed. |
| Fails when / edge behavior | Returns a nil `MaterialID` when no material is registered for the hash. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> MaterialID` |
| Params | `source_hash: u64, source: S` |
| Returns | `MaterialID` |
| Use when | The `material_load!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `MaterialID` when the file is missing. |

### `create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn create(&self, material: Material3D) -> MaterialID` |
| Params | `material: Material3D` |
| Returns | `MaterialID` |
| Use when | Registering a material defined in code. |
| Fails when / edge behavior | Returns a nil `MaterialID` when the material data is invalid. |

### `get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn get_data(&self, id: MaterialID) -> Option<Material3D>` |
| Params | `id: MaterialID` |
| Returns | `Option<Material3D>` |
| Use when | Reading a material's typed data to inspect or modify it. |
| Fails when / edge behavior | Returns `None` when the material data is unavailable, stale, or the ID is unknown. |

### `write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn write(&self, id: MaterialID, material: Material3D) -> bool` |
| Params | `id: MaterialID, material: Material3D` |
| Returns | `bool` |
| Use when | Overwriting an existing material, for example to recolor a surface. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or the data is invalid. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn is_loaded(&self, id: MaterialID) -> bool` |
| Params | `id: MaterialID` |
| Returns | `bool` |
| Use when | Polling whether the async load/upload has finished. |
| Fails when / edge behavior | Returns `false` while the upload is pending or when the ID is unknown. |

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn reserve<A: MaterialReserveArg>(&self, arg: A) -> MaterialID` |
| Params | `arg: A` (a path source, or an existing `MaterialID` to promote) |
| Returns | `MaterialID` |
| Use when | Pinning a material so it stays resident. |
| Fails when / edge behavior | Promoting an unknown `MaterialID` returns a nil `MaterialID`. |

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> MaterialID` |
| Params | `source_hash: u64` |
| Returns | `MaterialID` |
| Use when | Reserving by a precomputed path hash. |
| Fails when / edge behavior | Returns a nil `MaterialID` when no material is registered for the hash. |

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> MaterialID` |
| Params | `source_hash: u64, source: S` |
| Returns | `MaterialID` |
| Use when | The `material_reserve!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `MaterialID` when the file is missing. |

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `pub fn drop(&self, id: MaterialID) -> bool` |
| Params | `id: MaterialID` |
| Returns | `bool` |
| Use when | Releasing a material the game no longer uses. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `material_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_load!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `MaterialID` |
| Use when | Macro form of `load`. A literal path hashes at compile time; an expression path calls `load`. |
| Fails when / edge behavior | Returns a nil `MaterialID` when the file is missing. |

### `material_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_reserve!(ctx.res, source_or_id)` |
| Params | `ctx.res, source_or_id` |
| Returns | `MaterialID` |
| Use when | Macro form of `reserve`. |
| Fails when / edge behavior | Promoting an unknown `MaterialID` returns a nil `MaterialID`. |

### `material_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_drop!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `drop`. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `material_create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_create!(ctx.res, material)` |
| Params | `ctx.res, material` |
| Returns | `MaterialID` |
| Use when | Macro form of `create`. |
| Fails when / edge behavior | Returns a nil `MaterialID` when the material data is invalid. |

### `material_get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_get_data!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `Option<Material3D>` |
| Use when | Macro form of `get_data`. |
| Fails when / edge behavior | Returns `None` when the material data is unavailable, stale, or the ID is unknown. |

### `material_write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_write!(ctx.res, id, material)` |
| Params | `ctx.res, id, material` |
| Returns | `bool` |
| Use when | Macro form of `write`. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or the data is invalid. |

### `material_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Materials()` |
| Signature | `material_is_loaded!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `is_loaded`. |
| Fails when / edge behavior | Returns `false` while the upload is pending or when the ID is unknown. |
