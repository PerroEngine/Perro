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
`SelfNodeType` is an optional Rust alias for when you know the script is always attached to that node type and you don't want to think about the type after you've written the alias.

```rust
type SelfNodeType = CharacterBody3D;

let speed = with_node!(ctx.run, SelfNodeType, ctx.id, |node| {
    node.velocity.length()
}).unwrap_or_default();

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
let health = with_state!(ctx.run, PlayerState, player_id, |state| state.health).unwrap_or_default();

let alive = with_state_mut!(ctx.run, PlayerState, player_id, |state| {
    state.health = (state.health - 10).max(0);
    state.health > 0
}).unwrap_or(false);
```

This path avoids dynamic member lookup and `Variant` conversion, and it just makes sense when you know the id of the node you're calling has that definitive state.

## Failure, Borrow, And Performance

Typed read and mutable access return `None` when the ID is absent, removed, or
has the wrong concrete type. Use `unwrap_or_default()` only when a neutral
fallback is valid. Keep the `Option` when absence must stay distinct. Copy only
the result needed by the next call. Never invoke another `ctx.run` API inside a
typed state/node closure.

Use `warn_none` when a missing value must be visible but the feature can skip
work and keep the game alive. Use `warn_none_once` in update loops so one bad
reference does not print every frame. Both helpers return the original
`Option`, so normal `let Some`, `?`, and fallback flow still works.

```rust
let Some(speed) = with_node!(ctx.run, CharacterBody3D, body_id, |body| {
    body.velocity.length()
})
.warn_none_once(format_args!(
    "movement skip: node={} expect=CharacterBody3D missing",
    body_id.as_u64()
)) else {
    return;
};
```

Use `warn_err` and `warn_err_once` for `Result`. They append the source error to
the supplied operation context. Include the failed operation, target ID/path,
expected type, and chosen fallback in warning text. Do not warn inside the base
lookup API because absence may be expected by another caller.

## Related

- [References And Queries](references_and_queries.md)
- [Script Communication](communication.md)
- [Nodes runtime API](../contexts/runtime_modules/nodes.md)

[Back To Guide](index.md)
