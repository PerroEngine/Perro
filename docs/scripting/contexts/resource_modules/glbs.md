# GLBs Module

## Page Map

| Header            | Link                                  |
| ----------------- | ------------------------------------- |
| Purpose           | [Purpose](#purpose)                   |
| Use Cases         | [Use Cases](#use-cases)               |
| Context           | [Context](#context)                   |
| Practical Example | [Practical Example](#practical-example) |
| API Reference     | [API Reference](#api-reference)       |
| `GltfInfo`        | [`GltfInfo`](#gltfinfo)               |
| `inspect`         | [`inspect`](#inspect)                 |
| `mesh_count`      | [`mesh_count`](#mesh_count)           |
| `material_count`  | [`material_count`](#material_count)   |
| `skeleton_count`  | [`skeleton_count`](#skeleton_count)   |
| `animation_count` | [`animation_count`](#animation_count) |
| `node_count`      | [`node_count`](#node_count)           |
| `scene_count`     | [`scene_count`](#scene_count)         |
| `texture_count`   | [`texture_count`](#texture_count)     |
| `animation_to_panim` | [`animation_to_panim`](#animation_to_panim) |
| `material_to_pmat` | [`material_to_pmat`](#material_to_pmat) |

## Purpose

`ctx.res.Glbs()` reads the table of contents of a `.glb` / `.gltf` container without loading any of it as scene nodes. A single container often packs many meshes, materials, skeletons, and animations; this module tells you how many of each it holds so code can build a valid indexed sub-asset path like `res://pack.glb:mesh[5]` or wrap an index safely.

## Use Cases

- Variant packs: a prop `.glb` holds many meshes, so `mesh_count!(ctx.res, source)` gives the count to pick or wrap-around one before `mesh_load!` with a `:mesh[i]` suffix.
- Character rig packs: read both `mesh_count` and `animation_count` from one `inspect` to pair a body mesh with a compatible clip.
- Content validation: check a downloaded model actually contains a skeleton or the expected material count before using it.
- Random cosmetic selection: use `texture_count` to roll a random skin index that is guaranteed in range.
- Authoring/tooling: extract a single animation or material out of a container to engine text with `animation_to_panim` / `material_to_pmat`.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Glbs()`
- Sub-asset suffixes such as `:mesh[0]` are accepted and stripped before inspect, to make sure we look at the entire glb

## Practical Example

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        // Pick a prop variant that is guaranteed in range.
        self.spawn_variant(ctx, "res://props/variant_pack.glb", 17);
    }
});

methods!({
    fn spawn_variant(&self, ctx: &mut ScriptContext<'_, API>, source: &str, requested: usize) {
        let mesh_count = mesh_count!(ctx.res, source).unwrap_or(0);
        if mesh_count > 0 {
            let index = requested % mesh_count;
            let mesh = mesh_load!(ctx.res, format!("{source}:mesh[{index}]"));
            let _ = mesh; // assign to a mesh node
        }
    }
});
```

## API Reference

### `GltfInfo`

`inspect` returns this struct when source can be read as glTF.

```rust
pub struct GltfInfo {
    pub mesh_count: usize,
    pub material_count: usize,
    pub skeleton_count: usize,
    pub animation_count: usize,
    pub node_count: usize,
    pub scene_count: usize,
    pub texture_count: usize,
}
```

Use it when code needs more than one count from same file.

```rust
let source = "res://characters/rig_pack.glb";
if let Some(info) = glb_inspect!(ctx.res, source) {
    if info.mesh_count > 0 && info.animation_count > 0 {
        let mesh_index = requested_mesh % info.mesh_count;
        let anim_index = requested_anim % info.animation_count;
        let mesh = mesh_load!(ctx.res, format!("{source}:mesh[{mesh_index}]"));
        let anim = animation_load!(ctx.res, format!("{source}:animation[{anim_index}]"));
    }
}
```

### `inspect`

| Field                      | Detail                                                                   |
| -------------------------- | ------------------------------------------------------------------------ |
| Access                     | `ctx.res.Glbs()`                                                         |
| Signature                  | `pub fn inspect<S: ResPathSource>(&self, source: S) -> Option<GltfInfo>` |
| Macro                      | `glb_inspect!(ctx.res, source)`                                          |
| Params                     | `&self, source: S`                                                       |
| Returns                    | `Option<GltfInfo>`                                                       |
| Use when                   | Use when code needs several counts from one glTF read.                   |
| Fails when / edge behavior | Returns `None` for non-glTF paths, missing files, or invalid glTF data.  |

### `mesh_count`

| Field                      | Detail                                                                                        |
| -------------------------- | --------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.Glbs()`                                                                              |
| Signature                  | `pub fn mesh_count<S: ResPathSource>(&self, source: S) -> Option<usize>`                      |
| Macro                      | `mesh_count!(ctx.res, source)`                                                                |
| Params                     | `&self, source: S`                                                                            |
| Returns                    | `Option<usize>`                                                                               |
| Use when                   | Use when one glTF stores many meshes and code needs count for wrap-around or indexed loading. |
| Fails when / edge behavior | Returns `None` when `inspect` cannot read the source.                                         |

### `material_count`

| Field     | Detail                                                                       |
| --------- | ---------------------------------------------------------------------------- |
| Access    | `ctx.res.Glbs()`                                                             |
| Signature | `pub fn material_count<S: ResPathSource>(&self, source: S) -> Option<usize>` |
| Macro     | `material_count!(ctx.res, source)`                                           |
| Params    | `&self, source: S`                                                           |
| Returns   | `Option<usize>`                                                              |

### `skeleton_count`

| Field     | Detail                                                                       |
| --------- | ---------------------------------------------------------------------------- |
| Access    | `ctx.res.Glbs()`                                                             |
| Signature | `pub fn skeleton_count<S: ResPathSource>(&self, source: S) -> Option<usize>` |
| Macro     | `skeleton_count!(ctx.res, source)`                                           |
| Params    | `&self, source: S`                                                           |
| Returns   | `Option<usize>`                                                              |

### `animation_count`

| Field     | Detail                                                                        |
| --------- | ----------------------------------------------------------------------------- |
| Access    | `ctx.res.Glbs()`                                                              |
| Signature | `pub fn animation_count<S: ResPathSource>(&self, source: S) -> Option<usize>` |
| Macro     | `animation_count!(ctx.res, source)`                                           |
| Params    | `&self, source: S`                                                            |
| Returns   | `Option<usize>`                                                               |

### `node_count`

| Field     | Detail                                                                   |
| --------- | ------------------------------------------------------------------------ |
| Access    | `ctx.res.Glbs()`                                                         |
| Signature | `pub fn node_count<S: ResPathSource>(&self, source: S) -> Option<usize>` |
| Macro     | `node_count!(ctx.res, source)`                                           |
| Params    | `&self, source: S`                                                       |
| Returns   | `Option<usize>`                                                          |

### `scene_count`

| Field     | Detail                                                                    |
| --------- | ------------------------------------------------------------------------- |
| Access    | `ctx.res.Glbs()`                                                          |
| Signature | `pub fn scene_count<S: ResPathSource>(&self, source: S) -> Option<usize>` |
| Macro     | `scene_count!(ctx.res, source)`                                           |
| Params    | `&self, source: S`                                                        |
| Returns   | `Option<usize>`                                                           |

### `texture_count`

| Field     | Detail                                                                      |
| --------- | --------------------------------------------------------------------------- |
| Access    | `ctx.res.Glbs()`                                                            |
| Signature | `pub fn texture_count<S: ResPathSource>(&self, source: S) -> Option<usize>` |
| Macro     | `texture_count!(ctx.res, source)`                                           |
| Params    | `&self, source: S`                                                          |
| Returns   | `Option<usize>`                                                             |

### `animation_to_panim`

| Field                      | Detail                                                                                                                     |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.Glbs()`                                                                                                           |
| Signature                  | `pub fn animation_to_panim<S: ResPathSource>(&self, source: S, fps: f32, animation_index: usize, skeleton_object: &str) -> Result<String, String>` |
| Params                     | `source: S, fps: f32, animation_index: usize, skeleton_object: &str`                                                       |
| Returns                    | `Result<String, String>`                                                                                                   |
| Use when                   | Tooling code needs to extract one glTF animation as engine `.panim` text bound to a named skeleton object.                 |
| Fails when / edge behavior | Returns `Err(String)` when the source cannot be read or the animation index is out of range.                              |

### `material_to_pmat`

| Field                      | Detail                                                                                                          |
| -------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.Glbs()`                                                                                               |
| Signature                  | `pub fn material_to_pmat<S: ResPathSource>(&self, source: S, material_index: usize) -> Result<String, String>` |
| Params                     | `source: S, material_index: usize`                                                                            |
| Returns                    | `Result<String, String>`                                                                                       |
| Use when                   | Tooling code needs to extract one glTF material as engine `.pmat` text.                                        |
| Fails when / edge behavior | Returns `Err(String)` when the source cannot be read or the material index is out of range.                   |
