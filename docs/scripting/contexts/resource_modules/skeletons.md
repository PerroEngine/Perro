# Skeletons Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `load_bones_2d` | [`load_bones_2d`](#load_bones_2d) |
| `load_bones_3d` | [`load_bones_3d`](#load_bones_3d) |
| `load_bones` | [`load_bones`](#load_bones) |
| `skeleton_load_bones` | [`skeleton_load_bones`](#skeleton_load_bones) |

## Purpose

`ctx.res.Skeletons()` loads the bone hierarchy for a rigged character as a plain `Vec<Bone2D>` or `Vec<Bone3D>`. Use it when a script needs to build or reconfigure a `Skeleton2D` / `Skeleton3D` node from bone data at runtime rather than authoring the rig in a scene file: swapping a character's rig, sharing one skeleton across many meshes, or generating bones procedurally.

## Use Cases

- Runtime character assembly: load a rig with `skeleton_load_bones!(ctx.res, "res://chars/hero.glb")` and feed the bones into a `Skeleton3D` node.
- 2D cutout puppets: build a `Skeleton2D` from `load_bones_2d` for paper-doll style animation.
- Shared rigs: load one `Vec<Bone3D>` and reuse it across several skinned meshes that share the same skeleton.
- Modular characters: decode a rig from bytes downloaded or embedded in a save with `skeleton_load_bones_3d_from_bytes!`.
- Rig inspection: read bone names and parents from the returned vector before wiring animation.

## Ownership And Choice

The resource cache owns skeleton data; skeleton/player nodes own pose and playback state. Inject or load a stable skeleton asset for the rig, then drive it through animation or IK. Use runtime bytes only when the rig arrives dynamically. Do not rebuild the skeleton resource to make a per-frame pose change.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Skeletons()`
- Return types: `perro_nodes::skeleton_2d::Bone2D`, `perro_nodes::skeleton_3d::Bone3D`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when skeleton data is already in memory, for example a rig streamed over the network or embedded in save data.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Skeletons().load_bones_2d_from_bytes(bytes)` | `Vec<Bone2D>` | Decodes packed 2D skeleton bytes. |
| `ctx.res.Skeletons().load_bones_3d_from_bytes(bytes)` | `Vec<Bone3D>` | Decodes packed 3D skeleton bytes. |
| `skeleton_load_bones_2d_from_bytes!(ctx.res, bytes)` | `Vec<Bone2D>` | Macro form. |
| `skeleton_load_bones_3d_from_bytes!(ctx.res, bytes)` | `Vec<Bone3D>` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Load a 3D rig at init and hand the bones to a helper that would configure a skeleton node.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let bones = skeleton_load_bones!(ctx.res, "res://characters/hero.glb");
        self.build_rig(ctx, bones);
    }
});

methods!({
    fn build_rig(&self, ctx: &mut ScriptContext<'_, API>, bones: Vec<Bone3D>) {
        // Apply `bones` to a Skeleton3D node here.
        let _ = (ctx, bones);
    }
});
```

## API Reference

### `load_bones_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones_2d<S: ResPathSource>(&self, source: S) -> Vec<Bone2D>` |
| Params | `source: S` |
| Returns | `Vec<Bone2D>` |
| Use when | Loading a 2D rig for a `Skeleton2D` node. |
| Fails when / edge behavior | Returns an empty vector when the source is missing or holds no 2D skeleton. |

### `load_bones_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones_3d<S: ResPathSource>(&self, source: S) -> Vec<Bone3D>` |
| Params | `source: S` |
| Returns | `Vec<Bone3D>` |
| Use when | Loading a 3D rig for a `Skeleton3D` node. |
| Fails when / edge behavior | Returns an empty vector when the source is missing or holds no 3D skeleton. |

### `load_bones`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones<S: ResPathSource>(&self, source: S) -> Vec<Bone3D>` |
| Params | `source: S` |
| Returns | `Vec<Bone3D>` |
| Use when | The common 3D case; the `skeleton_load_bones!` macro expands to this. |
| Fails when / edge behavior | Returns an empty vector when the source is missing or holds no 3D skeleton. |

### `skeleton_load_bones`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `skeleton_load_bones!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `Vec<Bone3D>` |
| Use when | Macro form of `load_bones`. |
| Fails when / edge behavior | Returns an empty vector when the source is missing or holds no 3D skeleton. |
