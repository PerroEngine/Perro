# Materials Module

Access:
- `res.Materials()`

Macros:
- `material_load!(res, source) -> MaterialID`
- `material_reserve!(res, source) -> MaterialID`
- `material_drop!(res, source) -> bool`
- `material_create!(res, material) -> MaterialID`

Methods:
- `res.Materials().load(source) -> MaterialID`
- `res.Materials().reserve(source) -> MaterialID`
- `res.Materials().drop(source) -> bool`
- `res.Materials().create(material) -> MaterialID`

What `load` does:
- Loads material data from `source` and returns a stable `MaterialID`.
- If source is already cached, returns existing ID.
- If not cached, allocates an ID and queues renderer material creation with `reserved: false`.
- Creation is async relative to script call.

What `reserve` does:
- Same as `load`, but marks/creates as reserved (`reserved: true`).
- If already created, reserve flag is set immediately.
- If pending, reserve intent is deferred and applied after creation.

What `drop` does:
- Removes source mapping and queues renderer drop when material exists.
- If creation is pending, marks drop-pending so it is dropped right after creation resolves.
- Returns `true` when matching pending/loaded source exists.
- Returns `false` when source is not known.

What `create_material` does:
- Creates a runtime material directly from `Material3D` data.
- Does not create a source-path mapping.
- Intended for transient/generated materials.

Important behavior:
- `load/reserve/drop` are source-cache operations.
- `create_material` is data-driven and bypasses source cache lookup.
- Reserved policy:
- `reserved: false` (from `load`) means the material can be automatically evicted from cache when no references remain.
- `reserved: true` (from `reserve`) means it will not be auto-evicted; only explicit `material_drop!` removes it.

Example:

```rust
let src_id = material_load!(res, "res://materials/smoke.pmat");
let _same_id = material_reserve!(res, "res://materials/smoke.pmat");
let _ = material_drop!(res, "res://materials/smoke.pmat");
```
