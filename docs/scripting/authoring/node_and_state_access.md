# Node And State Access

## Purpose And Mental Model

Node access edits engine data. State access edits script-owned gameplay data.
Choose a typed path when the Rust type is known; choose dynamic access only when
runtime data selects the type/member.

```text
ctx.id -> own node + own script state
NodeID -> other node + scripts attached to that node
```

## Choose A Node

`ctx.id` is the node that owns the current script.

Use the narrowest stable lookup:

1. fixed scene dependency -> state `NodeID`
2. structural dependency -> parent or child relation
3. dynamic membership -> query

Do not search by name each frame for a dependency the scene already knows.
`#[node_ref(...)]` gives the editor and doctor a type hint. Runtime data remains
a `NodeID`.

## Edit Known Node Types

Use `with_node!` for typed reads and `with_node_mut!` for typed writes.
`SelfNodeType` is an optional Rust alias, not an engine rule.

```rust
type SelfNodeType = CharacterBody3D;

let speed = with_node!(ctx.run, SelfNodeType, ctx.id, |node| {
    node.velocity.length()
});

with_node_mut!(ctx.run, Camera3D, camera_id, |camera| {
    camera.fov = 70.0;
});
```

Use base helpers for shared identity, hierarchy, and transform fields. Use a
concrete type for type-specific fields.

```rust
let parent = get_node_parent_id!(ctx.run, ctx.id);

with_base_node_mut!(ctx.run, Node2D, ctx.id, |base| {
    base.position.x += 1.0;
});
```

## Edit Known Script State

Use `with_state!` and `with_state_mut!` for your own state or another script
whose Rust state type is known.

```rust
let health = with_state!(ctx.run, PlayerState, player_id, |state| state.health);

let alive = with_state_mut!(ctx.run, PlayerState, player_id, |state| {
    state.health = (state.health - 10).max(0);
    state.health > 0
}).unwrap_or(false);
```

This path avoids dynamic member lookup and `Variant` conversion.

## Failure, Borrow, And Performance

Typed read access returns the closure output's `Default` value when the ID is
absent, removed, or has the wrong concrete type. Typed mutable access returns
`None` in those cases. Choose a read output whose default is a safe neutral
result, or use a mutable/optional path when absence must stay distinct. Copy
only the result needed by the next call. Never invoke another `ctx.run` API
inside a typed state/node closure.

## Related

- [References And Queries](references_and_queries.md)
- [Script Communication](communication.md)
- [Nodes runtime API](../contexts/runtime_modules/nodes.md)

[Back To Guide](index.md)
