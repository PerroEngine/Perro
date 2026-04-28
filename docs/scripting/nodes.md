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

Physics 2D:

- `CollisionShape2D`
- `StaticBody2D`
- `RigidBody2D`
- `Area2D`
- `CollisionShape2D` should be authored as a child of `StaticBody2D` or `RigidBody2D`.

## 3D Nodes

`Node3D`

- Base transform for 3D nodes (position, rotation, scale, visible).

`MeshInstance3D`

- Renders a mesh with per-surface material bindings.
- Holds `MeshID` and `surfaces: Vec<MeshSurfaceBinding>` instead of raw mesh/material data.
- Reason: resource IDs allow caching, reuse, and async GPU upload.
- `MeshSurfaceBinding` supports:
  - `material: Option<MaterialID>`
  - `modulate` (RGBA multiplier)
  - `overrides` (named material parameter overrides)
  - flat/smooth override names: `flat_shading`/`flatShading`, `shade_flat`/`shadeFlat`, `shade_smooth`/`shadeSmooth`
- Scene authoring supports `surfaces = [ ... ]` where each entry can be:
  - a material source string
  - an object with `material`, `modulate`, and `overrides`
- Legacy `material = ...` is still accepted and maps to surface index `0`.
- Skinning: if the mesh has vertex weights, it can be **deformed by a Skeleton3D**.
- Runtime link: `skeleton: NodeID` points to the `Skeleton3D` node that supplies bone transforms.
- Scene authoring: `skeleton = "NodeName"` uses the **scene node name** and is resolved to a `NodeID` at load time.
- Skinning only works if the mesh has proper vertex weights (`JOINTS_0/WEIGHTS_0`).

`Camera3D`

- Active 3D camera with projection settings.
- Supports camera post-processing via `post_processing` (see "Camera Post-Processing" below).

`ParticleEmitter3D`

- 3D particle emitter driven by a particle profile.

`AnimationPlayer`

- Plays an `AnimationClip` resource and applies tracks to bound scene nodes.
- `animation` points to clip source/ID; tracks are mapped by object name via `bindings`.
- `playback` supports `once`, `loop`, `boomerang`.
- Runtime control is exposed through Animation macros in Runtime Context.

Lights:

- `AmbientLight3D`
- `Sky3D`
- `RayLight3D`
- `PointLight3D`
- `SpotLight3D`

Physics 3D:

- `CollisionShape3D`
- `StaticBody3D`
- `RigidBody3D`
- `Area3D`
- `CollisionShape3D` should be authored as a child of `StaticBody3D` or `RigidBody3D`.
- `CollisionShape3D` supports primitive `shape` and mesh-backed `trimesh` source.
- Trimesh source format: `res://path/to/model.glb:mesh[0]` (mesh index optional, default `0`).

`Skeleton3D`

- Holds `Vec<Bone3D>` (data-only).
- Bones are loaded via `ResourceContext::Skeletons().load_bones(source)`.
- Typical flow: scene specifies a `skeleton` path, and scene loader fills `bones`.

## UI Nodes

UI nodes inherit from `UiRoot` in the node registry:

- `UiPanel`
- `UiButton`
- `UiLabel`
- `UiHBox`
- `UiVBox`
- `UiGrid`

UI positions and sizes resolve against the parent UI rect.
Root UI nodes use the virtual viewport as parent.
Each axis can be pixels or percent, so `UiVector2::percent(50.0, 50.0)` means parent center.
All UI nodes can have children; `UiHBox`, `UiVBox`, and `UiGrid` only add automatic child placement.

See [UI Nodes](ui.md).

## Scene Authoring Templates

For copy/paste scene node authoring templates (with all exposed fields, including nil/empty-default fields), see:

- [Scene Node Templates](scene_node_templates.md)

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

```text
[Rig]
    [Skeleton3D]
        skeleton = "res://models/rig.gltf:skeleton[0]"
    [/Skeleton3D]
[/Rig]

[SkinnedMesh]
    [MeshInstance3D]
        mesh = "res://models/rig.gltf:mesh[0]"
        surfaces = [
            {
                material = "res://materials/skin.pmat"
                modulate = (1, 1, 1, 1)
            }
        ]
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

Swapping a mesh's skeleton at runtime (Mesh must have vertex weights):

```rust
with_node_mut!(ctx, MeshInstance3D, mesh_id, |mesh| {
    mesh.skeleton = new_skeleton_node_id;
});
```
