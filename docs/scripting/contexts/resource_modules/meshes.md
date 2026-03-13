# Meshes Module

Access:
- `res.Meshes()`

Macros:
- `load_mesh!(res, source) -> MeshID`
- `reserve_mesh!(res, source) -> MeshID`
- `drop_mesh!(res, source) -> bool`

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

Example:

```rust
let id = load_mesh!(res, "res://meshes/rock.glb");
let _same_id = reserve_mesh!(res, "res://meshes/rock.glb");
let _ = drop_mesh!(res, "res://meshes/rock.glb");
```
