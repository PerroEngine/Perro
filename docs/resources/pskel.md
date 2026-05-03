# `.pskel` Format

`.pskel` is a **Perro Skeleton** resource. It stores bone data used by `Skeleton3D`.
In dev, `.pskel` is **text** (authored by hand if needed). During static builds it is
compiled into a **binary** `.pskel` with the same extension.

## Usage

- Dev/runtime loads bones from glTF:
  - `res://models/rig.gltf:skeleton[0]`
- Static builds embed `.pskel` and keep the same lookup key:
  - `res://models/rig.gltf:skeleton[0]`
- You can also reference `res://models/rig.pskel` directly.

## Text Format (Dev)

Author a `.pskel` as text in `res/`:

```text
[bone "Root"]
    parent = -1
    rest_pos = (0, 0, 0)
    rest_scale = (1, 1, 1)
    rest_rot = (0, 0, 0, 1)
    inv_pos = (0, 0, 0)
    inv_scale = (1, 1, 1)
    inv_rot = (0, 0, 0, 1)
[/bone]
```

Notes:

- `parent` is the parent bone index (`-1` for root).
- `rest_*` are the local rest transform.
- `inv_*` are the inverse bind transform.
- Missing fields default to identity transforms.

## Binary Layout (Static)

Header:

- Magic: `PSKEL` (5 bytes)
- Version: `u32` (currently `1`)
- Bone count: `u32`
- Raw size (bytes): `u32`
- Compressed payload: zlib

Raw bone payload (repeated `bone_count` times):

- `name_len: u32`
- `name_bytes: [u8; name_len]` (UTF-8)
- `parent: i32` (`-1` for root)
- `rest`: Transform3D (pos xyz, scale xyz, rot xyzw)
- `inv_bind`: Transform3D (pos xyz, scale xyz, rot xyzw)

Transform3D layout (each `f32` LE):

- position: `x, y, z`
- scale: `x, y, z`
- rotation (quat): `x, y, z, w`

## Example References

Script usage:

```rust
let bones = skeleton_load_bones!(res, "res://models/rig.pskel");
with_node_mut!(ctx.run, Skeleton3D, node_id, |skel| {
    skel.bones = bones;
});
```


