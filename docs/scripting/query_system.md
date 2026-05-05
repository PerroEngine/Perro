# Query System

Perro query system returns `NodeID` lists from scene graph filters.
Use it when direct refs are not enough and you want dynamic lookup.

This works on top of Perro's object-centric node model.
Queries do not replace nodes or script state.
Queries help find nodes, then you act through normal node/script APIs.

## Why Use Query

- Find nodes by name, tag, concrete type, or base type.
- Build dynamic groups without hardcoding scene paths.
- Chain query -> `NodeID` -> `with_node!` / `with_node_mut!` / script macros.
- Keep logic data-driven while runtime access stays ID-based.

## Core Macros

- `query!(ctx.run, expr) -> Vec<NodeID>`
- `query!(ctx.run, expr, in_subtree(parent_id)) -> Vec<NodeID>`
- `query_first!(ctx.run, expr) -> Option<NodeID>`
- `query_first!(ctx.run, expr, in_subtree(parent_id)) -> Option<NodeID>`

## Expression Grammar

Boolean forms:

- `all(expr1, expr2, ...)`
- `any(expr1, expr2, ...)`
- `not(expr)`

Scope form:

- `in_subtree(parent_id)`

Predicate forms:

- `name["Player", "Boss"]`
- `tags["enemy", "alive"]`
- `is[Camera3D, MeshInstance3D]`
- `is_type[Camera3D, MeshInstance3D]`
- `base[Node3D]`
- `base_type[Node3D]`

## Mental Model

- Query filters current runtime node set.
- Return value is `NodeID` handles only.
- You still choose typed access after query:
  - `with_node!` for exact type
  - `with_base_node!` for base-type access
  - script access macros for state/method vars

## Common Patterns

### 1) Tag Groups

```rust
let enemies = query!(ctx.run, all(tags["enemy"], not(tags["dead"])));
for id in enemies {
    let _ = with_base_node_mut!(ctx.run, Node3D, id, |node| {
        node.transform.position.y += 0.1;
    });
}
```

### 2) Name or Tag Fallback

```rust
let target = query_first!(ctx.run, any(name["Boss"], tags["primary_target"]));
if let Some(id) = target {
    set_var!(ctx.run, id, var!("alert"), variant!(true));
}
```

### 3) Subtree-Scoped Scan

```rust
let local_hits = query!(
    ctx,
    all(base[Node3D], tags["interactable"]),
    in_subtree(zone_root_id)
);
```

### 4) Query -> Script Interop

```rust
let allies = query!(ctx.run, all(tags["ally"], tags["alive"]));
for id in allies {
    call_method!(ctx.run, id, method!("on_team_buff"), params![variant!(5.0_f32)]);
}
```

## Performance Notes

- Core node/script storage is flat and ID-indexed, so post-query operations stay cheap.
- Query cost depends on match set size and predicate complexity.
- For hot loops:
  - cache stable `NodeID`s when safe
  - refresh cache on scene changes or lifecycle events
  - prefer narrower predicates + subtree limits

## Failure Behavior

- Query miss => empty `Vec` or `None`.
- Follow-up ops can fail if target node/script no longer exists.
- This keeps failure tied to actual scene/runtime state, not borrow timing.

## Related Docs

- [Runtime Nodes Module](contexts/runtime_modules/nodes.md)
- [Mesh Query Perf Snapshot](mesh_query_perf.md)
- [Script Contexts](contexts/README.md)
- [Script State](state.md)
- [Script Methods](methods.md)

