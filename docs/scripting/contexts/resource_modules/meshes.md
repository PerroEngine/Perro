# Meshes Module

Access:
- `res.Meshes()`

Macros:
- `mesh_load!(res, source) -> MeshID`
- `mesh_reserve!(res, source) -> MeshID`
- `mesh_drop!(res, source) -> bool`

Methods:
- `res.Meshes().load(source) -> MeshID`
- `res.Meshes().reserve(source) -> MeshID`
- `res.Meshes().drop(source) -> bool`

What `load` does:
- Returns a stable `MeshID` for `source`.
- If already cached, returns existing ID.
- If not cached, allocates an ID and queues mesh creation with `reserved: false`.
- GPU upload/creation happens asynchronously.

What `reserve` does:
- Same lookup/allocation flow as `load`, but sets `reserved: true`.
- If mesh already exists and is ready, reserve state is applied immediately.
- If mesh is still pending, reserve intent is stored and applied on completion.

What `drop` does:
- Removes source mapping and queues mesh drop when mesh exists.
- If pending, marks drop-pending so it is dropped once creation resolves.
- Returns `true` when matching pending/loaded source exists.
- Returns `false` when source is not known.

Important behavior:
- Exact source string is the cache key.
- Repeated `load`/`reserve` returns the same `MeshID` for that source.
- `drop` operates by source path.
- Reserved policy:
- `reserved: false` (from `load`) means the mesh can be automatically evicted from cache when no references remain.
- `reserved: true` (from `reserve`) means it will not be auto-evicted; only explicit `mesh_drop!` removes it.

Practical tip:
- If a complex mesh is used repeatedly and you see lag spikes from drop/recreate churn, call `mesh_reserve!` to keep it cached.

Example:

```rust
let id = mesh_load!(res, "res://meshes/rock.glb");
let _same_id = mesh_reserve!(res, "res://meshes/rock.glb");
let _ = mesh_drop!(res, "res://meshes/rock.glb");
```

