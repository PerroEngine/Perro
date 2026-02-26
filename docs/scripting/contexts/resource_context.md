# Resource Context

Type:
- `res: &ResourceContext<'_, RS>`

Purpose:
- Load or create render resources from scripts.
- Keep resource access shared by source path so repeated use reuses cached IDs.

Accessors:
- `res.Textures()`
- `res.Meshes()`
- `res.Materials()`

## Resource Macros

### `load_texture!(res, source) -> TextureID`
- `source`: `&str | String`.
- Loads a texture and returns its `TextureID`.

### `reserve_texture!(res, source) -> TextureID`
- `source`: `&str | String`.
- Reserves texture identity by source path.

### `drop_texture!(res, source) -> bool`
- `source`: `&str | String`.
- Drops the source mapping from the runtime cache.

### `load_mesh!(res, source) -> MeshID`
- `source`: `&str | String`.
- Loads a mesh and returns its `MeshID`.

### `reserve_mesh!(res, source) -> MeshID`
- `source`: `&str | String`.
- Reserves mesh identity by source path.

### `drop_mesh!(res, source) -> bool`
- `source`: `&str | String`.
- Drops the source mapping from the runtime cache.

### `load_material!(res, source) -> MaterialID`
- `source`: `&str | String`.
- Loads a material source and returns its `MaterialID`.

### `reserve_material!(res, source) -> MaterialID`
- `source`: `&str | String`.
- Reserves material identity by source path.

### `drop_material!(res, source) -> bool`
- `source`: `&str | String`.
- Drops the source mapping from the runtime cache.

### `create_material!(res, material) -> MaterialID`
- `material`: `Material3D`.
- Creates a runtime material from data.

## Module Methods

### `res.Textures().load(source) -> TextureID`
### `res.Textures().reserve(source) -> TextureID`
### `res.Textures().drop(source) -> bool`

### `res.Meshes().load(source) -> MeshID`
### `res.Meshes().reserve(source) -> MeshID`
### `res.Meshes().drop(source) -> bool`

### `res.Materials().load(source) -> MaterialID`
### `res.Materials().reserve(source) -> MaterialID`
### `res.Materials().drop(source) -> bool`
### `res.Materials().create(material) -> MaterialID`
- `material`: `Material3D`.

## Simple Example

```rust
let texture_id = load_texture!(res, "res://textures/smoke.png");
let mesh_id = load_mesh!(res, "res://meshes/rock.glb");
let material_id = load_material!(res, "res://materials/smoke.pmat");
let _reserved = reserve_texture!(res, "res://textures/smoke.png");
let _ = drop_mesh!(res, "res://meshes/old.glb");
```
