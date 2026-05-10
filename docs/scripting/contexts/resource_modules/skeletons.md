# Skeletons Module

Access:

- `res.Skeletons()`

Macros:

- `skeleton_load_bones!(res, source) -> Vec<Bone3D>`

Methods:

- `res.Skeletons().load_bones_2d(source) -> Vec<Bone2D>`
- `res.Skeletons().load_bones_3d(source) -> Vec<Bone3D>`
- `res.Skeletons().load_bones(source) -> Vec<Bone3D>` legacy 3D alias

What `load_bones` does:

- Returns a **copy** of cached bone data for `source`.
- If not cached yet, loads and decodes the skeleton, then caches it.
- The cache key is the exact `source` string.

Supported sources:

- `res://path/to/rig.glb:skeleton[0]` (parsed from glTF)
- `res://path/to/rig.pskel2d`
- `res://path/to/rig.pskel3d`
- `res://path/to/rig.pskel` legacy 3D

Important behavior:

- Bones are **data-only**. Mutate `Skeleton2D.bones` or `Skeleton3D.bones` for runtime edits.
- This module does **not** return a handle/ID; it returns data by value.
- Repeated calls return a new copy (safe to edit without affecting cache).
- To skin a mesh, bind a `MeshInstance3D` to a `Skeleton3D` node (scene `skeleton = @NodeName`).

Example:

```rust
use perro_api::prelude::*;

let bones = skeleton_load_bones!(res, "res://models/rig.glb:skeleton[0]");
with_node_mut!(ctx.run, Skeleton3D, self_id, |skel| {
    skel.bones = bones;
});
```

glTF sub-asset access:

- `res://path/to/model.gltf:skeleton[0]`
- `res://path/to/model.glb:skeleton[1]`

Use the `:skeleton[index]` suffix to target a specific skeleton/skin inside a glTF/glb.

Direct `.pskel` sources:
- `res://path/to/rig.pskel2d`
- `res://path/to/rig.pskel3d`
- `res://path/to/rig.pskel` legacy 3D


