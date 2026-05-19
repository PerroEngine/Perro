# MeshQuery Module

Runtime handle:

- Direct module: `ctx.run.MeshQuery()`
- Macro route: `mesh_*_3d!`

Use `MeshQuery` for surface hits, batch rays, and coarse mesh regions.
Use `Nodes` for node fields/transforms when you are not intersecting mesh topology.
Use `NodeQuery` to find mesh nodes by name/tag/type.

Macros:

- `mesh_instance_surface_at_global_point_3d!(ctx.run, node_id, global_point) -> Option<MeshSurfaceHit3D>`
- `mesh_instance_surface_on_global_ray_3d!(ctx.run, node_id, ray_origin, ray_direction, max_distance) -> Option<MeshSurfaceHit3D>`
- `mesh_instance_surfaces_on_global_rays_3d!(ctx.run, node_id, rays, resolve_material) -> Vec<Option<MeshSurfaceHit3D>>`
- `mesh_instance_material_regions_3d!(ctx.run, node_id, material_id) -> Vec<MeshMaterialRegion3D>`
- `mesh_data_surface_at_local_point_3d!(ctx.run, mesh_id, local_point) -> Option<MeshDataSurfaceHit3D>`
- `mesh_data_surface_on_local_ray_3d!(ctx.run, mesh_id, ray_origin_local, ray_direction_local, max_distance) -> Option<MeshDataSurfaceHit3D>`
- `mesh_data_surface_regions_3d!(ctx.run, mesh_id, surface_index) -> Vec<MeshDataSurfaceRegion3D>`

Direct API:

```rust
let hit = ctx.run.MeshQuery().instance_surface_on_global_ray(
    mesh_node_id,
    ray_origin,
    ray_direction,
    100.0,
);
```

```rust
let hits = ctx.run.MeshQuery().instance_surfaces_on_global_rays(
    mesh_node_id,
    &rays,
    false,
);
```

Pick correct lane:

- Need one nearest surface from a global point on a live node -> use `mesh_instance_surface_at_global_point_3d!`.
- Need one downward/forward ray hit on a live node -> use `mesh_instance_surface_on_global_ray_3d!`.
- Need several rays against the same live node in one frame -> use `mesh_instance_surfaces_on_global_rays_3d!`.
- Need only surface index from batch hits -> pass `resolve_material = false`.
- Need runtime material from batch hits -> pass `resolve_material = true`.
- Need "where is material X on this mesh instance now?" -> use `mesh_instance_material_regions_3d!`.
- Need "where is surface slot N from mesh data?" -> use `mesh_data_surface_regions_3d!` with `MeshID`.
- Need inverse hit "local point/ray -> raw surface index only" -> use mesh-data lane with `MeshID`.
- Need global transforms, runtime material binds, or multimesh instance index -> use instance lane.
- Need raw topology only, no node transform, no material bind -> use mesh-data lane.

`MeshSurfaceRay3D` fields:

- `origin`: ray origin in global space
- `direction`: ray direction in global space; it does not need to be normalized
- `max_distance`: max ray travel distance

`MeshSurfaceHit3D` fields:

- `instance_index`: instance id for `MultiMeshInstance3D` (0 for regular mesh)
- `surface_index`: matched mesh surface
- `material`: material bound on that surface (`Option<MaterialID>`)
- `global_point`: nearest point on mesh in global space
- `local_point`: nearest point in mesh local space
- `global_normal`: surface normal in global space at nearest point
- `local_normal`: surface normal in mesh local space at nearest point
- `distance`: distance from query point to nearest point

`MeshMaterialRegion3D` fields:

- `instance_index`, `surface_index`, `material`
- `triangle_count`
- `center_global`, `center_local`
- `aabb_min_global`, `aabb_max_global`
- `aabb_min_local`, `aabb_max_local`

`MeshDataSurfaceHit3D` fields:

- `surface_index`: matched mesh surface
- `local_point`: nearest point in mesh local space
- `local_normal`: surface normal in mesh local space at nearest point
- `distance`: distance from query point to nearest point in mesh local space

`MeshDataSurfaceRegion3D` fields:

- `surface_index`
- `triangle_count`
- `center_local`
- `aabb_min_local`, `aabb_max_local`

Examples:

```rust
let p = Vector3::new(2.0, 1.0, -5.0);
if let Some(hit) = mesh_instance_surface_at_global_point_3d!(ctx.run, mesh_node_id, p) {
    // hit.surface_index
    // hit.material
    // hit.global_point
    // hit.global_normal
}
```

```rust
let ray_origin = Vector3::new(2.0, 10.0, -5.0);
let ray_dir = Vector3::new(0.0, -1.0, 0.0);
if let Some(hit) =
    mesh_instance_surface_on_global_ray_3d!(ctx.run, mesh_node_id, ray_origin, ray_dir, 64.0)
{
    // hit.surface_index
    // hit.global_point
}
```

```rust
let y = Vector3::new(0.0, 12.0, 0.0);
let down = Vector3::new(0.0, -1.0, 0.0);
let rays = [
    MeshSurfaceRay3D {
        origin: sample_pos + y,
        direction: down,
        max_distance: 64.0,
    },
    MeshSurfaceRay3D {
        origin: sample_pos + Vector3::new(0.1, 12.0, 0.0),
        direction: down,
        max_distance: 64.0,
    },
    MeshSurfaceRay3D {
        origin: sample_pos + Vector3::new(-0.1, 12.0, 0.0),
        direction: down,
        max_distance: 64.0,
    },
];
let hits = mesh_instance_surfaces_on_global_rays_3d!(
    ctx.run,
    terrain_mesh_id,
    &rays,
    false
);
for hit in hits.into_iter().flatten() {
    // hit.surface_index
    // hit.material is None because resolve_material=false
}
```

```rust
let regions = mesh_instance_material_regions_3d!(ctx.run, mesh_node_id, material_id);
for r in regions {
    // r.surface_index
    // r.center_global
    // r.aabb_min_global / r.aabb_max_global
}
```

```rust
let data_regions = mesh_data_surface_regions_3d!(ctx.run, mesh_id, 2);
for r in data_regions {
    // r.surface_index == 2
    // r.center_local
    // r.aabb_min_local / r.aabb_max_local
}
```

```rust
if let Some(hit) = mesh_data_surface_on_local_ray_3d!(
    ctx.run,
    mesh_id,
    ray_origin_local,
    ray_dir_local,
    100.0
) {
    // hit.surface_index
    // hit.local_point
}
```

Perf rules:

- Use batch rays when probing many offsets against same terrain mesh.
- Use `resolve_material = false` when `surface_index` is enough.
- Cache `NodeID`, `MeshID`, and `MaterialID`; avoid scene queries in hot loops.
- Prefer surface-index classification when terrain surface order is stable.
- Use material lookup for imported content where surface order may shift.
- Use region queries during setup, spawn placement, or infrequent layout logic.
- Avoid region queries every frame; they summarize triangle sets and can scan topology.

More:

- [Mesh Query Perf Snapshot](../../mesh_query_perf.md)
