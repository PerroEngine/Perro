# Query System

## Page Map

| Header        | Link                              |
| ------------- | --------------------------------- |
| Why Use Query | [Why Use Query](#why-use-query)   |
| Use Cases     | [Use Cases](#use-cases)           |
| Example       | [Example](#example)               |
| Reference     | [Reference](#reference)           |

## Use Cases

Use queries when game logic needs a set of nodes chosen by runtime state, not a hardcoded reference.

- Find all enemies, pickups, interactables, or team members by tag.
- Treat tags like dataless components: `enemy`, `alive`, `quest_target`, `damage_zone`.
- Limit work to one room, UI panel, spawned wave, or scene chunk with `in_subtree(...)`.
- Find nodes by type when a system owns behavior for all `Node2D`, `Node3D`, camera, light, or UI nodes.
- Find nodes near a point with `within[origin, size]`: proximity triggers, AI awareness, area damage, spatial pickups.
- Combine filters for gameplay systems, debug tools, editor panels, and target selection.
- Use `query_iter!`, `query_each!`, and `query_map!` to keep common loop code short.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        query_each!(ctx.run, all(tags["enemy"], tags["alive"]), |id| {
            call_method!(ctx.run, id, method!("on_player_seen"), params![]);
        });
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
- `query_iter!(ctx.run, expr) -> impl Iterator<Item = NodeID>`
- `query_each!(ctx.run, expr, |id| { ... })`
- `query_map!(ctx.run, expr, |id| value) -> Vec<T>`
- `query_first!(ctx.run, expr) -> Option<NodeID>`
- `query_first!(ctx.run, expr, in_subtree(parent_id)) -> Option<NodeID>`
- `query_expr!(expr) -> QueryExpr`
- `query_builder!(expr) -> NodeQuery`
- `query_builder!(expr, in_subtree(parent_id)) -> NodeQuery`

## Which Helper To Use

### `query!`

Use `query!` when the matched IDs are the thing you want.
It returns `Vec<NodeID>`.
Pick it when you need to sort, count, store, diff, reuse, or loop more than once.

```rust
let enemies = query!(ctx.run, all(tags["enemy"], not(tags["dead"])));
if enemies.len() > 20 {
    signal_emit!(ctx.run, signal!("too_many_enemies"));
}
```

### `query_iter!`

Use `query_iter!` when iterator adapters make the code smaller or clearer.
It exists so query results can flow into normal Rust iterator chains.
Pick it for `take`, `filter`, `filter_map`, `map`, `find`, `any`, `all`, or `collect`.

```rust
let first_three = query_iter!(ctx.run, all(tags["pickup"], not(tags["claimed"])))
    .take(3)
    .collect::<Vec<_>>();
```

### `query_each!`

Use `query_each!` when each match triggers an action and you do not need a result list.
It exists to remove boilerplate for query -> for loop -> side effect.
Pick it for calling methods, setting vars, adding tags, moving nodes, or sending signals per node.

```rust
query_each!(ctx.run, all(tags["enemy"], tags["awake"]), |id| {
    call_method!(ctx.run, id, method!("on_alarm"), params![]);
});
```

### `query_map!`

Use `query_map!` when each matched node becomes one output value.
It exists for query -> transform -> collect.
Pick it for collecting positions, names, script vars, distances, or optional lookups.

```rust
let enemy_positions = query_map!(ctx.run, all(tags["enemy"], base_type[Node3D]), |id| {
    get_global_pos_3d!(ctx.run, id)
});
```

### `query_first!`

Use `query_first!` when one match is enough.
It exists for singleton-style lookup and fallback target lookup.
Pick it for player, camera, boss, selected node, current objective, or first available interactable.

```rust
if let Some(target) = query_first!(ctx.run, any(name["Boss"], tags["primary_target"])) {
    set_var!(ctx.run, target, var!("tracked"), variant!(true));
}
```

### `query_expr!`

Use `query_expr!` when you need a `QueryExpr` value.
It exists so you can compose or conditionally add filters in normal Rust code.

```rust
let hidden_filter = query_expr!(not(tags["hidden"]));
let query = NodeQuery::new().where_expr(hidden_filter);
```

### `query_builder!`

Use `query_builder!` when the same filter is reused.
It exists to turn macro syntax into a reusable `NodeQuery`.
Pick it for shared helper functions, systems that run every frame, and filters with optional subtree overrides.

```rust
let actors = query_builder!(all(base_type[Node3D], tags["actor"]));
let room_actors = query!(ctx.run, &actors, in_subtree(room_root_id));
```

### Cost Model

Most helper macros run the same runtime query first.
That query builds an owned `Vec<NodeID>`.

- `query!` returns that `Vec`.
- `query_iter!` turns that `Vec` into `Vec::into_iter()`.
- `query_each!` loops over that iterator.
- `query_map!` maps that iterator into a new `Vec<T>`.
- `query_first!` uses a first-match runtime path and avoids building the full result list when the runtime supports it.

Use these helpers to pick the clearest gameplay code shape.
Do not pick `query_iter!` expecting a streaming scene scan or zero allocation.

Why not make `query_iter!` fully borrowed?
Because gameplay usually does more runtime work inside the loop.
A borrowed iterator would keep `ctx.run` borrowed for the whole scan, so this would fail:

```rust
for id in query_iter!(ctx.run, all(tags["enemy"])) {
    call_method!(ctx.run, id, method!("tick"), params![]);
}
```

Current owned `query_iter!` releases the runtime borrow before the loop body.
That keeps normal script code usable.

For hot paths, prefer one of these:

- Cache stable `NodeID`s after scene load or spawn.
- Re-run queries only when tags, scene chunks, or spawned groups change.
- Use `in_subtree(...)` to shrink the scanned node set.
- Use rare tags like `boss`, `quest_target`, or `active_enemy` to narrow candidates.
- Add a dedicated runtime API later if a system needs true early-exit or no-alloc traversal.

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
- `within[origin, size]` (global-space box; `Vector2` pair for 2D nodes, `Vector3` pair for 3D nodes)

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
query_each!(ctx.run, all(tags["enemy"], not(tags["dead"])), |id| {
    let _ = with_base_node_mut!(ctx.run, Node3D, id, |node| {
        node.transform.position.y += 0.1;
    });
});
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

### 6) Iterator Adapters

Use `query_iter!` when iterator shape makes the operation clearer.
This is useful for caps, maps, and chained ID lookups.

```rust
let closest_three = query_iter!(ctx.run, all(tags["pickup"], not(tags["claimed"])))
    .take(3)
    .collect::<Vec<_>>();
```

### 7) Query Map

Use `query_map!` when the output is data derived from each node.

```rust
let enemy_positions = query_map!(ctx.run, all(tags["enemy"], base_type[Node3D]), |id| {
    get_global_pos_3d!(ctx.run, id)
});
```

### 8) Query -> Script Interop

```rust
query_each!(ctx.run, all(tags["ally"], tags["alive"]), |id| {
    call_method!(ctx.run, id, method!("on_team_buff"), params![variant!(5.0_f32)]);
});
```

### 9) Render Layer Filters

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

### 10) Spatial Box Filter

Use `within[origin, size]` to match nodes whose **global position** lies inside an axis-aligned box.

- `origin` is the box center in global space.
- `size` is the full box extent along each axis (half on each side of `origin`).
- Box edges are inclusive.
- A `Vector2` pair matches only 2D nodes; a `Vector3` pair matches only 3D nodes.
- Non-spatial nodes (UI, resource, base nodes) never match `within[...]`, and always match `not(within[...])`.

```rust
// All living enemies inside a 10x10x10 box around the player.
let player_pos = get_global_pos_3d!(ctx.run, player_id);
let nearby = query!(ctx.run, all(
    tags["enemy"],
    not(tags["dead"]),
    within[player_pos, Vector3::new(10.0, 10.0, 10.0)]
));

// 2D pickups inside a screen-space region.
let hits = query!(ctx.run, all(
    tags["pickup"],
    within[Vector2::new(640.0, 360.0), Vector2::new(200.0, 200.0)]
));

// Builder form.
let q = NodeQuery::new()
    .tags(["enemy"])
    .within(player_pos, Vector3::new(10.0, 10.0, 10.0));
let ids = ctx.run.NodeQuery().query(&q);
```

## Performance Notes

- Core node/script storage is flat and ID-indexed, so post-query operations stay cheap.
- Query cost depends on match set size and predicate complexity.
- Literal `tags["enemy"]` values hash at compile time; dynamic tag expressions hash at runtime.
- Literal `node_type[...]` and `base_type[...]` predicates compile into growable type bitmasks.
- Literal `layers[...]` and `mask[...]` predicates compile into `BitMask` layer masks.
- Queries with `within[...]` snapshot global node positions once up front, so the scan itself stays read-only and parallel-safe. Queries without `within[...]` pay nothing for this.
- The spatial snapshot refreshes dirty global transforms once, then reads the clean transform cache directly (parallel fill on large scenes, reused buffers, only the dimensions the query tests, subtree-only fill for `in_subtree` scopes).
- A `within[...]` clause also narrows the base-type mask to `Node2D` or `Node3D` automatically, so non-spatial nodes are pruned before predicate eval.
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
