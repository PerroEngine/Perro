# Nodes Module

Purpose:

- Read and mutate scene nodes through `NodeID` handles at runtime.
- `NodeID` is how you address nodes created in scene load, queries, parent/child traversal, or dynamic creation.

Creation macros:

- `create_node!(ctx.run, NodeType) -> NodeID`
- `create_node!(ctx.run, NodeType, name) -> NodeID`
- `create_node!(ctx.run, NodeType, name, tags) -> NodeID`
- `create_node!(ctx.run, NodeType, name, tags, parent_id) -> NodeID`
- `create_nodes!(ctx.run, &[NodeCreationTemplate]) -> Vec<NodeID>`
- `create_nodes!(ctx.run, &[NodeCreationTemplate], parent_id) -> Vec<NodeID>`
- `node_template!(NodeType) -> NodeCreationTemplate`
- `node_template!(NodeType, name) -> NodeCreationTemplate`
- `node_template!(NodeType, name, tags) -> NodeCreationTemplate`

Batch creation example:

```rust
let ids = create_nodes!(
    ctx.run,
    [
        node_template!(Node2D, "EnemyA", tags!["enemy"]),
        node_template!(Node2D, "EnemyB", tags!["enemy"]),
        node_template!(Node2D, "EnemyC", tags!["enemy"]),
        node_template!(Node2D, "EnemyD", tags!["enemy"]),
    ],
    parent_id
);
```

Access macros:

- `with_node_mut!(ctx.run, NodeType, node_id, |node| -> V { ... }) -> Option<V>`
- `with_node!(ctx.run, NodeType, node_id, |node| -> V { ... }) -> V`
- `with_base_node!(ctx.run, BaseType, node_id, |node| -> V { ... }) -> Option<V>`
- `with_base_node_mut!(ctx.run, BaseType, node_id, |node| -> V { ... }) -> Option<V>`

Exact type vs base type:

- `with_node*` requires exact concrete type match.
- `with_base_node*` uses inheritance checks (`is_a`) and succeeds for descendants of `BaseType`.
- Use `with_base_node*` when you do not know exact type of a node at compile time but you do know a common base.

Mutability semantics:

- `with_node_mut` and `with_base_node_mut` let you write fields.
- `with_node` and `with_base_node` are read-only.
- Mutating access returns `Option<V>` because invalid IDs or type mismatch can fail.

Practical inheritance example:

- If query/parent traversal gives mixed `Node3D` descendants, use `with_base_node_mut!(ctx.run, Node3D, id, ...)`.
- Inside closure you can only access fields defined on `Node3D`, but that's expected since that's the type passed in.

Metadata/hierarchy macros:

- `get_node_name!(ctx.run, node_id) -> Option<Cow<'static, str>>`
- `set_node_name!(ctx.run, node_id, name) -> bool`
- `get_node_parent_id!(ctx.run, node_id) -> Option<NodeID>`
- `get_node_children_ids!(ctx.run, node_id) -> Option<Vec<NodeID>>`
- `get_node_type!(ctx.run, node_id) -> Option<NodeType>`
- `reparent!(ctx.run, parent_id, child_id) -> bool`
- `force_rerender!(ctx.run, root_id) -> bool`
- `reparent_multi!(ctx.run, parent_id, child_ids) -> usize`
- `remove_node!(ctx.run, node_id) -> bool`
- `bind_locale_text!(ctx.run, node_id, "ui.key") -> bool`
- `bind_locale_placeholder!(ctx.run, node_id, "ui.key") -> bool`

Runtime node base data:

- `SceneNode.name` stores `Cow<'static, str>`.
- `SceneNode.parent` stores `NodeID`.
- `SceneNode.children` stores `Vec<NodeID>`.
- `SceneNode.tags` stores `Vec<NodeTag>`.
- `node.get_children_ids()` / `node.children_slice()` -> `&[NodeID]`.
- `node.get_tag_ids()` returns ids for internal query/index use.
- `node.tags_slice()` -> `&[NodeTag]`.
- `node.set_children_ids(Some(children))` replaces children from any `Into<Vec<NodeID>>`.
- `node.set_children_ids(None)` clears children.
- `node.set_tags(Some(tags))` replaces tags from `Vec<NodeTag>`.
- `node.set_tag_ids(None)` clears tags.
- `get_node_children_ids!(...)` and `get_node_tags!(...)` return owned `Vec` copies through runtime context.
- `get_node_tags!(...)` returns tag names; ids stay under hood.
- `tag_set!(ctx.run, node_id, tags)` uploads tags back through runtime context.

`force_rerender!` behavior:

- Marks `root_id` + all descendants dirty for current extraction frame.
- Use if you want to force rerender instead of the engine deciding.
- Returns `false` if `root_id` is nil or missing.

Runtime locale text binding:

- `bind_locale_text!` binds main text to a localization key.
- Works on `UiLabel.text`, `UiTextBox.text`, and `UiTextBlock.text`.
- `bind_locale_placeholder!` binds placeholder text.
- Works on `UiTextBox.placeholder` and `UiTextBlock.placeholder`.
- Calling bind again on same node/field replaces the old key.
- Bound text refreshes when current locale changes.

