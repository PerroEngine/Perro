# Node Types

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| Use Cases     | [Use Cases](#use-cases)         |
| Example       | [Example](#example)             |
| Reference     | [Reference](#reference)         |
| 3D Mesh Flips | [3D Mesh Flips](#3d-mesh-flips) |

## Purpose

Use `Node Types` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# Node Types

This page lists the built-in node types and their purpose. Nodes store **data-only** state.
Rendering and resource loading are handled by the runtime and `ResourceWindow`.

## Base Node

`Node`

- Generic non-spatial scene node.
- Use it as a script/root grouping node when no 2D, 3D, UI, or resource data is needed.

## Resource Nodes

`Webcam`

- Live webcam capture source for camera stream nodes.
- Does not draw by itself.
- Use it as `CameraStream2D.camera`, `CameraStream3D.camera`, or `UiCameraStream.camera`.
- `slot` is the device slot string. Empty uses the default device at index `0`.
- `resolution`, `width`, `height`, `fps`, `mirror`, `cpu_frames`, and `enabled` configure capture.
- The stream path opens capture automatically while the referenced `Webcam` node is enabled and visible.
- The stream path closes capture when the node is disabled, hidden, or no longer referenced.
- Use `ctx.res.Webcams().devices()` to list connected devices and avoid direct `nokhwa` use.
- See [Webcam Module](contexts/resource_modules/webcam.md).

## 2D Nodes

`Node2D`

- Base transform for 2D nodes (position, rotation, scale, z_index, visible).
- `render_layers` uses [`BitMask`](bitmask.md). A renderable node draws when it does not intersect the active `Camera2D.render_mask`.
- Default `render_layers` is all layers.
- `modulate`, `self_modulate`, and `children_modulate` are RGBA multipliers. `modulate` affects self and descendants; `self_modulate` affects only self; `children_modulate` affects only descendants.
- Effective draw color is parent child modulate * node modulate * node self modulate. All fields default to white.

`Sprite2D`

- Renders a textured quad.
- Holds a `TextureID` (not raw pixels). Use `Texture` module to load.
- `texture_region` selects an atlas rect.

`Label2D`

- Renders world-space text through the UI text renderer.
- Uses `Node2D` position, rotation, scale, z index, visibility, render layers, and modulation.
- `size` is in 2D world units before camera projection.
- Uses `text`, `color`, `font_size`, `h_align`, and `v_align` like `UiLabel`.
- Supports `%loc:` scene text markers and runtime locale text binding like `UiLabel`.
- Use it for nameplates, speech text, signs, and diegetic UI.

`Button2D`

- Clickable world-space rect.
- Uses `Node2D` position, rotation, scale, z index, visibility, and render layers.
- Holds `size`, normal/hover/pressed fills, input state, cursor icon, and extra button signal lists.
- Uses pointer cursor by default on hover. Set `cursor_icon` or `hover_cursor_icon` to override.
- Emits default `<node_name>_<event>` signals for hover enter, hover exit, pressed, released, and clicked.
- `*_signals` fields add extra signals; they do not replace the default named signal.
- Hit testing uses the active 2D camera and world mouse position.

`ImageButton2D`

- Clickable world-space image.
- Uses `Node2D` transform fields like `Button2D`.
- Holds `texture`, `texture_region`, `size`, normal/hover/pressed tint, input state, cursor icon, and extra button signal lists.
- Uses pointer cursor by default on hover. Set `cursor_icon` or `hover_cursor_icon` to override.
- Emits the same default `<node_name>_<event>` signals as `Button2D`.
- `*_signals` fields add extra signals; they do not replace the default named signal.
- Use it for sprite-like buttons, diegetic UI, and world-space interact prompts.

`NineSlice2D`

- Scalable world-space texture panel.
- Uses `texture`, `texture_region`, `size`, `margins`, and `tint`.
- Keeps corners fixed while stretching edges and center.
- Use it as a frame, speech bubble, health bar, or child/background near `Button2D` nodes.

`AnimatedSprite2D`

- Renders a sprite sheet.
- Uses normal `texture = "res://..."` like `Sprite2D`.
- `animations` lists named strip/grid layouts with `frame_size`, `frame_count`, `columns`, and `fps`.
- Uses `current_animation`, `current_frame`, `fps_scale`, `playing`, and `looping`.
- Advances during internal update and renders the current atlas frame.
- Omit `columns` for a left-to-right strip.
- Set `columns` when frames wrap to more than one row in a grid.

`WaterBody2D`

- Shaped water surface centered on its `Node2D` position.
- Supports quad/rect and circle bounds through `shape`.
- Renders through the retained 2D water path when visible and not hidden by camera `render_mask`.
- Runs GPU height/foam simulation with idle modes, wind, flow, damping, wake, foam, camera-distance LOD, and sample readback controls.
- Applies camera-distance-LOD fixed-step buoyancy and vertical drag to `RigidBody2D` when body centers are inside the water shape and below sampled surface height.
- Uses `RigidBody2D.density` for buoyancy scale.
- Does not create collision shapes, raycast hits, contacts, or area signals by itself.
- Add `StaticBody2D`, `Area2D`, or `CollisionShape2D` nodes separately for solid banks, floors, triggers, or queries.
- See [Water Bodies](water.md).

`Camera2D`

- Active 2D camera (position/rotation/zoom).
- `render_mask` hides matching `render_layers` on 2D renderable nodes.
- Default `render_mask` is no layers (`BitMask::NONE`), so the camera hides nothing.
- Supports camera post-processing via `post_processing` (see "Camera Post-Processing" below).
- Supports listener audio effects via `audio_options`; `audio_mask` ignores matching emitted `audio_layer`.

2D lights:

- `AmbientLight2D`
- `RayLight2D`
- `PointLight2D`
- `SpotLight2D`
- Uses `color`, `intensity`, `cast_shadows`, and `active`.
- `PointLight2D` and `SpotLight2D` use `range`.
- `SpotLight2D` uses `inner_angle_radians` and `outer_angle_radians`.
- `cast_shadows = true` uses hard shadows from visible `CollisionShape2D` casters.
- Soft shadows and sprite-alpha silhouettes are future work.

`Skeleton2D`

- 2D rig root.
- Owns `Vec<Bone2D>` data like `Skeleton3D`.
- Load bones with `skeleton = "res://rig.pskel2d"`.
- Use `BoneAttachment2D`, `IKTarget2D`, and `PhysicsBoneChain2D` nodes to target bones.

`Bone2D`

- Data-only bone inside `Skeleton2D.bones`.
- Fields: `name`, `parent`, `rest`, `pose`, `inv_bind`.
- `.panim` bone tracks animate `pose` with `Transform2D`.

Physics 2D:

- `CollisionShape2D`
- `StaticBody2D`
- `RigidBody2D`
- `CharacterBody2D`
- `Area2D`
- `CollisionShape2D` should be authored as a child of `StaticBody2D`, `RigidBody2D`, `CharacterBody2D`, or `Area2D`.
- `CharacterBody2D` is a script-driven kinematic body: engine applies gravity with a collision sweep, no velocity/force state. See [Physics Nodes](physics_nodes.md).
- Static/rigid bodies and areas participate in audio propagation by default through `audio_interaction`.
- Collision shapes only provide geometry.
- See [Physics Nodes](physics_nodes.md) for scene authoring examples.

2D physics shape authoring:

```text
[Body]
    [RigidBody2D]
        [Node2D/]
    [/RigidBody2D]
[/Body]

[BodyShape]
parent = @Body
    [CollisionShape2D]
        shape = { type = quad width = 1.0 height = 1.0 }
    [/CollisionShape2D]
[/BodyShape]
```

2D physics layer/mask fields:

- `collision_layers: BitMask`
- `collision_mask: BitMask`
- Default `collision_layers` is all layers.
- Default `collision_mask` is no layers.
- `collision_layers` tags the collider; `collision_mask` lists tags to ignore.
- Colliders interact only when neither collider's mask ignores the other collider's layers.
- Scene files use `collision_layers = [1, 2]` and `collision_mask = [3]`; Rust code should use `BitMask::with([1, 2])`.

2D joint nodes:

- `PinJoint2D`
- `DistanceJoint2D`
- `FixedJoint2D`

Joint common fields are `body_a`, `body_b`, `anchor_a`, `anchor_b`, `enabled`, and `collide_connected`.
`DistanceJoint2D` also uses `min_distance` and `max_distance`.
Anchors are local to each connected body.

Audio 2D:

- `AudioMask2D`
- `AudioEffectZone2D`
- `AudioPortal2D`
- `AudioMask2D` is invisible audio-only geometry with `CollisionShape2D` children.
- `AudioEffectZone2D` stores ordered reverb/echo/dampening effects. `audio_mask` ignores matching emitted `audio_layer`; shape overlap with source/listener/path applies the chain.
- `AudioPortal2D` marks one-way inputs with `CollisionShape2D` children and linked portal exits. Hit point and ray direction transform through target portal global transforms, then continue through portal hits or physics bounces. Immediate re-entry into the portal just exited is blocked until another portal hit or physics bounce.
- See [Audio Nodes](audio_nodes.md) for scene authoring examples.

2D audio shape authoring:

```text
[Zone]
    [AudioEffectZone2D]
        active = true
        effects = [{ reverb_send = 0.35 echo = 0.0 dampening = 0.0 }]
        [Node2D/]
    [/AudioEffectZone2D]
[/Zone]

[ZoneShape]
parent = @Zone
    [CollisionShape2D]
        shape = { type = quad width = 4.0 height = 4.0 }
    [/CollisionShape2D]
[/ZoneShape]
```

`TileMap2D` is the tile map node.
It uses `.ptileset` data and can emit static 2D colliders from `collision_shape = "auto"`.
See [TileMap2D](tilemap.md).

## 3D Nodes

`Node3D`

- Base transform for 3D nodes (position, rotation, scale, visible).
- `render_layers` uses [`BitMask`](bitmask.md). A renderable node draws when it does not intersect the active `Camera3D.render_mask`.
- Default `render_layers` is all layers.
- `modulate`, `self_modulate`, and `children_modulate` work like `Node2D`, multiplying rendered colors and material surface modulate.

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
- Scene authoring: `skeleton = @NodeName` uses the **scene node key** and is resolved to a `NodeID` at load time.
- Skinning only works if the mesh has proper vertex weights (`JOINTS_0/WEIGHTS_0`).
- Shared-skeleton mesh reuse works when meshes follow the same rig contract: same joint order/indices and compatible weights.
- Automatic retargeting between mismatched rigs is not implemented.
- glTF morph targets import as blend shapes.
- glTF `TEXCOORD_1` imports as dedicated paint UVs. Meshes without UV1 use UV0 as paint UV fallback.
- `blend_shape_weights` stores indexed blend shape weights in Blender-style `0.0..1.0`.
- Aliases: `shape_key_weights`, `morph_weights`.
- Overflow weights are ignored. Missing weights act as `0.0`.
- Mesh LOD is automatic for authored meshes.
- Dynamic/dev load builds LODs on load; static build packs render meshes into `.pmesh` v2.
- Meshes with joints/weights skip LOD generation.
- Surface/material slots stay stable across LODs.
- Current switch distances are radius-scaled (`36x`, `54x`, `72x`, `108x`, `144x` mesh bounds radius).
- `lod` stores `LODOptions` with `min_lod`/`max_lod`.
- LOD values are quality levels, not baked index numbers.
- `LODOptions::MAX = 5` is most detail (`100%`, baked LOD0).
- `LODOptions::MIN = 0` is least detail (`12.5%`, last baked LOD when present).
- Middle names: `LOW = 1`, `MEDIUM_LOW = 2`, `MEDIUM = 3`, `HIGH = 4`.
- Scene authoring supports `min_lod`/`max_lod` plus `lod_min`/`lod_max` aliases.
- Defaults keep full auto range: `min_lod = LODOptions::MIN`, `max_lod = LODOptions::MAX`.
- `flip_x`, `flip_y`, and `flip_z` mirror the rendered mesh around the node's local origin/pivot.
- Use mesh flips when a part needs to become its real opposite-side shape without making a second mesh resource.
- This is different from rotation. A rotated left hair part is still shaped like the left-side mesh. A flipped left hair part becomes the right-side mirror.
- This is also different from "inside out". The renderer handles mirrored winding/flat normals so the mesh remains renderable as a proper mirror.

`MultiMeshInstance3D`

- Renders many copies of one mesh.
- Uses shared mesh/material surface bindings.
- `instances` stores per-instance transform: position, rotation, and scale.
- `instance_grid` can emit per-instance scale via `scale` and `scale_wave`.
- Use it for repeated static props, foliage, debris, or crowd-like non-skinned copies.
- Supports same LOD clamp fields as `MeshInstance3D`.
- Supports `flip_x`, `flip_y`, and `flip_z` on the whole multimesh node.
- `blend_shape_weights` is the default blend shape weight array for every dense instance.
- Per-instance `blend_shape_weights` inside `instances` overrides the node default for that instance.

`Sprite3D`

- Renders a floating textured rect in 3D space.
- Uses `Node3D` position, rotation, scale, visibility, render layers, and modulation.
- `size` is world-space width/height before camera projection.
- Holds a `TextureID` through `texture`; `texture_region` selects an atlas rect.
- Supports `flip_x` and `flip_y`.
- Unlike `Decal3D`, it does not project onto geometry. It can float in front of or above meshes.
- Use it for pickups, markers, billboards, damage icons, and world prompts.

`Label3D`

- Renders floating text in 3D space through the UI text renderer.
- Uses `Node3D` position, rotation, scale, visibility, render layers, and modulation.
- `size` is world-space width/height before camera projection.
- Uses `text`, `color`, `font_size`, `h_align`, and `v_align` like `UiLabel`.
- Text wrapping uses the authored `size` aspect and `font_size`, so camera angle/distance does not change line breaks.
- Supports `%loc:` scene text markers and runtime locale text binding like `UiLabel`.
- Use it for nameplates, signs, speech text, and world HUD labels.

`Camera3D`

- Active 3D camera with projection settings.
- `render_mask` hides matching `render_layers` on 3D renderable nodes.
- Default `render_mask` is no layers (`BitMask::NONE`), so the camera hides nothing.
- Supports camera post-processing via `post_processing` (see "Camera Post-Processing" below).
- Supports listener audio effects via `audio_options`; `audio_mask` ignores matching emitted `audio_layer`.

`ParticleEmitter3D`

- 3D particle emitter driven by a particle profile.

`WaterBody3D`

- Shaped water surface centered on its `Node3D` position.
- Uses local X/Z as surface axes and world Y as height.
- Supports box and cylinder bounds through `shape`.
- Renders through the 3D water path when visible and not hidden by camera `render_mask`.
- Runs GPU height/foam simulation with idle modes, wind, flow, damping, wake, foam, camera-distance LOD, and sample readback controls.
- Applies camera-distance-LOD fixed-step buoyancy and vertical drag to `RigidBody3D` when body centers are inside the water shape and below sampled surface height.
- Uses `RigidBody3D.density` for buoyancy scale.
- Does not create collision shapes, raycast hits, contacts, or area signals by itself.
- Add `StaticBody3D`, `Area3D`, or `CollisionShape3D` nodes separately for lake beds, shores, triggers, or queries.
- See [Water Bodies](water.md).

`Decal3D`

- Projected box decal: paints albedo/normal/emission onto lit 3D geometry inside its `size` box.
- Projects along local -Z (rotate the node like a spotlight to aim it).
- Applied in the material shaders before lighting, so decals receive shadows and lights like the surface under them.
- `albedo_texture`, `normal_texture`, `emission_texture` are all optional; with no albedo texture the `modulate` color paints flat.
- `albedo_mix` blends decal albedo over the surface; `normal_strength` scales the normal patch.
- `normal_fade` (0..1) rejects surfaces facing away from the projection axis.
- `distance_fade_begin`/`distance_fade_length` fade by camera distance; begin `0` disables.
- Higher `sort_priority` blends over lower when decals overlap.
- Affects standard and toon materials plus multimesh instances; unlit materials ignore decals.

`TextDecal3D`

- Projected text decal: rasterizes `text` into a runtime texture, then paints it through the Decal3D path.
- Uses Node3D transform; projects along local -Z.
- `size` is `(width, height, depth)` of the projection box.
- `color` tints the text; alpha controls opacity.
- `font_size`, `h_align`, `v_align`, and `texture_resolution` control the backing texture.
- `emission_energy > 0` reuses the text mask as an emissive decal.
- Use it for wall text, signs, floor labels, and labels that must stick to geometry.

`ParticleEmitter2D`

- 2D particle emitter driven by a particle profile.
- Reads `.ppart` `x` and `y`; ignores `z`.

`AnimationPlayer`

- Plays an `AnimationClip` resource and applies tracks to bound scene nodes.
- `animation` points to clip source/ID; tracks are mapped by object name via `bindings`.
- `playback` supports `once`, `loop`, `boomerang`.
- Runtime control is exposed through Animation macros in Runtime API.

Lights:

- `AmbientLight3D`
- `Sky3D`
- `RayLight3D`
- `PointLight3D`
- `SpotLight3D`
- Shadow-casting 3D lights use `cast_shadows`, `shadow_strength`, `shadow_depth_bias`, and `shadow_normal_bias`. See [3D Shadows](../resources/shadows3d.md).

Physics 3D:

- `CollisionShape3D`
- `StaticBody3D`
- `RigidBody3D`
- `CharacterBody3D`
- `Area3D`
- `CollisionShape3D` should be authored as a child of `StaticBody3D`, `RigidBody3D`, `CharacterBody3D`, or `Area3D`.
- `CharacterBody3D` is a script-driven kinematic body: engine applies gravity with a collision sweep, no velocity/force state. See [Physics Nodes](physics_nodes.md).
- `CollisionShape3D` supports primitive `shape` and mesh-backed `trimesh` source.
- `flip_x`, `flip_y`, and `flip_z` mirror collision geometry around local origin.
- Trimesh source format: `res://path/to/model.glb:mesh[0]` (mesh index optional, default `0`).
- Static/rigid bodies and areas participate in audio propagation by default through `audio_interaction`.
- Collision shapes only provide geometry.
- See [Physics Nodes](physics_nodes.md) for scene authoring examples.

3D physics shape authoring:

```text
[Body]
    [StaticBody3D]
        [Node3D/]
    [/StaticBody3D]
[/Body]

[BodyShape]
parent = @Body
    [CollisionShape3D]
        shape = { type = cube, size = (1, 1, 1) }
    [/CollisionShape3D]
[/BodyShape]
```

## 3D Mesh Flips

Use `flip_x`, `flip_y`, and `flip_z` when one asymmetric mesh should draw as its mirrored counterpart.

Common cases:

- Character creator hair, horns, shoulder pads, pockets, belt pouches, or earrings.
- Left/right accessory variants where rotation does not create the correct shape.
- Modular level pieces where one mesh needs mirrored layout variation.
- Runtime customization where many nodes reuse one `MeshID`.

Do not use flip for:

- Implicit physics collision mirror. Collision shapes are separate nodes and need their own `flip_x`, `flip_y`, or `flip_z`.
- Author-time mesh baking. Flip is per node render state.
- Cases where rotating the object is actually the intended result.

Scene example:

```text
[HairLeft]
parent = @Character
    [MeshInstance3D]
        mesh = "res://models/hair_part.glb:mesh[0]"
        flip_x = false
        [Node3D]
            position = (-0.18, 1.72, 0.04)
        [/Node3D]
    [/MeshInstance3D]
[/HairLeft]

[HairRight]
parent = @Character
    [MeshInstance3D]
        mesh = "res://models/hair_part.glb:mesh[0]"
        flip_x = true
        [Node3D]
            position = (0.18, 1.72, 0.04)
        [/Node3D]
    [/MeshInstance3D]
[/HairRight]
```

Script example:

```rust
methods!({
    fn set_hair_side(&self, ctx: &mut ScriptContext<'_, API>, hair: NodeID, right_side: bool) {
        with_node_mut!(ctx.run, MeshInstance3D, hair, |mesh| {
            mesh.flip_x = right_side;
        });
    }
});
```

Character creator example:

```rust
methods!({
    fn equip_accessory(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        accessory: NodeID,
        mesh_id: MeshID,
        mirror_x: bool,
    ) {
        with_node_mut!(ctx.run, MeshInstance3D, accessory, |node| {
            node.mesh = mesh_id;
            node.flip_x = mirror_x;
            node.flip_y = false;
            node.flip_z = false;
        });
    }
});
```

Level variation example:

```rust
with_node_mut!(ctx.run, MeshInstance3D, wall_trim, |node| {
    node.flip_z = variation_index & 1 != 0;
});
```

Behavior notes:

- Flip mirrors around local origin/pivot after the node transform is chosen.
- Multiple axes can be enabled at once.
- Odd-axis flips reverse triangle winding internally for render batching.
- Materials still use the same mesh and surface bindings.
- `MeshInstance3D` and `MultiMeshInstance3D` share the same field names.
- For skinned meshes, flip mirrors the rendered skinned result at the mesh node level.
- `CollisionShape3D` also accepts these fields, but mesh render flip does not affect collision shape flip.

## 3D Blend Shapes

Perro imports glTF morph targets as blend shapes.
Weights use Blender-style `0.0..1.0` values.
Weights are applied by array index.
Overflow entries are ignored.
Missing entries behave as `0.0`.
Weights are clamped to `0.0..1.0`.
Weights are not normalized across targets.

Scene defaults:

```text
[Face]
    [MeshInstance3D]
        mesh = "res://characters/face.glb:mesh[0]"
        blend_shape_weights = [0.0, 0.2, 0.1]
    [/MeshInstance3D]
[/Face]
```

Aliases:

```text
shape_key_weights = [0.0, 0.2, 0.1]
morph_weights = [0.0, 0.2, 0.1]
```

MultiMesh defaults and per-instance overrides:

```text
[Crowd]
    [MultiMeshInstance3D]
        mesh = "res://characters/face.glb:mesh[0]"
        blend_shape_weights = [0.25, 0.0, 0.5]
        instances = [
            { position = (0, 0, 0) },
            { position = (2, 0, 0), scale = (1.2, 1.0, 1.2), blend_shape_weights = [1.0, 0.2] },
        ]
    [/MultiMeshInstance3D]
[/Crowd]
```

Runtime mutation:

```rust
with_node_mut!(ctx.run, MeshInstance3D, face_id, |node| {
    node.blend_shape_weights.resize(3, 0.0);
    node.blend_shape_weights[1] = 0.75;
});
```

Runtime MultiMesh mutation:

```rust
with_node_mut!(ctx.run, MultiMeshInstance3D, crowd_id, |node| {
    node.blend_shape_weights = vec![0.2, 0.0];
    node.instances[4].blend_shape_weights = Some(vec![1.0, 0.5]);
});
```

3D physics layer/mask fields:

- `collision_layers: BitMask`
- `collision_mask: BitMask`
- Default `collision_layers` is all layers.
- Default `collision_mask` is no layers.
- `collision_layers` tags the collider; `collision_mask` lists tags to ignore.
- Colliders interact only when neither collider's mask ignores the other collider's layers.
- Scene files use `collision_layers = [1, 2]` and `collision_mask = [3]`; Rust code should use `BitMask::with([1, 2])`.

3D joint nodes:

- `BallJoint3D`
- `HingeJoint3D`
- `FixedJoint3D`

Joint common fields are `body_a`, `body_b`, `anchor_a`, `anchor_b`, `enabled`, and `collide_connected`.
`HingeJoint3D` also uses `axis`.
Anchors are local to each connected body.

Audio 3D:

- `AudioMask3D`
- `AudioEffectZone3D`
- `AudioPortal3D`
- `AudioMask3D` is invisible audio-only geometry with `CollisionShape3D` children.
- `AudioEffectZone3D` stores ordered reverb/echo/dampening effects. `audio_mask` ignores matching emitted `audio_layer`; shape overlap with source/listener/path applies the chain.
- `AudioPortal3D` marks one-way inputs with `CollisionShape3D` children and linked portal exits. Hit point and ray direction transform through target portal global transforms, then continue through portal hits or physics bounces. Immediate re-entry into the portal just exited is blocked until another portal hit or physics bounce.
- See [Audio Nodes](audio_nodes.md) for scene authoring examples.

3D audio shape authoring:

```text
[AudioWall]
    [AudioMask3D]
        active = true
        [Node3D/]
    [/AudioMask3D]
[/AudioWall]

[AudioWallShape]
parent = @AudioWall
    [CollisionShape3D]
        shape = { type = cube, size = (1, 2, 0.2) }
    [/CollisionShape3D]
[/AudioWallShape]
```

`Skeleton3D`

- Holds `Vec<Bone3D>` (data-only).
- Bones are loaded via `ResourceWindow::Skeletons().load_bones(source)`.
- Typical flow: scene specifies a `skeleton` path, and scene loader fills `bones`.

`BoneAttachment3D`

- Follows one bone on a `Skeleton3D`.
- Fields:
  - `skeleton`: `@SkeletonNodeKey` ref.
  - `bone` or `bone_index`: zero-based index into `Skeleton3D.bones`.
- Runtime resolves `skeleton = @NodeName` to a `NodeID` at load time.
- Each internal update computes skeleton global transform + bone pose chain transform.
- Attachment's global 3D transform is set to that bone transform.
- Children of the attachment inherit that transform.
- Use it for held gear, muzzle flashes, hit markers, socketed VFX, or any node that should follow a bone.

`PhysicsBoneChain3D`

- Simulates one bone chain on a `Skeleton3D` during fixed update.
- Scene fields:
  - `skeleton`: `@SkeletonNodeKey` ref.
  - `bone` or `bone_index`: zero-based end bone index.
  - `chain_length`: number of links back from end bone.
  - `gravity`: world-space acceleration.
  - `damping`, `stiffness`, `radius`, `collisions`, `iterations`.
- `iterations` default is `3`; use `2` for fast chains or `4` for slower quality chains.
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
  - `skeleton`: `@SkeletonNodeKey` ref.
  - `bone` or `bone_index`: zero-based end bone index.
  - `chain_length`: parent chain length to solve.
  - `iterations`: CCD pass count.
  - `tolerance`: stop distance in skeleton-local units.
  - `weight`: solve blend `0..1`.
  - `match_rotation`: match target rotation on end bone.
- Writes solved transforms into bone `pose`; keeps bone `rest` unchanged.

## UI Nodes

UI nodes inherit from `UiNode` in the node registry:

- `UiNode`
- `UiPanel`
- `UiButton`
- `UiImage`
- `UiImageButton`
- `UiNineSlice`
- `UiAnimatedImage`
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
`UiNode` is the invisible generic container.
`UiPanel` draws a styled box.
`UiButton` draws an interactive styled box and emits default named signals plus extra configured signals.
`UiImage` draws a texture region with tint and scale mode.
`UiImageButton` draws an interactive texture region and emits default named signals plus extra configured signals.
`UiNineSlice` draws a scalable texture panel with fixed corners and stretched edges/center.
`UiAnimatedImage` draws sprite-sheet animation in UI space.
`UiLabel` draws text.
Use `Label2D` or `Label3D` for world-space text that still uses `UiLabel` text, alignment, and locale binding fields.
`UiTextBox` edits one line of text.
`UiTextBlock` edits multi-line text.
`UiLayout`, `UiHLayout`, `UiVLayout`, and `UiGrid` add automatic child placement.
`UiTreeList` renders nested data rows from node-owned item data.

See [UI Nodes](ui.md).

Buttons emit default `<node_name>_<event>` signals.
`hover_signals`, `hover_exit_signals`, `pressed_signals`, `released_signals`, and `clicked_signals` add extra signals on top of the default named signal.

UI style resources:

- Inline `style = { ... }` stays valid.
- Resource `style = "res://ui/panel.uistyle"` loads the same `UiStyle` schema.
- Button `hover` / `pressed` and text edit `focused_style` accept `.uistyle` paths.
- `.uistyle` is visual-only; layout stays on UI nodes.

## Scene Authoring Templates

For copy/paste scene node authoring templates (with all exposed fields, including nil/empty-default fields), see:

- [Node Collections](node_collections.md)

## Camera Post-Processing

Post-processing is configured per camera using `post_processing`.
See `docs/resources/postprocess.md` for full details and examples.

## Visual Accessibility

Visual accessibility is configured globally through `ResourceWindow` (not per-camera).
It runs after camera and global post-processing as the final pass.
See [Visual Accessibility](contexts/resource_modules/visual_accessibility.md).

## Bone3D

`Bone3D` fields:

- `name`: bone name
- `parent`: parent bone index (`-1` for root)
- `rest`: rest transform (local)
- `inv_bind`: inverse bind transform

## Skeleton2D And Bone2D

`Skeleton2D` is a `Node2D` container that owns `Vec<Bone2D>`.
Bones are data, not scene nodes.
This mirrors `Skeleton3D`.

Scene:

```text
[Rig2D]
    [Skeleton2D]
        skeleton = "res://rigs/hero.pskel2d"
    [/Skeleton2D]
[/Rig2D]

[HandMarker]
parent = @Rig2D
    [BoneAttachment2D]
        skeleton = @Rig2D
        bone = 1
    [/BoneAttachment2D]
[/HandMarker]
```

Animation:

```text
[Objects]
Rig2D = Skeleton2D
[/Objects]

[Frame0]
@Rig2D {
    bone["UpperArm"].rotation = 0.0
}
[/Frame0]

[Frame12]
@Rig2D {
    bone["UpperArm"].rotation = 0.7
}
[/Frame12]
```

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
        skeleton = @Rig
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

- `skeleton = @CharacterSkeleton` binds to the skeleton scene node.
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
        skeleton = @CharacterSkeleton
    [/MeshInstance3D]
[/CharacterMesh]

[RightHandSocket]
parent = @Character
    [BoneAttachment3D]
        skeleton = @CharacterSkeleton
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
