# PUP test suites

All PUP tests live under `res/puptests/`, grouped by category. Scripts in `res/` (including subdirs) are transpiled; only scripts attached in the scene need a `script_path` in the `.scn`.

## Layout

```
res/puptests/
├── node_tests/     — Node API (Node2D, Node3D, Sprite2D, MeshInstance3D, Camera2D, ShapeInstance2D, engine structs)
├── type_tests/      — Types, conversions, casting, syntax edge cases
├── resource_tests/ — Resource APIs (Texture, Mesh, Quaternion, Shape2D, Signal, Array, Map)
└── TEST_SUITES.md  — This file
```

## Node tests (`puptests/node_tests/`)

| File | Extends | What it exercises |
|------|--------|-------------------|
| test_node2d_api.pup | Node2D | transform, global_transform, pivot, visible, z_index, get_node, get_parent, get_var, set_var, call, for (i in X..Y) |
| test_node3d_api.pup | Node3D | transform (position, rotation, scale), Quaternion rotate on transform.rotation, assignments, for loops |
| test_sprite2d_api.pup | Sprite2D | texture, region + Node2D |
| test_mesh_instance_3d_api.pup | MeshInstance3D | mesh + Node3D |
| test_camera2d_api.pup | Camera2D | zoom, active + Node2D |
| test_shape_instance_2d_api.pup | ShapeInstance2D | shape, color, filled + Node2D |
| test_engine_structs.pup | Node2D | Nested engine struct access (transform.position), new Vector2/Color, variable assign |

## Type tests (`puptests/type_tests/`)

- **types.pup** — Primitives, custom structs, inheritance, arrays/maps (static and dynamic), casting, containers. Same concepts as types.cs / types.ts. Attached in main.scn as `res://puptests/type_tests/types.pup`.
- **test_syntax_edge.pup** — Casts (`as` float/int/string/decimal/big), numeric literals with `_`, script vars/methods by name, `node::var` / `node::method(args)` for other nodes.

## Resource tests (`puptests/resource_tests/`)

- **test_resource_api.pup** — Texture, Mesh, Quaternion, Shape2D, Signal (new, connect, emit, emit_deferred), Array, Map. Multiple test functions; signal handlers by name (no `self::`).

## Numeric literals with `_`

PUP supports `1_000_000`, `123.456_789`. For `Decimal`/`BigInt` the codegen strips underscores before `from_str(...)`.

## Loops

PUP uses range-based loops: `for (i in X..Y) { ... }`.

**Why we build:** The test project build is mainly for **transpiler / compile-time validation**. If we write valid frontend (PUP) code, the generated Rust should compile without issues — the end user has no control over that output. There can still be runtime panics or logic bugs; the goal here is to confirm that valid script syntax and API usage produce valid Rust. If the project builds, these scripts are valid from the transpiler’s perspective.
