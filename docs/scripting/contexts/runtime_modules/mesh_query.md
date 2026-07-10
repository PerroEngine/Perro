# Mesh Query Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `instance_surface_at_global_point` | [`instance_surface_at_global_point`](#instance_surface_at_global_point) |
| `instance_surface_on_global_ray` | [`instance_surface_on_global_ray`](#instance_surface_on_global_ray) |
| `instance_surfaces_on_global_rays` | [`instance_surfaces_on_global_rays`](#instance_surfaces_on_global_rays) |
| `instance_material_regions` | [`instance_material_regions`](#instance_material_regions) |
| `data_surface_at_local_point` | [`data_surface_at_local_point`](#data_surface_at_local_point) |
| `data_surface_on_local_ray` | [`data_surface_on_local_ray`](#data_surface_on_local_ray) |
| `data_surface_regions` | [`data_surface_regions`](#data_surface_regions) |
| `mesh_instance_surface_at_global_point_3d` | [`mesh_instance_surface_at_global_point_3d`](#mesh_instance_surface_at_global_point_3d) |
| `mesh_instance_surface_on_global_ray_3d` | [`mesh_instance_surface_on_global_ray_3d`](#mesh_instance_surface_on_global_ray_3d) |
| `mesh_instance_surfaces_on_global_rays_3d` | [`mesh_instance_surfaces_on_global_rays_3d`](#mesh_instance_surfaces_on_global_rays_3d) |
| `mesh_instance_material_regions_3d` | [`mesh_instance_material_regions_3d`](#mesh_instance_material_regions_3d) |
| `mesh_data_surface_at_local_point_3d` | [`mesh_data_surface_at_local_point_3d`](#mesh_data_surface_at_local_point_3d) |
| `mesh_data_surface_on_local_ray_3d` | [`mesh_data_surface_on_local_ray_3d`](#mesh_data_surface_on_local_ray_3d) |
| `mesh_data_surface_regions_3d` | [`mesh_data_surface_regions_3d`](#mesh_data_surface_regions_3d) |

## Overview

This runtime module belongs to `ctx.run` and documents mesh query calls.

`MeshSurfaceHit3D` includes `triangle_index`, `(a, b, c)` `barycentric`
weights, interpolated `uv0`, and `paint_uv`. `paint_uv` reads glTF UV1 and
falls back to UV0 for runtime meshes, built-ins, and PMESH assets.

`MeshInstance3D` queries linked to `Skeleton3D` use current bone poses. Hit
triangle IDs and UVs keep original mesh topology. Posed queries cap at
1,000,000 vertices; larger posed meshes return no hit.

Build pointer rays with
`ctx.run.Nodes().camera_screen_ray_3d(camera_id, pixel, viewport_size)`.
Pixels use a top-left origin. The result supports perspective, orthographic,
and off-axis frustum cameras and passes directly to
`instance_surface_on_global_ray`.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.MeshQuery()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

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

