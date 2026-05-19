# NodeQuery Module

Runtime handle:

- Direct module: `ctx.run.NodeQuery()`
- Macro route: `query!` and `query_first!`

Use `NodeQuery` when you need `NodeID` values from scene filters.
Use `Nodes` when you already have a `NodeID` and need read/write node data.

Macros:

- `query!(ctx.run, expr) -> Vec<NodeID>`
- `query!(ctx.run, expr, in_subtree(parent_id)) -> Vec<NodeID>`
- `query!(ctx.run, &node_query) -> Vec<NodeID>`
- `query!(ctx.run, &node_query, in_subtree(parent_id)) -> Vec<NodeID>`
- `query_first!(ctx.run, expr) -> Option<NodeID>`
- `query_first!(ctx.run, expr, in_subtree(parent_id)) -> Option<NodeID>`
- `query_expr!(expr) -> QueryExpr`
- `query_builder!(expr) -> NodeQuery`
- `query_builder!(expr, in_subtree(parent_id)) -> NodeQuery`

Direct API:

```rust
let q = NodeQuery::new().where_expr(query_expr!(all(name["Player"])));
let ids = ctx.run.NodeQuery().query(&q);
```

What queries are:

- Query is a runtime filter that returns `NodeID` values.
- Query execution belongs to `NodeQuery`, not `Nodes`.
- `in_subtree(parent_id)` limits search to descendants of that node.
- Default query scope is full scene tree.
- `all(...)` means every condition must match.
- `any(...)` means one condition must match.
- `not(...)` means inner condition must not match.

Query forms:

- `all(expr1, expr2, ...)`
- `any(expr1, expr2, ...)`
- `not(expr)`
- `in_subtree(parent_id)`

Predicates:

- `name["Player", "Boss"]`
- `tags["enemy", "alive"]`
- `node_type[Camera3D, MeshInstance3D]`
- `base_type[Node3D]`
- `layers[1, 2]`
- `mask[3]`

Examples:

```rust
let enemies = query!(ctx.run, all(tags["enemy"], not(tags["dead"])));
```

```rust
let local_hits = query!(
    ctx.run,
    all(any(tags["enemy"], name["Boss"]), not(tags["dead"])),
    in_subtree(room_root_id)
);
```

```rust
if let Some(camera_id) = query_first!(ctx.run, all(node_type[Camera3D])) {
    // use camera_id
}
```

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

Reusable query rules:

- Pass `&query` to reuse without cloning.
- Use `query_expr!` for shared predicate chunks.
- Use `query_builder!` for dynamic filters.
- `query!(..., in_subtree(...))` overrides scope for one call and keeps source query unchanged.
- Cache stable `NodeID` values when a hot path does not need dynamic lookup.

Perf rules:

- Cache stable `NodeID` values after setup.
- Avoid full-tree query in hot loops.
- Prefer `in_subtree(...)` when you know the owner/root.
- Query once during bind/init, then use `Nodes` for direct access.
- Prefer `node_type[...]` or `base_type[...]` filters in broad queries.
- Prefer tag-only queries when tags define the group; runtime can seed candidates from tag index.
- Prefer literal type predicates in macros; they compile to type masks.

Runtime optimizations:

- Type masks prune exact/base type misses before full expression eval.
- Tag index can seed candidate IDs for tag-only and simple tag queries.
- Indexed tag candidates are intersected from smallest set to largest set.
- `all(...)` with tag + non-tag predicates scans only the smallest indexed candidate set.
- Missing required indexed tags return exact empty results without full scan.
- `all(...)` and `any(...)` children are reordered by estimated cost.
- Large full-tree scans can split work across workers.
- `in_subtree(...)` scans only the requested subtree.

Bench:

```bash
cargo bench -p perro_runtime --bench query_hotpaths
```

Bench groups:

- `query/rt_ctx_queries`: broad/selective/rare-tag+name queries over `100`, `2_500`, `10_000`, `50_000`, `100_000` nodes.
- `query/compile_repr`: vec-type predicates vs type-mask predicates.

More:

- [Query System](../../query_system.md)
