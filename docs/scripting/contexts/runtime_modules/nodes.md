# Nodes Module

Purpose:

- Read and mutate scene nodes through `NodeID` handles at runtime.
- `NodeID` is how you address nodes created in scene load, queries, parent/child traversal, or dynamic creation.

Creation macros:

- `create_node!(ctx, NodeType) -> NodeID`
- `create_node!(ctx, NodeType, name) -> NodeID`
- `create_node!(ctx, NodeType, name, tags) -> NodeID`
- `create_node!(ctx, NodeType, name, tags, parent_id) -> NodeID`

Access macros:

- `with_node_mut!(ctx, NodeType, node_id, |node| -> V { ... }) -> Option<V>`
- `with_node!(ctx, NodeType, node_id, |node| -> V { ... }) -> V`
- `with_base_node!(ctx, BaseType, node_id, |node| -> V { ... }) -> Option<V>`
- `with_base_node_mut!(ctx, BaseType, node_id, |node| -> V { ... }) -> Option<V>`

Exact type vs base type:

- `with_node*` requires exact concrete type match.
- `with_base_node*` uses inheritance checks (`is_a`) and succeeds for descendants of `BaseType`.
- Use `with_base_node*` when you do not know exact type of a node at compile time but you do know a common base.

Mutability semantics:

- `with_node_mut` and `with_base_node_mut` let you write fields.
- `with_node` and `with_base_node` are read-only.
- Mutating access returns `Option<V>` because invalid IDs or type mismatch can fail.

Practical inheritance example:

- If query/parent traversal gives mixed `Node3D` descendants, use `with_base_node_mut!(ctx, Node3D, id, ...)`.
- Inside closure you can only access fields defined on `Node3D`, but that's expected since that's the type passed in.

Metadata/hierarchy macros:

- `get_node_name!(ctx, node_id) -> Option<Cow<'static, str>>`
- `set_node_name!(ctx, node_id, name) -> bool`
- `get_node_parent_id!(ctx, node_id) -> Option<NodeID>`
- `get_node_children_ids!(ctx, node_id) -> Option<Vec<NodeID>>`
- `get_node_type!(ctx, node_id) -> Option<NodeType>`
- `reparent!(ctx, parent_id, child_id) -> bool`
- `reparent_multi!(ctx, parent_id, child_ids) -> usize`
- `remove_node!(ctx, node_id) -> bool`

Global transform macros:

- `get_global_transform_2d!(ctx, node_id) -> Option<Transform2D>`
- `get_global_transform_3d!(ctx, node_id) -> Option<Transform3D>`
- `set_global_transform_2d!(ctx, node_id, global_transform) -> bool`
- `set_global_transform_3d!(ctx, node_id, global_transform) -> bool`
- `to_global_point_2d!(ctx, node_id, local_point) -> Option<Vector2>`
- `to_local_point_2d!(ctx, node_id, global_point) -> Option<Vector2>`
- `to_global_point_3d!(ctx, node_id, local_point) -> Option<Vector3>`
- `to_local_point_3d!(ctx, node_id, global_point) -> Option<Vector3>`
- `to_global_transform_2d!(ctx, node_id, local_transform) -> Option<Transform2D>`
- `to_local_transform_2d!(ctx, node_id, global_transform) -> Option<Transform2D>`
- `to_global_transform_3d!(ctx, node_id, local_transform) -> Option<Transform3D>`
- `to_local_transform_3d!(ctx, node_id, global_transform) -> Option<Transform3D>`

Tag/query macros:

- `get_node_tags!(ctx, node_id) -> Option<Vec<TagID>>`
- `tag_set!(ctx, node_id, tags) -> bool`
- `tag_set!(ctx, node_id) -> bool`
- `tag_add!(ctx, node_id, tags) -> bool`
- `tag_remove!(ctx, node_id, tag) -> bool`
- `tag_remove!(ctx, node_id) -> bool`
- `query!(ctx, expr) -> Vec<NodeID>`
- `query!(ctx, expr, in_subtree(parent_id)) -> Vec<NodeID>`
- `query_first!(ctx, expr) -> Option<NodeID>`
- `query_first!(ctx, expr, in_subtree(parent_id)) -> Option<NodeID>`