Global transform macros:

- `get_global_transform_2d!(ctx.run, node_id) -> Option<Transform2D>`
- `get_global_transform_3d!(ctx.run, node_id) -> Option<Transform3D>`

- `get_local_transform_2d!(ctx.run, node_id) -> Option<Transform2D>`
- `get_local_transform_3d!(ctx.run, node_id) -> Option<Transform3D>`

- `set_global_transform_2d!(ctx.run, node_id, global_transform) -> bool`
- `set_global_transform_3d!(ctx.run, node_id, global_transform) -> bool`

- `set_local_transform_2d!(ctx.run, node_id, local_transform) -> bool`
- `set_local_transform_3d!(ctx.run, node_id, local_transform) -> bool`

- `get_local_pos_2d!(ctx.run, node_id) -> Option<Vector2>`
- `get_local_pos_3d!(ctx.run, node_id) -> Option<Vector3>`

- `set_local_pos_2d!(ctx.run, node_id, pos) -> bool`
- `set_local_pos_3d!(ctx.run, node_id, pos) -> bool`

- `get_global_pos_2d!(ctx.run, node_id) -> Option<Vector2>`
- `get_global_pos_3d!(ctx.run, node_id) -> Option<Vector3>`

- `set_global_pos_2d!(ctx.run, node_id, pos) -> bool`
- `set_global_pos_3d!(ctx.run, node_id, pos) -> bool`

- `get_local_rot_2d!(ctx.run, node_id) -> Option<f32>`
- `get_local_rot_3d!(ctx.run, node_id) -> Option<Quaternion>`

- `set_local_rot_2d!(ctx.run, node_id, rot) -> bool`
- `set_local_rot_3d!(ctx.run, node_id, rot) -> bool`

- `get_global_rot_2d!(ctx.run, node_id) -> Option<f32>`
- `get_global_rot_3d!(ctx.run, node_id) -> Option<Quaternion>`

- `set_global_rot_2d!(ctx.run, node_id, rot) -> bool`
- `set_global_rot_3d!(ctx.run, node_id, rot) -> bool`

- `get_local_scale_2d!(ctx.run, node_id) -> Option<Vector2>`
- `get_local_scale_3d!(ctx.run, node_id) -> Option<Vector3>`

- `set_local_scale_2d!(ctx.run, node_id, scale) -> bool`
- `set_local_scale_3d!(ctx.run, node_id, scale) -> bool`

- `get_global_scale_2d!(ctx.run, node_id) -> Option<Vector2>`
- `get_global_scale_3d!(ctx.run, node_id) -> Option<Vector3>`

- `set_global_scale_2d!(ctx.run, node_id, scale) -> bool`
- `set_global_scale_3d!(ctx.run, node_id, scale) -> bool`

- `to_global_point_2d!(ctx.run, node_id, local_point) -> Option<Vector2>`
- `to_local_point_2d!(ctx.run, node_id, global_point) -> Option<Vector2>`

- `to_global_point_3d!(ctx.run, node_id, local_point) -> Option<Vector3>`
- `to_local_point_3d!(ctx.run, node_id, global_point) -> Option<Vector3>`

- `to_global_transform_2d!(ctx.run, node_id, local_transform) -> Option<Transform2D>`
- `to_local_transform_2d!(ctx.run, node_id, global_transform) -> Option<Transform2D>`

- `to_global_transform_3d!(ctx.run, node_id, local_transform) -> Option<Transform3D>`
- `to_local_transform_3d!(ctx.run, node_id, global_transform) -> Option<Transform3D>`

Tag macros:

- `get_node_tags!(ctx.run, node_id) -> Option<Vec<Cow<'static, str>>>`
- `tag_set!(ctx.run, node_id, tags) -> bool`
- `tag_set!(ctx.run, node_id) -> bool`
- `tag_add!(ctx.run, node_id, tags) -> bool`
- `tag_remove!(ctx.run, node_id, tag) -> bool`
- `tag_remove!(ctx.run, node_id) -> bool`

Related modules:

- Use [NodeQuery Module](node_query.md) for `query!` and `query_first!`.
- Use [MeshQuery Module](mesh_query.md) for mesh surface hits, batch rays, and mesh regions.

## Node Types

See the full list and per-node notes here:

- [Node Types](../../nodes.md)

Global transform example:

```rust
// Read global transform
if let Some(global) = get_global_transform_3d!(ctx.run, self_id) {
    // Move 1 meter up in global space while keeping parent relation
    let mut target = global;
    target.position.y += 1.0;
    let _ = set_global_transform_3d!(ctx.run, self_id, target);
}

// Convert a local offset to global point
let muzzle_local = Vector3::new(0.0, 0.0, -1.0);
if let Some(muzzle_global) = to_global_point_3d!(ctx.run, self_id, muzzle_local) {
    // Use global-space point for spawning/projectiles/etc.
}
```

Force rerender example:

```rust
// Script updates custom material params outside node fields.
// Force subtree refresh in same frame.
let ok = force_rerender!(ctx.run, character_root_id);
if !ok {
    // invalid/missing root id
}
```


