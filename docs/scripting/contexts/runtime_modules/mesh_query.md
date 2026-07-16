# Mesh Query Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Hit Data and Rays | [Hit Data and Rays](#hit-data-and-rays) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `instance_surface_at_global_point` | [`instance_surface_at_global_point`](#instance_surface_at_global_point) |
| `instance_surface_global_point` | [`instance_surface_global_point`](#instance_surface_global_point) |
| `instance_surface_on_global_ray` | [`instance_surface_on_global_ray`](#instance_surface_on_global_ray) |
| `instance_surfaces_on_global_rays` | [`instance_surfaces_on_global_rays`](#instance_surfaces_on_global_rays) |
| `instance_material_regions` | [`instance_material_regions`](#instance_material_regions) |
| `data_surface_at_local_point` | [`data_surface_at_local_point`](#data_surface_at_local_point) |
| `data_surface_on_local_ray` | [`data_surface_on_local_ray`](#data_surface_on_local_ray) |
| `data_surface_regions` | [`data_surface_regions`](#data_surface_regions) |
| `mesh_instance_surface_at_global_point_3d` | [`mesh_instance_surface_at_global_point_3d`](#mesh_instance_surface_at_global_point_3d) |
| `mesh_instance_surface_global_point_3d` | [`mesh_instance_surface_global_point_3d`](#mesh_instance_surface_global_point_3d) |
| `mesh_instance_surface_on_global_ray_3d` | [`mesh_instance_surface_on_global_ray_3d`](#mesh_instance_surface_on_global_ray_3d) |
| `mesh_instance_surfaces_on_global_rays_3d` | [`mesh_instance_surfaces_on_global_rays_3d`](#mesh_instance_surfaces_on_global_rays_3d) |
| `mesh_instance_material_regions_3d` | [`mesh_instance_material_regions_3d`](#mesh_instance_material_regions_3d) |
| `mesh_data_surface_at_local_point_3d` | [`mesh_data_surface_at_local_point_3d`](#mesh_data_surface_at_local_point_3d) |
| `mesh_data_surface_on_local_ray_3d` | [`mesh_data_surface_on_local_ray_3d`](#mesh_data_surface_on_local_ray_3d) |
| `mesh_data_surface_regions_3d` | [`mesh_data_surface_regions_3d`](#mesh_data_surface_regions_3d) |

## Purpose

Mesh queries answer "exactly where on this model did a ray or point land?" at
triangle precision. Where physics raycasts hit collision shapes, mesh queries
hit the rendered geometry itself, returning the triangle, barycentric weights,
interpolated UVs, and surface point. That is what you need to place a bullet hole
on a wall, let a player click a specific panel of a control console, or sample
which texel of a paintable surface was struck. Queries against a skinned
`MeshInstance3D` use the live skeleton pose, so hits track an animated character.

## Use Cases

- Click-to-place / click-to-select on a 3D model: build a pointer ray with `ctx.run.Nodes().camera_screen_ray_3d(camera_id, pixel, viewport_size)` and hit the mesh via `instance_surface_on_global_ray`.
- Stick a decal or bullet hole precisely on a surface: use the returned hit point (and `paint_uv`) to spawn the decal exactly where the ray landed.
- Damage zones / paintable surfaces: read the hit's `paint_uv` or `instance_material_regions` to know which material or texel was struck.
- Animated hitboxes: raycast a posed skeletal character so hits follow the current animation instead of the rest pose.
- Reconstruct a stored hit under the current pose: save `triangle_index` + `barycentric`, later resolve the authoritative point with `instance_surface_global_point`.
- Tool / procedural sampling: query raw mesh data in local space with `data_surface_on_local_ray` or `data_surface_at_local_point`.

## Hit Data and Rays

`MeshSurfaceHit3D` includes `triangle_index`, `(a, b, c)` `barycentric`
weights, interpolated `uv0`, and `paint_uv`. `paint_uv` reads glTF UV1 and
falls back to UV0 for runtime meshes, built-ins, and PMESH assets.

`MeshInstance3D` queries linked to `Skeleton3D` use current bone poses. Hit
triangle IDs and UVs keep original mesh topology. Posed queries cap at
1,000,000 vertices; larger posed meshes return no hit.

Resolve a saved hit without a second ray via
`instance_surface_global_point(node_id, triangle_index, barycentric)`.
It uses the same query triangle numbering and live skeleton pose. It rejects
non-finite or non-unit barycentric values, non-`MeshInstance3D` nodes, and
meshes over 1,000,000 vertices.

Build pointer rays with
`ctx.run.Nodes().camera_screen_ray_3d(camera_id, pixel, viewport_size)`.
Pixels use a top-left origin. The result supports perspective, orthographic,
and off-axis frustum cameras and passes directly to
`instance_surface_on_global_ray`.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.MeshQuery()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

Click-to-inspect: cast a ray from the camera through the mouse pixel, hit the
rendered mesh, and read back the exact surface point where the player clicked.

```rust
#[State]
struct PickState {
    #[default = NodeID::nil()]
    pub camera: NodeID,
    #[default = NodeID::nil()]
    pub mesh: NodeID,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let camera = with_state!(ctx.run, PickState, ctx.id, |s| s.camera);
        let target = with_state!(ctx.run, PickState, ctx.id, |s| s.mesh);
        let pixel = mouse_position!(ctx.ipt);
        let viewport = viewport_size!(ctx.ipt);

        if let Some(ray) = ctx.run.Nodes().camera_screen_ray_3d(camera, pixel, viewport) {
            let hit = ctx.run.MeshQuery().instance_surface_on_global_ray(
                target,
                ray.origin,
                ray.direction,
                ray.max_distance,
            );
            if let Some(hit) = hit {
                // hit.triangle_index / hit.paint_uv identify what was struck;
                // resolve the world point to place a decal there.
                let _ = ctx.run.MeshQuery().instance_surface_global_point(
                    target,
                    hit.triangle_index,
                    hit.barycentric,
                );
            }
        }
    }
});
```

## API Reference

### `instance_surface_at_global_point`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn instance_surface_at_global_point( &mut self, node_id: NodeID, global_point: Vector3, ) -> Option<MeshSurfaceHit3D>` |
| Params | `&mut self, node_id: NodeID, global_point: Vector3,` |
| Returns | `Option<MeshSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `instance_surface_global_point`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn instance_surface_global_point(&mut self, node_id: NodeID, triangle_index: u32, barycentric: Vector3) -> Option<Vector3>` |
| Params | Mesh instance node, query triangle index, and `(a, b, c)` barycentric weights. |
| Returns | Posed global-space surface point. |
| Use when | Reconstructing an authoritative point from a saved mesh hit. |
| Fails when / edge behavior | Returns `None` for invalid nodes, triangle IDs, barycentric weights, mesh data, skeleton data, non-finite output, or meshes over 1,000,000 vertices. |

### `instance_surface_on_global_ray`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn instance_surface_on_global_ray( &mut self, node_id: NodeID, ray_origin: Vector3, ray_direction: Vector3, max_distance: f32, ) -> Option<MeshSurfaceHit3D>` |
| Params | `&mut self, node_id: NodeID, ray_origin: Vector3, ray_direction: Vector3, max_distance: f32,` |
| Returns | `Option<MeshSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `instance_surfaces_on_global_rays`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn instance_surfaces_on_global_rays( &mut self, node_id: NodeID, rays: &[MeshSurfaceRay3D], resolve_material: bool, ) -> Vec<Option<MeshSurfaceHit3D>>` |
| Params | `&mut self, node_id: NodeID, rays: &[MeshSurfaceRay3D], resolve_material: bool,` |
| Returns | `Vec<Option<MeshSurfaceHit3D>>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `instance_material_regions`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn instance_material_regions( &mut self, node_id: NodeID, material: MaterialID, ) -> Vec<MeshMaterialRegion3D>` |
| Params | `&mut self, node_id: NodeID, material: MaterialID,` |
| Returns | `Vec<MeshMaterialRegion3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `data_surface_at_local_point`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn data_surface_at_local_point( &mut self, mesh_id: MeshID, local_point: Vector3, ) -> Option<MeshDataSurfaceHit3D>` |
| Params | `&mut self, mesh_id: MeshID, local_point: Vector3,` |
| Returns | `Option<MeshDataSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `data_surface_on_local_ray`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn data_surface_on_local_ray( &mut self, mesh_id: MeshID, ray_origin_local: Vector3, ray_direction_local: Vector3, max_distance: f32, ) -> Option<MeshDataSurfaceHit3D>` |
| Params | `&mut self, mesh_id: MeshID, ray_origin_local: Vector3, ray_direction_local: Vector3, max_distance: f32,` |
| Returns | `Option<MeshDataSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `data_surface_regions`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `pub fn data_surface_regions( &mut self, mesh_id: MeshID, surface_index: u32, ) -> Vec<MeshDataSurfaceRegion3D>` |
| Params | `&mut self, mesh_id: MeshID, surface_index: u32,` |
| Returns | `Vec<MeshDataSurfaceRegion3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_surface_at_global_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_instance_surface_at_global_point_3d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_surface_global_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_instance_surface_global_point_3d!(ctx.run, id, triangle_index, barycentric)` |
| Params | Runtime context, mesh instance node, query triangle index, and `(a, b, c)` barycentric weights. |
| Returns | `Option<Vector3>` posed global-space point. |
| Use when | Reconstructing an exact saved surface hit under the live skeleton pose. |
| Fails when / edge behavior | Same validation as `instance_surface_global_point`. |

### `mesh_instance_surface_on_global_ray_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_instance_surface_on_global_ray_3d!(ctx.run, id, origin, direction, max_distance)` |
| Params | `ctx, id, origin, direction, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_surfaces_on_global_rays_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_instance_surfaces_on_global_rays_3d!(ctx.run, id, rays, resolve_material)` |
| Params | `ctx, id, rays, resolve_material` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_material_regions_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_instance_material_regions_3d!(ctx.run, id, material)` |
| Params | `ctx, id, material` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_data_surface_at_local_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_data_surface_at_local_point_3d!(ctx.run, mesh_id, point_local)` |
| Params | `ctx, mesh_id, point_local` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_data_surface_on_local_ray_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_data_surface_on_local_ray_3d!(ctx.run, mesh_id, origin_local, direction_local, max_distance)` |
| Params | `ctx, mesh_id, origin_local, direction_local, max_distance` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_data_surface_regions_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.MeshQuery()` |
| Signature | `mesh_data_surface_regions_3d!(ctx.run, mesh_id, surface_index)` |
| Params | `ctx, mesh_id, surface_index` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

