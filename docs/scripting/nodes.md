# Node Types

This page lists the built-in node types and their purpose. Nodes store **data-only** state.
Rendering and resource loading are handled by the runtime and `ResourceWindow`.

## Base Node

`Node`

- Generic non-spatial scene node.
- Use it as a script/root grouping node when no 2D, 3D, UI, or resource data is needed.

## 2D Nodes

`Node2D`

- Base transform for 2D nodes (position, rotation, scale, z_index, visible).

`Sprite2D`

- Renders a textured quad.
- Holds a `TextureID` (not raw pixels). Use `Texture` module to load.
- `texture_region` selects an atlas rect.

`AnimatedSprite2D`

- Renders a sprite sheet.
- Uses normal `texture = "res://..."` like `Sprite2D`.
- `animations` lists named strip/grid layouts with `frame_size`, `frame_count`, `columns`, and `fps`.
- Uses `current_animation`, `current_frame`, `fps_scale`, `playing`, and `looping`.
- Advances during internal update and renders the current atlas frame.
- Omit `columns` for a left-to-right strip.
- Set `columns` when frames wrap to more than one row in a grid.

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
- Shared-skeleton mesh reuse works when meshes follow the same rig contract: same joint order/indices and compatible weights.
- Automatic retargeting between mismatched rigs is not implemented.
- Mesh LOD is automatic for authored meshes.
- Dynamic/dev load builds LODs on load; static build packs LODs into `.pmesh` v8.
- Meshes with joints/weights skip LOD generation.
- Surface/material slots stay stable across LODs.
- Current switch distances are radius-scaled (`36x`, `72x`, `144x` mesh bounds radius).
- `MeshInstance3D` does not expose per-node LOD controls yet.

`MultiMeshInstance3D`

- Renders many copies of one mesh.
- Uses shared mesh/material surface bindings.
- `instances` stores per-instance position and rotation.
- Use it for repeated static props, foliage, debris, or crowd-like non-skinned copies.

`Camera3D`

- Active 3D camera with projection settings.
- Supports camera post-processing via `post_processing` (see "Camera Post-Processing" below).

`ParticleEmitter3D`

- 3D particle emitter driven by a particle profile.

`ParticleEmitter2D`

- 2D particle emitter driven by a particle profile.
- Reads `.ppart` `x` and `y`; ignores `z`.

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
- Bones are loaded via `ResourceWindow::Skeletons().load_bones(source)`.
- Typical flow: scene specifies a `skeleton` path, and scene loader fills `bones`.

`BoneAttachment3D`

- Follows one bone on a `Skeleton3D`.
- Fields:
  - `skeleton`: scene node name of the `Skeleton3D`.
  - `bone` or `bone_index`: zero-based index into `Skeleton3D.bones`.
- Runtime resolves `skeleton = "NodeName"` to a `NodeID` at load time.
- Each internal update computes skeleton global transform + bone pose chain transform.
- Attachment's global 3D transform is set to that bone transform.
- Children of the attachment inherit that transform.
- Use it for held gear, muzzle flashes, hit markers, socketed VFX, or any node that should follow a bone.

`PhysicsBoneChain3D`

- Simulates one bone chain on a `Skeleton3D` during fixed update.
- Scene fields:
  - `skeleton`: scene node name of the `Skeleton3D`.
  - `bone` or `bone_index`: zero-based end bone index.
  - `chain_length`: number of links back from end bone.
  - `gravity`: world-space acceleration.
  - `damping`, `stiffness`, `radius`, `collisions`, `iterations`.
- Uses Verlet-style points, pins the root of the selected chain, writes solved positions back into bone `pose`.
- Reacts to skeleton movement because simulation state is kept in world space.

`BoneCollider3D`

- Static collider source for `PhysicsBoneChain3D`.
- Add `CollisionShape3D` children, like `StaticBody3D`.
- Chain collisions support all `CollisionShape3D` child shapes.
- Primitive shapes use local shape pushout; `TriMesh` uses a conservative node-space sphere fallback.

`IKTarget3D`

