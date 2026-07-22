# Navmesh Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Basic Path | [Basic Path](#basic-path) |
| Area Costs and Query Obstacles | [Area Costs and Query Obstacles](#area-costs-and-query-obstacles) |
| Off-mesh Links | [Off-mesh Links](#off-mesh-links) |
| Practical Example | [Practical Example](#practical-example) |
| Limits | [Limits](#limits) |

## Purpose

The navmesh module answers the core AI-movement question: "how do I walk from
here to there without cutting through walls?" Given a baked `.pnav` mesh, it
returns an ordered list of world points an agent can follow to reach a goal.
This is what drives enemy pathfinding, companion follow behaviour, and any
agent that has to route around level geometry rather than beeline to a target.

Paths are computed on demand and reflect optional per-area travel costs and
temporary obstacles, so routes can prefer roads over mud or detour around a
moving blocker for a single query.

## Use Cases

- Enemy chases the player around corners: `ctx.run.NavMesh().find_path_3d(navmesh, enemy_pos, player_pos, NavMeshPathOptions::default())`, then steer toward each returned point.
- Companion follows the party through a level: recompute a path to the leader when they move far enough.
- Prefer safe terrain: bias a route away from hazard tiles with `area_costs` in `find_path_query_3d` (a higher multiplier makes that area less desirable).
- Reroute around a dropped crate or another unit: pass a temporary `obstacles` circle/box in `NavMeshQueryOptions` so the query avoids it without re-baking the mesh.
- Ladders, jumps, and teleporters: authored off-mesh links are followed by default and appear as endpoints in the returned point list.
- Straight-line vs. corridor movement: set `simplify` true for a funnelled, string-pulled path, or false to get shared-edge corridor midpoints.

## Ownership And Choice

The navmesh owns walkable topology and area costs. An agent/controller owns its current goal and movement along the returned path. Query a path when the goal or navigation world changes, not every frame without need. Use direct steering for unobstructed local motion; use navmesh paths when authored traversal rules and obstacles must constrain the route.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.NavMesh()`
- Load the `.pnav` resource through `ctx.res.NavMeshes()`; query it through `ctx.run.NavMesh()`.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Basic Path

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

`find_path_3d` returns a `NavMeshPath3D` with `status` (`Complete`, `Partial`, or `Failed`), the ordered `points`, and total `distance`.

## Area Costs and Query Obstacles

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

## Off-mesh Links

Resource links participate by default. Set `use_off_mesh_links` to `false` in `NavMeshQueryOptions` to exclude them. One-way links preserve authored start-to-end direction.

The returned point list includes both link endpoints. A query obstacle that intersects an off-mesh segment disables that link for the query.

## Practical Example

An enemy recomputes a path to the player and walks toward the next waypoint each
frame. The path is only useful when a route was found, so branch on `status`.

```rust
#[State]
struct ChaserState {
    #[default = NavMeshID::nil()]
    pub navmesh: NavMeshID,
    #[default = NodeID::nil()]
    pub target: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let navmesh = ctx.res.NavMeshes().load("res://nav/level.pnav");
        with_state_mut!(ctx.run, ChaserState, ctx.id, |s| s.navmesh = navmesh);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let (navmesh, target) = with_state!(ctx.run, ChaserState, ctx.id, |s| (s.navmesh, s.target)).unwrap_or_default();
        if navmesh.is_nil() || target.is_nil() {
            return;
        }

        let here = get_global_pos_3d!(ctx.run, ctx.id);
        let goal = get_global_pos_3d!(ctx.run, target);
        let path = ctx.run.NavMesh().find_path_3d(
            navmesh,
            here,
            goal,
            NavMeshPathOptions::default(),
        );

        if path.status != NavMeshPathStatus::Failed {
            // points[0] is the current position; step toward points[1].
            if let Some(next) = path.points.get(1).copied() {
                let dt = delta_time!(ctx.run);
                let step = (next - here).normalized() * 3.0 * dt;
                set_global_pos_3d!(ctx.run, ctx.id, here + step);
            }
        }
    }
});
```

## Limits

- Navigation projects and smooths on XZ while retaining vertex Y values.
- Obstacles block whole triangles; no local geometry carving occurs.
- Mesh or scene geometry auto-bake is not included.
- Binary `.pnav` is not included.
