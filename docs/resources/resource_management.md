# Resource Management

## Page Map

| Header            | Link                                    |
| ----------------- | --------------------------------------- |
| Purpose           | [Purpose](#purpose)                     |
| Resource States   | [Resource States](#resource-states)     |
| Async ID Flow     | [Async ID Flow](#async-id-flow)         |
| Load Reserve Drop | [Load Reserve Drop](#load-reserve-drop) |
| Auto Load         | [Auto Load](#auto-load)                 |
| Ref Counts        | [Ref Counts](#ref-counts)               |
| Auto Drop         | [Auto Drop](#auto-drop)                 |
| GC Cadence        | [GC Cadence](#gc-cadence)               |
| Performance Model | [Performance Model](#performance-model) |
| Examples          | [Examples](#examples)                   |

## Purpose

Perro uses ID handles and a resource store for render resources.

Main resource kinds:

- `TextureID`
- `MeshID`
- `MaterialID`

IDs are small copy values.
The store owns loaded resource data and decides when data can leave memory.

## Async ID Flow

Render resource loads do not block gameplay.
`load` returns the next available stable ID immediately.

That ID can be stored on a node in the same frame.
The renderer uses the resource once decode/upload finishes.
If the load takes a few frames, the ID still stays valid and the node keeps pointing at it.

Normal path:

1. script calls `mesh_load!`, `texture_load!`, or `material_load!`
2. runtime returns an ID immediately
3. backend starts or reuses the async load
4. node stores the ID
5. renderer binds the real GPU resource when ready

This avoids frame stalls from disk IO, decode, or GPU upload.
Dev builds may take a couple frames for larger assets.
Release builds usually load much faster, but the contract stays the same, with no blocking, and a couple frames of delayed change is imperceptible for small quick changing assets, and you can easily use a loading screen for bigger assets that go from nothing to something to hide the load

Use `*_is_loaded!` only when gameplay needs to branch on readiness.
Do not wait on it just to assign a mesh, texture, or material to a node.

## Resource States

| State              | Meaning                                                                   | Auto GC        |
| ------------------ | ------------------------------------------------------------------------- | -------------- |
| loaded, never used | ID exists, data may still be loading or ready, no render node uses it yet | no             |
| reserved           | explicit keep-alive                                                       | no             |
| used, referenced   | one or more render nodes use the ID this frame                            | no             |
| used, unreferenced | used at least once, now no render nodes use it                            | yes, after TTL |
| dropped            | ID generation stale, data removed                                         | no             |

`load` creates or reuses an ID.
`reserve` creates or reuses an ID and marks it keep-alive.
`drop` removes the ID manually and makes old handles stale.

## Load Reserve Drop

Use `load` for normal short-lived or scene-owned resources.

```rust
let mesh_id = mesh_load!(ctx.res, "res://meshes/crate.glb:mesh[0]");
let texture_id = texture_load!(ctx.res, "res://textures/crate.png");
```

Loaded resources do not enter GC until they are used by a render node once.
This protects async load paths.
A resource loaded early can sit at zero refs while async work finishes or while it waits for first bind.

Use `reserve` for explicit keep-alive.

```rust
let mesh_id = mesh_reserve!(ctx.res, "res://meshes/player.glb:mesh[0]");
let texture_id = texture_reserve!(ctx.res, "res://textures/player.png");
```

You can also promote an ID you already have.
This avoids remembering the original source path.

```rust
let mesh_id = mesh_load!(ctx.res, "res://meshes/player.glb:mesh[0]");
let same_mesh_id = mesh_reserve!(ctx.res, mesh_id);
```

Reserved resources stay in memory until manual drop.
Use this for player assets, shared atlases, common materials, streaming anchors, or loading screens that you don't want to risk auto dropping/know you will never drop unless explicitly requested.

Use `drop` for explicit release.

```rust
mesh_drop!(ctx.res, mesh_id);
texture_drop!(ctx.res, texture_id);
```

Manual drop wins over auto GC.
After drop, old IDs fail generation checks.

## Auto Load

Scenes and node render paths can load resources automatically.

Examples:

- `Sprite2D.texture`
- `AnimatedSprite2D.texture`
- `MeshInstance3D.mesh`
- `MeshInstance3D` surface materials
- `MultiMeshInstance3D.mesh`
- `MultiMeshInstance3D` surface materials
- tilemap textures
- UI image textures

When a scene or animation assigns a resource path, runtime asks the resource API for an ID.
Render backend creates the resource if missing and reuses the existing ID if the source already exists.

If several nodes ask for the same source, they share one ID.
Duplicate load requests do not duplicate GPU resource identity.

## Ref Counts

Ref counts come from retained render state.
They count live nodes using the resources

Per frame when retained render refs change:

1. clear old nonzero refs
2. count `Sprite2D` and `AnimatedSprite2D` texture users
3. count UI image texture users
4. count `MeshInstance3D` and `MultiMeshInstance3D` mesh users
5. count material users from mesh surfaces
6. write counts into resource metadata

Example:

| Scene state                   | Resource ref count                                 |
| ----------------------------- | -------------------------------------------------- |
| one sprite uses texture A     | texture A = 1                                      |
| two sprites use texture A     | texture A = 2                                      |
| one mesh instance uses mesh M | mesh M = 1                                         |
| two mesh instances use mesh M | mesh M = 2                                         |
| mesh instance removed         | mesh M count decreases on next retained ref update |
| mesh changed from M to N      | M decreases, N increases                           |

Only touched refs are reset.
The store does not scan every loaded resource to clear counts.

## Auto Drop

Auto GC only sees used, unreserved candidates.

Flow:

1. first render use marks `used_once`
2. unreserved resource enters GC candidate queue
3. GC checks candidate refs on interval
4. `ref_count > 0` resets zero-ref age
5. `ref_count == 0` increments zero-ref age
6. TTL hit drops resource

Loaded but never used resources are not candidates.
Reserved resources are not candidates.

This means `mesh_load!` can return an ID before GPU data exists.
The ID stays valid while async load finishes or while code prepares a node.
After first node use, normal ref-count GC applies.

## GC Cadence

GC does not run every frame.
Renderer runs resource GC on a fixed frame interval.

Current defaults:

| Setting            | Value     | Meaning                                   |
| ------------------ | --------- | ----------------------------------------- |
| GC interval        | 60 frames | Check candidates about once per 60 frames |
| zero-ref TTL       | 60 frames | Drop after about 60 frames with no refs   |
| max drops per kind | 64        | Cap drops per GC churn per resource kind  |

Each GC pass adds elapsed frames to zero-ref age.
TTL stays frame-based even though GC runs less often.

Large cleanup batches are split.
If 1000 textures expire at once, GC drops at most 64 textures in one churn, then keeps the rest queued for later passes.
This avoids one large stall.

If a resource gains a ref before the next GC churn, age resets and it stays alive.

## Performance Model

GC cost depends on candidate count, not total resource count.

Fast paths:

- reserved resources: no candidate scan
- never-used loaded resources: no candidate scan
- active resources: candidate check sees refs and resets age
- stale async results: ignored if ID was dropped or generation changed
- mass expiry: drop work is capped per resource kind per GC pass

Ref count cost depends on retained render changes.
If sprites and draws do not change, cached counts are reused.
When they change, backend rebuilds count maps from retained nodes, then writes compact counts to resource metadata.

The store keeps:

- slot metadata for fast ID generation checks
- source maps for source-to-ID reuse
- GC candidate queues for used unreserved resources
- touched-ref lists for fast ref reset

## Examples

Load now, bind later:

```rust
let mesh = mesh_load!(ctx.res, "res://meshes/enemy.glb:mesh[0]");
// No node uses it yet.
// It stays loaded/valid and does not enter GC candidate queue.
```

Assign immediately, render when ready:

```rust
methods!({
    fn set_enemy_mesh(&self, ctx: &mut ScriptContext<'_, API>, mesh_node: NodeID) {
        let mesh_id = mesh_load!(ctx.res, "res://meshes/enemy.glb:mesh[0]");

        with_node_mut!(ctx.run, MeshInstance3D, mesh_node_id, |node| {
            node.mesh = mesh_id;
        });

        // No blocking wait.
        // Renderer uses mesh once async load finishes.
    }
});
```

Swap tool mesh without a stall:

```rust
methods!({
    fn equip_tool_version(&self, ctx: &mut ScriptContext<'_, API>, tool_node: NodeID, version: i32) {
        let source = match version {
            2 => "res://meshes/tools/hammer_v2.glb:mesh[0]",
            3 => "res://meshes/tools/hammer_v3.glb:mesh[0]",
            _ => "res://meshes/tools/hammer_v1.glb:mesh[0]",
        };
        let next_mesh = mesh_load!(ctx.res, source);

        with_node_mut!(ctx.run, MeshInstance3D, tool_node, |node| {
            node.mesh = next_mesh;
        });

        // Animations, transforms, scripts, and physics keep running.
        // If next_mesh takes a few frames, renderer keeps last retained mesh.
        // Renderer swaps to next_mesh only after it becomes ready.
    }
});
```

Example: tool node renders mesh ID `5`.
Script assigns pending mesh ID `9`.
Runtime node now points at `9`, but renderer keeps retained draw for `5` until `9` finishes.
No blank frame is emitted for that node just because `9` is still loading.
When `9` is ready, renderer emits the new draw and swaps the retained mesh.

Use by nodes:

```rust
// Two MeshInstance3D nodes use same mesh ID.
// Resource ref count for that mesh becomes 2.
```

Reserve shared atlas:

```rust
let atlas = texture_reserve!(ctx.res, "res://textures/ui_atlas.png");
// Stays in memory until texture_drop!(ctx.res, atlas).
```

Normal scene cleanup:

```rust
// Remove sprites that use texture A.
// Ref count for texture A becomes 0.
// After TTL, auto GC drops texture A unless reserved.
```