- Solves a CCD IK chain on one `Skeleton3D`.
- Fields:
  - `skeleton`: scene node name of the `Skeleton3D`.
  - `bone` or `bone_index`: zero-based end bone index.
  - `chain_length`: parent chain length to solve.
  - `iterations`: CCD pass count.
  - `tolerance`: stop distance in skeleton-local units.
  - `weight`: solve blend `0..1`.
  - `match_rotation`: match target rotation on end bone.
- Writes solved transforms into bone `pose`; keeps bone `rest` unchanged.

## UI Nodes

UI nodes inherit from `UiBox` in the node registry:

- `UiBox`
- `UiPanel`
- `UiButton`
- `UiLabel`
- `UiTextBox`
- `UiTextBlock`
- `UiLayout`
- `UiHLayout`
- `UiVLayout`
- `UiGrid`
- `UiTreeList`

UI positions and sizes resolve against the parent UI rect.
Root UI nodes use the virtual viewport as parent.
Each axis can be pixels or ratio, so `UiVector2::ratio(0.5, 0.5)` means parent center.
All UI nodes can have children.
`UiBox` is the invisible generic container.
`UiPanel` draws a styled box.
`UiButton` draws an interactive styled box and emits configured signals.
`UiLabel` draws text.
`UiTextBox` edits one line of text.
`UiTextBlock` edits multi-line text.
`UiLayout`, `UiHLayout`, `UiVLayout`, and `UiGrid` add automatic child placement.
`UiTreeList` adds hierarchical row placement from referenced UI node ids.

See [UI Nodes](ui.md).

## Scene Authoring Templates

For copy/paste scene node authoring templates (with all exposed fields, including nil/empty-default fields), see:

- [Scene Node Templates](scene_node_templates.md)

## Camera Post-Processing

Post-processing is configured per camera using `post_processing`.
See `docs/resources/postprocess.md` for full details and examples.

## Visual Accessibility

Visual accessibility is configured globally through `ResourceWindow` (not per-camera).
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
with_node_mut!(ctx.run, Skeleton3D, node_id, |skel| {
    skel.bones = bones;
});
```

Swapping a mesh's skeleton at runtime (mesh must have vertex weights and match the skeleton rig contract):

```rust
with_node_mut!(ctx.run, MeshInstance3D, mesh_id, |mesh| {
    mesh.skeleton = new_skeleton_node_id;
});
```

## Bone Attachment Pattern

Use `BoneAttachment3D` when a normal child transform is not enough.
A normal child follows a scene node.
`BoneAttachment3D` follows a bone inside a `Skeleton3D`.

The binding is two-part:

- `skeleton = "CharacterSkeleton"` binds to the skeleton scene node.
- `bone = 15` binds to bone index `15` inside `Skeleton3D.bones`.

Then parent child nodes under the attachment.
The child can still have local offset/rotation/scale.
Example: sword in hand.

```text
[Character]
    [Node3D]
        position = (0, 0, 0)
    [/Node3D]
[/Character]

[CharacterSkeleton]
parent = @Character
    [Skeleton3D]
        skeleton = "res://characters/knight.glb:skeleton[0]"
    [/Skeleton3D]
[/CharacterSkeleton]

[CharacterMesh]
parent = @Character
    [MeshInstance3D]
        mesh = "res://characters/knight.glb:mesh[0]"
        skeleton = "CharacterSkeleton"
    [/MeshInstance3D]
[/CharacterMesh]

[RightHandSocket]
parent = @Character
    [BoneAttachment3D]
        skeleton = "CharacterSkeleton"
        bone = 15
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/BoneAttachment3D]
[/RightHandSocket]

[Sword]
parent = @RightHandSocket
    [MeshInstance3D]
        mesh = "res://weapons/sword.glb:mesh[0]"
        material = "res://weapons/sword.pmat"
        [Node3D]
            position = (0.05, 0.0, 0.0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/MeshInstance3D]
[/Sword]
```

Notes:

- Bone index comes from imported skeleton order.
- Meshes can share one skeleton when their `JOINTS_0/WEIGHTS_0` indices are authored for that same skeleton order.
- This is shared-rig reuse, not automatic retargeting. Perro does not currently remap bone names or solve rest-pose differences for mismatched rigs.
- `bone = -1` or missing `skeleton` disables attachment update.
- If index is out of range, attachment keeps its current transform.
- Child nodes render/use physics from attachment transform like any other parented node.



