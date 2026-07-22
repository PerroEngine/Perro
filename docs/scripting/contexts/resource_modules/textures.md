# Textures Module

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
| `reserve` | [`reserve`](#reserve) |
| `reserve_hashed` | [`reserve_hashed`](#reserve_hashed) |
| `reserve_hashed_with_source` | [`reserve_hashed_with_source`](#reserve_hashed_with_source) |
| `drop` | [`drop`](#drop) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `texture_load` | [`texture_load`](#texture_load) |
| `texture_reserve` | [`texture_reserve`](#texture_reserve) |
| `texture_drop` | [`texture_drop`](#texture_drop) |
| `texture_is_loaded` | [`texture_is_loaded`](#texture_is_loaded) |

## Purpose

`ctx.res.Textures()` turns an image path or raw pixels into a `TextureID` that sprites, UI, materials, and decals can reference. A load returns the ID the same frame and never blocks; the async decode and GPU upload finish in the background, and the renderer starts drawing the texture once it is ready. Use it to stream art on demand, build textures at runtime, and update dynamic images pixel by pixel.

## Use Cases

- Swapping a sprite or UI image on the fly: `texture_load!(ctx.res, "res://ui/icon.png")` and assign the returned `TextureID`.
- Procedural art: paint an RGBA8 buffer and upload it with `create_from_rgba`, for example a generated minimap or noise texture.
- Dynamic textures that change over time: `write_rgba` to replace the whole image, or `write_rgba_region` to patch just the dirty rectangle (a paintable canvas, a fog-of-war overlay).
- Decoding downloaded or embedded images: `create_from_bytes` for PNG/JPEG bytes or engine `PTEX` data.
- Preloading during a loading screen: `texture_reserve!` to pin textures so they stay resident, and `texture_is_loaded!` to poll when the async upload finishes.
- Freeing memory: `texture_drop!` to release a texture the game no longer shows.

## Ownership And Choice

The resource cache owns decoded texture data; scripts and nodes carry `TextureID` handles. Inject a typed ID from a scene path for a fixed per-instance texture. Load through `ctx.res` when the path or bytes become known at runtime. Reuse the returned ID instead of loading in `on_update`; the cache keeps repeated paths stable, but repeated calls still hide ownership and intent.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Textures()`
- Texture loads return a `TextureID` immediately and do not block the frame; the renderer uses the texture once async decode/upload completes.
- Webcam and camera-stream textures resolve through this module too; see [Webcam](webcam.md).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when texture data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Textures().create_from_rgba(w, h, rgba)` | `TextureID` | Raw RGBA8 bytes; length must be `w * h * 4`. |
| `ctx.res.Textures().create_from_bytes(bytes)` | `TextureID` | Decodes image bytes or `PTEX`. |
| `texture_create_from_rgba!(ctx.res, w, h, rgba)` | `TextureID` | Macro form. |
| `texture_create_from_bytes!(ctx.res, bytes)` | `TextureID` | Macro form. |
| `ctx.res.Textures().write_rgba(id, w, h, rgba)` | `bool` | Replace mutable RGBA8 data. |
| `ctx.res.Textures().write_rgba_region(id, x, y, w, h, rgba)` | `bool` | Update one tight RGBA8 subregion. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Build a solid-color runtime texture at init, then repaint one region each frame.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let pixels = vec![0u8; 64 * 64 * 4];
        let canvas = texture_create_from_rgba!(ctx.res, 64, 64, &pixels);
        self.paint(ctx, canvas);
    }
});

methods!({
    fn paint(&self, ctx: &mut ScriptContext<'_, API>, canvas: TextureID) {
        // Overwrite a 4x4 red patch at (0, 0).
        let patch = vec![255u8, 0, 0, 255].repeat(4 * 4);
        let ok = texture_write_rgba_region!(ctx.res, canvas, 0, 0, 4, 4, &patch);
        let _ = ok;
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> TextureID` |
| Params | `source: S` |
| Returns | `TextureID` |
| Use when | Getting an ID now; the renderer uses it once the async load finishes. |
| Fails when / edge behavior | Returns a nil `TextureID` when the path is empty; a missing file resolves to nil after the async load fails. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> TextureID` |
| Params | `source_hash: u64` |
| Returns | `TextureID` |
| Use when | A precomputed path hash is available and the source string is not needed. |
| Fails when / edge behavior | Returns a nil `TextureID` when no texture is registered for the hash. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> TextureID` |
| Params | `source_hash: u64, source: S` |
| Returns | `TextureID` |
| Use when | The `texture_load!` literal path builds a compile-time hash and passes the source for first-load resolution. |
| Fails when / edge behavior | Returns a nil `TextureID` when the file is missing. |

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn reserve<A: TextureReserveArg>(&self, arg: A) -> TextureID` |
| Params | `arg: A` (a path source, or an existing `TextureID` to promote) |
| Returns | `TextureID` |
| Use when | Pinning a texture so it stays resident, for example preloading during a loading screen. |
| Fails when / edge behavior | Promoting an unknown `TextureID` returns a nil `TextureID`. |

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> TextureID` |
| Params | `source_hash: u64` |
| Returns | `TextureID` |
| Use when | Reserving by a precomputed path hash. |
| Fails when / edge behavior | Returns a nil `TextureID` when no texture is registered for the hash. |

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> TextureID` |
| Params | `source_hash: u64, source: S` |
| Returns | `TextureID` |
| Use when | The `texture_reserve!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `TextureID` when the file is missing. |

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn drop(&self, id: TextureID) -> bool` |
| Params | `id: TextureID` |
| Returns | `bool` |
| Use when | Releasing a texture the game no longer draws. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `pub fn is_loaded(&self, id: TextureID) -> bool` |
| Params | `id: TextureID` |
| Returns | `bool` |
| Use when | Polling whether the async decode/upload has finished before relying on the texture. |
| Fails when / edge behavior | Returns `false` while the upload is pending or when the ID is unknown. |

### `texture_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_load!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `TextureID` |
| Use when | Macro form of `load`. A literal path hashes at compile time; an expression path calls `load`. |
| Fails when / edge behavior | Returns a nil `TextureID` when the file is missing. |

### `texture_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_reserve!(ctx.res, source_or_id)` |
| Params | `ctx.res, source_or_id` |
| Returns | `TextureID` |
| Use when | Macro form of `reserve`. |
| Fails when / edge behavior | Promoting an unknown `TextureID` returns a nil `TextureID`. |

### `texture_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_drop!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `drop`. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `texture_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Textures()` |
| Signature | `texture_is_loaded!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `is_loaded`. |
| Fails when / edge behavior | Returns `false` while the upload is pending or when the ID is unknown. |
