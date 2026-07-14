# Navmesh module

Load a `.pnav` resource through `ctx.res.NavMeshes()`. Query it through `ctx.run.NavMesh()`.

## Basic path

```rust
let navmesh = ctx.res.NavMeshes().load("res://nav/level.pnav");
let path = ctx.run.NavMesh().find_path_3d(
    navmesh,
    start,
    goal,
    NavMeshPathOptions::default(),
);
```

The runtime projects both endpoints onto enabled triangles, searches the shared-edge graph, and applies an XZ funnel/string-pull pass when `simplify` is true. Set `simplify` to false to receive shared-edge midpoint corridor points.

## Area costs and query obstacles

```rust
let path = ctx.run.NavMesh().find_path_query_3d(
    navmesh,
    start,
    goal,
    NavMeshQueryOptions {
        area_costs: vec![NavMeshAreaCost {
            area: 2,
            multiplier: 4.0,
        }],
        obstacles: vec![NavMeshObstacle3D::Circle {
            center: crate_pos,
            radius: 0.75,
        }],
        ..Default::default()
    },
);
```

Area multipliers must be finite and greater than zero. Unlisted areas use `1.0`. Higher values make routes through that area less desirable.

Query obstacles support XZ circles and axis-aligned boxes. They conservatively block overlapping triangles for one query. They do not mutate or carve mesh geometry. Use them for moving blockers where conservative rerouting is acceptable.

## Off-mesh links

Resource links participate by default. Set `use_off_mesh_links` to `false` in `NavMeshQueryOptions` to exclude them. One-way links preserve authored start-to-end direction.

The returned point list includes both link endpoints. A query obstacle that intersects an off-mesh segment disables that link for the query.

## Limits

- Navigation projects and smooths on XZ while retaining vertex Y values.
- Obstacles block whole triangles; no local geometry carving occurs.
- Mesh or scene geometry auto-bake is not included.
- Binary `.pnav` is not included.
