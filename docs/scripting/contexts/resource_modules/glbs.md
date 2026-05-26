# GLBs Module

## Page Map

| Header            | Link                                  |
| ----------------- | ------------------------------------- |
| Overview          | [Overview](#overview)                 |
| Context           | [Context](#context)                   |
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

## Overview

This resource module belongs to `ctx.res` and inspects `.glb` / `.gltf` containers.
Use it when one container stores many sub-assets and code needs a valid indexed path like `:mesh[5]`.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Glbs()`
- Sub-asset suffixes such as `:mesh[0]` are accepted and stripped before inspect, to make sure we look at the entire glb

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let source = "res://props/variant_pack.glb";
        let mesh_count = mesh_count!(ctx.res, source).unwrap_or(0);
        if mesh_count > 0 {
            let requested_index = 17usize;
            let index = requested_index % mesh_count;
            let mesh = mesh_load!(ctx.res, format!("{source}:mesh[{index}]"));
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
