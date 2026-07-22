# Physics + Queries

Physics handles collision and movement constraints.

Queries find nodes and world hits.

## Goal

Use bodies, areas, raycasts, shape casts, and node queries.

## Decision Model

Physics queries answer spatial questions. Node queries answer scene-membership
questions. Use a ray or shape cast for "what occupies this space?" Use tags and
node queries for "which active nodes belong to this gameplay set?" Do not replace
a fixed scene ref with either query type.

Use collision signals/area events for changes. Poll only when gameplay needs a
continuous answer, such as ground contact or aim line of sight.

## Bodies

Use static bodies for level geometry.

Use rigid bodies for simulated motion.

Use areas for triggers.

Use collision shapes as children of physics bodies.

## Layers + Masks

Use `BitMask` for physics layers and query filters.

Keep layer names documented in game code.

Avoid magic raw bit values in gameplay scripts.

## Casts

Use raycasts for line tests:

- aim
- ground checks
- line of sight
- interact tests

Use shape casts for volume tests:

- ledge checks
- movement clearance
- melee hitboxes

## Areas

Use areas for:

- pickups
- zone triggers
- hurt fields
- audio zones
- camera regions

## Node Queries

Use node queries for scene selection:

```rust
query_each!(ctx.run, all(tags["enemy"]), |id| {
    call_method!(ctx.run, id, method!("wake"), params![]);
});
```

Keep query use out of very hot loops unless the result set is small or cached.

## Reference

- [Physics Nodes](/docs/scripting/physics_nodes.md)
- [Physics Module](/docs/scripting/contexts/runtime_modules/physics.md)
- [Query System](/docs/scripting/query_system.md)
- [Node Query Module](/docs/scripting/contexts/runtime_modules/node_query.md)
- [BitMask](/docs/scripting/bitmask.md)
