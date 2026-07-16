# Meshes Module

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

## Purpose

`ctx.res.Meshes()` turns a model path or `Mesh3D` geometry into a `MeshID` that 3D nodes render. Loads return the ID the same frame and never block; the async decode and GPU upload finish in the background. Beyond loading, the module can build meshes from CPU vertex data, read a mesh's geometry back, and overwrite it, which is what procedural geometry and runtime mesh editing need.

## Use Cases

- Streaming level geometry: `mesh_load!(ctx.res, "res://meshes/pillar.glb")` and assign the `MeshID` to a mesh node.
- Procedural geometry: build a `Mesh3D` (terrain patch, generated wall) and register it with `mesh_create!`.
- Runtime mesh editing: `mesh_get_data!` to read CPU vertices, deform them, then `mesh_write!` the modified `Mesh3D` back into the same ID.
- Deforming built-in primitives: `get_data` returns canonical CPU geometry for engine preset meshes immediately, so a script can start from a cube or sphere and reshape it.
- Decoding downloaded models: `create_from_bytes` for engine `PMESH` or glTF/GLB mesh index 0.
- Preloading and memory control: `mesh_reserve!` to pin a mesh, `mesh_is_loaded!` to poll readiness, `mesh_drop!` to free it.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Meshes()`
- Mesh loads return a `MeshID` immediately and do not block the frame; the renderer uses the mesh once async decode/upload completes.
- Geometry type: `perro_render_bridge::Mesh3D`.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when mesh data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Meshes().create_from_bytes(bytes)` | `MeshID` | Decodes `PMESH` or glTF/GLB mesh index `0`. |
| `mesh_create_from_bytes!(ctx.res, bytes)` | `MeshID` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Load a mesh at init and, once the upload lands, read its geometry back for a helper.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let mesh = mesh_load!(ctx.res, "res://meshes/player.glb");
        self.inspect(ctx, mesh);
    }
});

methods!({
    fn inspect(&self, ctx: &mut ScriptContext<'_, API>, mesh: MeshID) {
        if mesh_is_loaded!(ctx.res, mesh) {
            if let Some(data) = mesh_get_data!(ctx.res, mesh) {
                // Deform `data` and push it back with mesh_write! if needed.
                let _ = data;
            }
        }
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> MeshID` |
| Params | `source: S` |
| Returns | `MeshID` |
| Use when | Getting an ID now; the renderer uses it once the async load finishes. |
| Fails when / edge behavior | Returns a nil `MeshID` when the path is empty; a missing file resolves to nil after the async load fails. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> MeshID` |
| Params | `source_hash: u64` |
| Returns | `MeshID` |
| Use when | A precomputed path hash is available and the source string is not needed. |
| Fails when / edge behavior | Returns a nil `MeshID` when no mesh is registered for the hash. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> MeshID` |
| Params | `source_hash: u64, source: S` |
| Returns | `MeshID` |
| Use when | The `mesh_load!` literal path builds a compile-time hash and passes the source for first-load resolution. |
| Fails when / edge behavior | Returns a nil `MeshID` when the file is missing. |

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn reserve<A: MeshReserveArg>(&self, arg: A) -> MeshID` |
| Params | `arg: A` (a path source, or an existing `MeshID` to promote) |
| Returns | `MeshID` |
| Use when | Pinning a mesh so it stays resident, for example preloading during a loading screen. |
| Fails when / edge behavior | Promoting an unknown `MeshID` returns a nil `MeshID`. |

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> MeshID` |
| Params | `source_hash: u64` |
| Returns | `MeshID` |
| Use when | Reserving by a precomputed path hash. |
| Fails when / edge behavior | Returns a nil `MeshID` when no mesh is registered for the hash. |

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> MeshID` |
| Params | `source_hash: u64, source: S` |
| Returns | `MeshID` |
| Use when | The `mesh_reserve!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `MeshID` when the file is missing. |

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn drop(&self, id: MeshID) -> bool` |
| Params | `id: MeshID` |
| Returns | `bool` |
| Use when | Releasing a mesh the game no longer renders. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn create(&self, data: Mesh3D) -> MeshID` |
| Params | `data: Mesh3D` |
| Returns | `MeshID` |
| Use when | Registering procedural geometry built in code. |
| Fails when / edge behavior | Returns a nil `MeshID` when the mesh data is invalid. |

### `get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn get_data(&self, id: MeshID) -> Option<Mesh3D>` |
| Params | `id: MeshID` |
| Returns | `Option<Mesh3D>` |
| Use when | Reading a mesh's CPU geometry to inspect or deform it. |
| Fails when / edge behavior | Built-in preset IDs return canonical CPU vertices, indices, one surface range, `uv`, and matching `paint_uv` immediately, including before renderer upload completes. Other IDs return `None` when CPU mesh data is unavailable, stale, or the target type does not match. |

### `write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn write(&self, id: MeshID, data: Mesh3D) -> bool` |
| Params | `id: MeshID, data: Mesh3D` |
| Returns | `bool` |
| Use when | Overwriting an existing mesh's geometry, for example after deforming it. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or the data is invalid. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `pub fn is_loaded(&self, id: MeshID) -> bool` |
| Params | `id: MeshID` |
| Returns | `bool` |
| Use when | Polling whether the async decode/upload has finished. |
| Fails when / edge behavior | Returns `false` while the upload is pending or when the ID is unknown. |

### `mesh_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_load!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `MeshID` |
| Use when | Macro form of `load`. A literal path hashes at compile time; an expression path calls `load`. |
| Fails when / edge behavior | Returns a nil `MeshID` when the file is missing. |

### `mesh_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_reserve!(ctx.res, source_or_id)` |
| Params | `ctx.res, source_or_id` |
| Returns | `MeshID` |
| Use when | Macro form of `reserve`. |
| Fails when / edge behavior | Promoting an unknown `MeshID` returns a nil `MeshID`. |

### `mesh_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_drop!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `drop`. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `mesh_create`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_create!(ctx.res, data)` |
| Params | `ctx.res, data` |
| Returns | `MeshID` |
| Use when | Macro form of `create`. |
| Fails when / edge behavior | Returns a nil `MeshID` when the mesh data is invalid. |

### `mesh_get_data`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_get_data!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `Option<Mesh3D>` |
| Use when | Macro form of `get_data`. |
| Fails when / edge behavior | Same built-in preset behavior and CPU-data limits as `get_data`. |

### `mesh_write`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_write!(ctx.res, id, data)` |
| Params | `ctx.res, id, data` |
| Returns | `bool` |
| Use when | Macro form of `write`. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or the data is invalid. |

### `mesh_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Meshes()` |
| Signature | `mesh_is_loaded!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `is_loaded`. |
| Fails when / edge behavior | Returns `false` while the upload is pending or when the ID is unknown. |
