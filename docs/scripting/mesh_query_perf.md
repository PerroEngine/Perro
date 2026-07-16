# Mesh Query Perf Snapshot

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |

## Purpose

Mesh queries answer "what part of this model did a ray or point hit?" — the exact triangle, surface index, and material of a `MeshInstance3D`, straight from CPU-side mesh data. This page records the benchmark that shows those queries stay in the microsecond range even for very large meshes, and points at the runtime macros that gameplay uses. Reach for it when you need precise per-surface hits rather than the coarse body hits that collision shapes give.

## Use Cases

- Place bullet holes, scorch marks, or paint splats exactly where a shot lands, including which material was hit: `mesh_instance_surface_on_global_ray_3d!(ctx.run, mesh_id, origin, dir, max_distance)`.
- Resolve a shotgun spread or lidar sweep against one mesh in a single batched call that reuses node and transform lookups: `mesh_instance_surfaces_on_global_rays_3d!(ctx.run, mesh_id, rays, resolve_material)`; pass `resolve_material = false` when only the surface index matters.
- Snap a decal, footprint, or UI marker to the nearest point on a surface: `mesh_instance_surface_at_global_point_3d!(ctx.run, mesh_id, point)`.
- Pick per-surface impact sounds or damage multipliers (metal vs. flesh) by material region: `mesh_instance_material_regions_3d!(ctx.run, mesh_id, material_id)`.

## Practical Example

A hitscan weapon casts one ray at a target mesh instance and reads the surface hit for decal and impact-sound placement.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // `target_id` is a MeshInstance3D node resolved elsewhere (query, ref, cache).
        let target_id = ctx.id;
        let origin = get_global_pos_3d!(ctx.run, ctx.id).unwrap_or(Vector3::ZERO);
        let dir = Vector3::new(0.0, 0.0, -1.0);

        if let Some(hit) = mesh_instance_surface_on_global_ray_3d!(ctx.run, target_id, origin, dir, 50.0) {
            // hit carries the impact point, surface index, and material for fx.
            let _ = hit;
        }
    }
});
```

## Reference

This page records mesh-query benchmark results.
Runtime module: `ctx.run.MeshQuery()`.

## Big Takeaway

- In this benchmark run, a mesh with about `50,000,000` vertices queried in around `~5us` (`time_to_query_us`).
- Most projects will never query a single mesh this large.
- This shows the strength of Perro mesh queries on very large topology.

## What `time_to_query_us` Means

- Time for one mesh query call, in microseconds.
- It does not include full test runtime overhead (mesh build + repeated sampling).
- For several rays against the same mesh node, prefer `mesh_instance_surfaces_on_global_rays_3d!`.
- Batch rays reuse node lookup, mesh cache lookup, node transform, and instance data.
- Pass `resolve_material = false` when surface index is enough.

## Test Output Shape

```text
running tests w/ vertices=<count>, triangles=<count>
surfaces,vertices,triangles,time_to_query_us
```

Surface lanes tested:

- `1`
- `4`
- `16`
- `64`
- `256`

## Re-run Command

```bash
cargo test -p perro_runtime mesh_query::tests::bench_mesh_query_fixed_vertex_count_latency -- --ignored --nocapture
```