Mesh surface/material query macros:

- `mesh_surface_at_world_point_3d!(ctx, node_id, world_point) -> Option<MeshSurfaceHit3D>`
- `mesh_material_regions_3d!(ctx, node_id, material_id) -> Vec<MeshMaterialRegion3D>`

`MeshSurfaceHit3D` fields:

- `instance_index`: instance id for `MultiMeshInstance3D` (0 for regular mesh)
- `surface_index`: matched mesh surface
- `material`: material bound on that surface (`Option<MaterialID>`)
- `world_point`: nearest point on mesh in world space
- `local_point`: nearest point in mesh local space
- `world_normal`: surface normal in world space at nearest point
- `local_normal`: surface normal in mesh local space at nearest point
- `distance`: distance from query point to nearest point

`MeshMaterialRegion3D` fields:

- `instance_index`, `surface_index`, `material`
- `triangle_count`
- `center_world`, `center_local`
- `aabb_min_world`, `aabb_max_world`
- `aabb_min_local`, `aabb_max_local`

What queries are:

- Query is a runtime filter that returns `NodeID` of nodes that match the values.
- You can combine boolean expressions and type/name/tag predicates.
- `in_subtree(parent_id)` restricts matches to descendants of that node's children, by default the entire tree is queried.
- `all(...)` means every condition inside must match.
- `any(...)` means at least one condition inside must match.
- `not(...)` means the inner condition must not match.
- You can nest these expressions to build complex filters.

Query forms:

- `all(expr1, expr2, ...)`
- `any(expr1, expr2, ...)`
- `not(expr)`
- `in_subtree(parent_id)`

Predicates:

- `name["Player", "Boss"]`
- `tags["enemy", "alive"]`
- `is[Camera3D, MeshInstance3D]`
- `is_type[Camera3D, MeshInstance3D]`
- `base[Node3D]`
- `base_type[Node3D]`

## Node Types

See the full list and per-node notes here:
- [Node Types](../../nodes.md)

Composition examples:

```rust
// Must satisfy BOTH: enemy and alive
let node_ids_a = query!(ctx, all(tags["enemy"], tags["alive"]));

// Must satisfy AT LEAST ONE: Player or Boss
let node_ids_b = query!(ctx, any(name["Player"], name["Boss"]));

// Must NOT satisfy: dead
let node_ids_c = query!(ctx, not(tags["dead"]));

// Nested combination:
// (enemy OR Boss) AND NOT dead, limited to one subtree
let node_ids_d = query!(
    ctx,
    all(any(tags["enemy"], name["Boss"]), not(tags["dead"])),
    in_subtree(root_id)
);
```

Example:

```rust
let ids = query!(ctx, all(base[Node3D], not(tags["dead"])));
for id in ids {
    let _ = with_base_node_mut!(ctx, Node3D, id, |node| {
        node.transform.position.y += 0.1;
    });
}
```

Global transform example:

```rust
// Read world transform
if let Some(world) = get_global_transform_3d!(ctx, self_id) {
    // Move 1 meter up in world space while keeping parent relation
    let mut target = world;
    target.position.y += 1.0;
    let _ = set_global_transform_3d!(ctx, self_id, target);
}

// Convert a local offset to world point
let muzzle_local = Vector3::new(0.0, 0.0, -1.0);
if let Some(muzzle_world) = to_global_point_3d!(ctx, self_id, muzzle_local) {
    // Use world-space point for spawning/projectiles/etc.
}
```

Mesh query examples:

```rust
let p = Vector3::new(2.0, 1.0, -5.0);
if let Some(hit) = mesh_surface_at_world_point_3d!(ctx, mesh_node_id, p) {
    // hit.surface_index
    // hit.material
    // hit.world_point
    // hit.world_normal
}
```

```rust
let regions = mesh_material_regions_3d!(ctx, mesh_node_id, material_id);
for r in regions {
    // r.surface_index
    // r.center_world
    // r.aabb_min_world / r.aabb_max_world
}
```
