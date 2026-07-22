# `.pnav` navigation meshes

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Records | [Records](#records) |
| Compatibility | [Compatibility](#compatibility) |

## Purpose

`.pnav` stores a static 3D triangle navigation mesh as UTF-8 text. It defines the walkable surface an agent can move across, plus off-mesh links for jumps, doors, ladders, and teleports between disconnected islands. Load it and call `navmesh_find_path_3d!` to get a route instead of writing your own graph search.

## Use Cases

- Enemy and NPC pathfinding: `navmesh_load!(ctx.res, "res://nav/level.pnav")`, then `navmesh_find_path_3d!(ctx.run, navmesh, start, end)` for a walk route.
- Levels connected by jumps or ladders: `link sx sy sz ex ey ez` off-mesh links with `cost` and `bidirectional` to bridge separate mesh islands.
- Mixed agent types: `layers=1,3` on triangles and links filter which agents may traverse them.
- Cost-weighted terrain: `area=2` area IDs (1..32) and per-link `cost` bias routes away from mud, hazards, or slow ground.
- Ledge-attachment tuning: `snap=1` sets the max XZ distance used to attach a link endpoint to a triangle.
- Generated or downloaded meshes: build a `NavMeshID` from bytes with `navmesh_create_from_bytes!`.

## Choice Guide

Use `.pnav` for authored/baked walkable topology and off-mesh links. Use runtime
bytes when navigation arrives from generation or network data. Layers express
agent eligibility; area/link costs express preference. Do not use high cost as
a substitute for an impassable layer.

## Example

Author `res://nav/level.pnav` (two triangle patches bridged by one link):

```text
pnav 1
v 0 0 0
v 2 0 0
v 0 0 2
v 5 0 0
v 7 0 0
v 5 0 2
tri 0 1 2 layers=1 area=1
tri 3 4 5 layers=1 area=2
link 0.5 0 0.5 5.5 0 0.5 layers=1 cost=1.25 snap=1 bidirectional=true
```

Load it and query a path from a script:

```rust
let navmesh = navmesh_load!(ctx.res, "res://nav/level.pnav");
let path = navmesh_find_path_3d!(
    ctx.run,
    navmesh,
    Vector3::new(0.5, 0.0, 0.5),
    Vector3::new(5.5, 0.0, 0.5),
);
// path.points holds the route; path.status reports success or failure.
let _ = path;
```

## Records

- `pnav 1` selects text format version 1.
- `v x y z` adds a vertex.
- `tri a b c` adds a triangle using zero-based vertex indices.
- `link sx sy sz ex ey ez` adds an off-mesh link.
- `#` starts a comment.

Triangle options:

- `layers=1,3` sets traversal layers. The default is all layers.
- `area=2` assigns an area ID from 1 through 32. The default is 1.

Link options:

- `layers=1,3` filters the link by query layers. The default is all layers.
- `cost=1.25` scales travel cost across the link. The value must be finite and greater than zero.
- `snap=1` sets maximum XZ distance used to attach each endpoint to a triangle. The default is 1.
- `bidirectional=false` creates a start-to-end link. The default is `true`.

Use links for jumps, doors, ladders, teleports, and separate mesh islands. A link whose endpoint cannot snap to an enabled triangle stays inactive for that layer query.

## Compatibility

`NavMesh3D` keeps the original vertices-and-triangles shape. Existing create, get, write, parse, and path calls keep working.

`NavMeshResource3D` carries the parallel triangle area list and off-mesh links. `parse_pnav_resource_bytes` and resource-aware API methods retain this metadata. Legacy parsers return the geometry and ignore extended metadata.

Static builds validate the full resource, including area and link fields, before embedding the original bytes.
