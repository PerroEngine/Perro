# Node Types

This page lists the built-in node types and their purpose. Nodes store **data-only** state.
Rendering and resource loading are handled by the runtime and `ResourceContext`.

## 2D Nodes

`Node2D`

- Base transform for 2D nodes (position, rotation, scale, z_index, visible).

`Sprite2D`

- Renders a textured quad.
- Holds a `TextureID` (not raw pixels). Use `Texture` module to load.

`Camera2D`

- Active 2D camera (position/rotation/zoom).
- Supports camera post-processing via `post_processing` (see "Camera Post-Processing" below).

## 3D Nodes

`Node3D`

- Base transform for 3D nodes (position, rotation, scale, visible).

`MeshInstance3D`

- Renders a mesh with a material.
- Holds `MeshID` and `MaterialID` instead of raw mesh/material data.
- Reason: resource IDs allow caching, reuse, and async GPU upload.
- Skinning: if the mesh has vertex weights, it can be **deformed by a Skeleton3D**.
- Runtime link: `skeleton: NodeID` points to the `Skeleton3D` node that supplies bone transforms.
- Scene authoring: `skeleton = "NodeName"` uses the **scene node name** and is resolved to a `NodeID` at load time.
- Skinning only works if the mesh has proper vertex weights (`JOINTS_0/WEIGHTS_0`).

`TerrainInstance3D`

- Runtime terrain renderer instance (terrain data is managed through `ResourceContext::Terrain()`).

`Camera3D`

- Active 3D camera with projection settings.
- Supports camera post-processing via `post_processing` (see "Camera Post-Processing" below).

`ParticleEmitter3D`

- 3D particle emitter driven by a particle profile.

Lights:

- `AmbientLight3D`
- `RayLight3D`
- `PointLight3D`
- `SpotLight3D`

`Skeleton3D`

- Holds `Vec<Bone3D>` (data-only).
- Bones are loaded via `ResourceContext::Skeletons().load_bones(source)`.
- Typical flow: scene specifies a `skeleton` path, and scene loader fills `bones`.

## Camera Post-Processing

Post-processing is configured per camera using `post_processing`.
See `docs/resources/postprocess.md` for full details and examples.

## Visual Accessibility

Visual accessibility is configured globally through `ResourceContext` (not per-camera).
It runs after camera and global post-processing as the final pass.
See [Visual Accessibility](../resources/visual_accessibility.md).

## Bone3D

`Bone3D` fields:

- `name`: bone name
- `parent`: parent bone index (`-1` for root)
- `rest`: rest transform (local)
- `inv_bind`: inverse bind transform

## Skeleton Load Patterns

From scene:

```
[Rig]
    [Skeleton3D]
        skeleton = "res://models/rig.gltf:skeleton[0]"
    [/Skeleton3D]
[/Rig]

[SkinnedMesh]
    [MeshInstance3D]
        mesh = "res://models/rig.gltf:mesh[0]"
        material = "res://materials/skin.pmat"
        skeleton = "Rig"
    [/MeshInstance3D]
[/SkinnedMesh]
```

From script:

```rust
let bones = skeleton_load_bones!(res, "res://models/rig.gltf:skeleton[0]");
with_node_mut!(ctx, Skeleton3D, node_id, |skel| {
    skel.bones = bones;
});
```

Swapping a mesh’s skeleton at runtime (Mesh must have vertex weights):

```rust
with_node_mut!(ctx, MeshInstance3D, mesh_id, |mesh| {
    mesh.skeleton = new_skeleton_node_id;
});
```
