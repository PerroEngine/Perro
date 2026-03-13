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
