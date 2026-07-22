# Runtime Nodes

Perro scripts talk to the scene through `NodeID`.

## Goal

Know how to mutate nodes without long borrows.

## Access Choice

- own known node -> `ctx.id` + typed node access
- fixed other node -> scene-injected `NodeID`
- owned child/parent -> relation API
- spawned or tagged set -> query
- runtime-selected type/member -> dynamic API

Copy or clone values out of state/node closures before the next `ctx.run` call.
The short borrow is both a correctness rule and a clear data-flow boundary.

## Node Handles

`NodeID` is a runtime handle.

Scripts get ids from:

- `ctx.id`
- scene refs in state
- parent/child calls
- node queries
- scene loads

Use ids right away or store them in state when the scene owns the target.

If a node is removed, old ids stop resolving.

## Mutate In Place

Node APIs use short calls or closure APIs.

They borrow the target only for the operation.

```rust
let pos = get_global_pos_3d!(ctx.run, ctx.id).unwrap_or(Vector3::ZERO);
set_global_pos_3d!(ctx.run, ctx.id, pos + Vector3::new(0.0, 1.0, 0.0));
```

Typed node mutation follows the same shape inside the runtime API.

The closure cannot hold a node borrow across later runtime calls.

This avoids long borrows and runtime borrow-check style failures.

Fail mode is simple:

- id not found
- id points to a different node type
- target was removed

## Queries

Queries return node ids.

Use them for dynamic groups such as enemies, pickups, buttons, or physics probes.

```rust
for enemy in query!(ctx.run, all(tag["enemy"])) {
    let _ = call_method!(ctx.run, enemy, method!("wake"), params![]);
}
```

Query ids are valid when returned.

Check bool/option returns when later code may remove nodes.

## Parent And Child IDs

Parent and child APIs let scripts navigate scene trees.

Use these for local scene work, such as finding spawned children under a room root.

```rust
if let Some(children) = ctx.run.Nodes().get_node_children_ids(ctx.id) {
    for child in children {
        let _ = ctx.run.Nodes().set_node_name(child, "visited");
    }
}
```

## Reference

- [Nodes Module](/docs/scripting/contexts/runtime_modules/nodes)
- [Node Query Module](/docs/scripting/contexts/runtime_modules/node_query)
- [Query System](/docs/scripting/query_system)
- [Node Types](/docs/scripting/nodes)
