# Meshes Module

Access:

- `res.Meshes()`

Macros:

- `mesh_load!(res, source) -> MeshID`
- `mesh_reserve!(res, source) -> MeshID`
- `mesh_drop!(res, source) -> bool`
- `mesh_get_data!(res, mesh_id) -> Option<Mesh3D>`
- `mesh_create!(res, data) -> MeshID`
- `mesh_write!(res, mesh_id, data) -> bool`

When to use each:

- `mesh_load!` / `mesh_reserve!`: default path for preauthored meshes (`.gltf/.glb/.pmesh`) used by scene/runtime nodes.
- `mesh_create!`: create a mesh fully from runtime data (no source file).
- `mesh_get_data!` + `mesh_write!`: runtime mutation path for an existing mesh id.
- Typical `get/write` use-cases: procedural edits, deformation, slicing, patching surfaces, generating variants at runtime.

Methods:

- `res.Meshes().load(source) -> MeshID`
- `res.Meshes().reserve(source) -> MeshID`
- `res.Meshes().drop(source) -> bool`
- `res.Meshes().get_data(mesh_id) -> Option<Mesh3D>`
- `res.Meshes().create(data) -> MeshID`
- `res.Meshes().write(mesh_id, data) -> bool`

Runtime mesh data shape:

- `Mesh3D { vertices, indices, surface_ranges }`
- `MeshSurfaceRange { index_start, index_count }`
- `surface_ranges` partitions triangle index buffer into surfaces.
- If `surface_ranges` is empty, runtime treats mesh as one full surface.

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
- Data APIs are copy-based. `mesh_get_data!` returns a copy snapshot.
- `mesh_write!` atomically replaces runtime mesh snapshot for that id.
- Mesh payload does not store material ids. Surface->material assignment stays in `MeshInstance3D.surfaces`.
- Prefer batched edits + one `mesh_write!`; avoid per-frame writes.
- Most projects primarily use authored assets via `mesh_load!`; `mesh_get_data!/mesh_write!` are for explicit runtime geometry updates.

Practical tip:

- If a complex mesh is used repeatedly and you see lag spikes from drop/recreate churn, call `mesh_reserve!` to keep it cached.

Example:

```rust
// `MeshSurfaceRange` comes from Perro prelude.
let id = mesh_load!(res, "res://meshes/rock.glb:mesh[0]");
let _same_id = mesh_reserve!(res, "res://meshes/rock.glb:mesh[0]");
let _ = mesh_drop!(res, "res://meshes/rock.glb:mesh[0]");

if let Some(mut data) = mesh_get_data!(res, id) {
    data.surface_ranges = vec![
        MeshSurfaceRange {
            index_start: 0,
            index_count: (data.indices.len() as u32) / 2,
        },
        MeshSurfaceRange {
            index_start: (data.indices.len() as u32) / 2,
            index_count: (data.indices.len() as u32) - ((data.indices.len() as u32) / 2),
        },
    ];
    let _ = mesh_write!(res, id, data);
}
```

glTF sub-asset access:

- `res://path/to/model.gltf:mesh[0]`
- `res://path/to/model.glb:mesh[1]`

Use the `:mesh[index]` suffix to target a specific mesh inside a glTF/glb.

Skinning note:

- If the mesh contains `JOINTS_0/WEIGHTS_0` and a `MeshInstance3D` is bound to a `Skeleton3D`,
  the mesh will be skinned using that skeleton.
