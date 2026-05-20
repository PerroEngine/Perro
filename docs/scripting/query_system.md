# Query System

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Query System` when this feature, type group, file format, or workflow appears in game code or assets.

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

# Query System

Perro query system returns `NodeID` lists from scene graph filters.
Use it when direct refs are not enough and you want dynamic lookup.

Runtime module: `ctx.run.NodeQuery()`.
The `query!` and `query_first!` macros route to `NodeQuery`.
Reusable query type: `NodeQuery`.

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
- `query!(ctx.run, &node_query) -> Vec<NodeID>`
- `query!(ctx.run, &node_query, in_subtree(parent_id)) -> Vec<NodeID>`
- `query_first!(ctx.run, expr) -> Option<NodeID>`
- `query_first!(ctx.run, expr, in_subtree(parent_id)) -> Option<NodeID>`
- `query_expr!(expr) -> QueryExpr`
- `query_builder!(expr) -> NodeQuery`
- `query_builder!(expr, in_subtree(parent_id)) -> NodeQuery`

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
- `node_type[Camera3D, MeshInstance3D]`
- `base_type[Node3D]`
- `layers[1, 2, 3]`
- `mask[1]`

## Mental Model

- Query filters current runtime node set.
- Return value is `NodeID` handles only.
- Query execution belongs to `ctx.run.NodeQuery()`, not `ctx.run.Nodes()`.
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
    ctx.run,
    all(base_type[Node3D], tags["interactable"]),
    in_subtree(zone_root_id)
);
```

### 4) Reusable NodeQuery

Use `query_builder!` when several systems share the same filter or when gameplay options add extra predicates.

```rust
fn actor_query(include_sleeping: bool) -> NodeQuery {
    let mut q = query_builder!(all(
        base_type[Node3D],
        tags["actor"],
        layers[1]
    ));

    if !include_sleeping {
        q = q.where_expr(query_expr!(not(tags["sleeping"])));
    }

    q
}

let actors = actor_query(false);

let all_actors = query!(ctx.run, &actors);
let room_actors = query!(ctx.run, &actors, in_subtree(room_root_id));
```

- Passing `&actors` reuses the query without cloning.
- `in_subtree(...)` on `query!` overrides scope for that call only.
- Use this for target systems, editor/tool panels, optional filters, and room-local scans.

### 5) Direct NodeQuery Module

```rust
let q = NodeQuery::new().where_expr(query_expr!(all(name["Player"])));
let ids = ctx.run.NodeQuery().query(&q);
```

### 6) Query -> Script Interop

```rust
let allies = query!(ctx.run, all(tags["ally"], tags["alive"]));
for id in allies {
    call_method!(ctx.run, id, method!("on_team_buff"), params![variant!(5.0_f32)]);
}
```

### 7) Render Layer Filters

```rust
let layer_one = query!(ctx.run, all(base_type[Node2D], layers[1]));
let gameplay = query!(ctx.run, all(base_type[Node3D], layers[1, 2, 3]));
let not_layer_one = query!(ctx.run, all(base_type[Node2D], mask[1]));
```

- `layers[...]` matches 2D/3D nodes whose `render_layers` intersects any listed layer.
- `mask[...]` rejects 2D/3D nodes whose `render_layers` intersects any listed layer.
- `layers[1]` means only nodes on render layer 1.
- `layers[1, 2, 3]` means nodes on any of layers 1, 2, or 3.
- `mask[1]` means all nodes except ones on layer 1.
- Combine with `base_type[Node2D]` or `base_type[Node3D]` to avoid non-spatial nodes.

## Performance Notes

- Core node/script storage is flat and ID-indexed, so post-query operations stay cheap.
- Query cost depends on match set size and predicate complexity.
- Literal `tags["enemy"]` values hash at compile time; dynamic tag expressions hash at runtime.
- Literal `node_type[...]` and `base_type[...]` predicates compile into growable type bitmasks.
- Literal `layers[...]` and `mask[...]` predicates compile into `BitMask` layer masks.
- Type-only boolean groups use mask algebra: `all` intersects, `any` unions, and `not` complements.
- Runtime query planning reorders predicates by estimated cost and uses tag indexes and type masks when possible.
- Indexed tag candidate sets are intersected smallest-to-largest before full predicate eval.
- Mixed queries like `all(tags["rare"], name["Boss"])` scan rare tag candidates, not the full scene.
- Large full-tree scans can split work across workers.
- Use the query benchmark when changing query planner/index code:

```bash
cargo bench -p perro_runtime --bench query_hotpaths
```

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
