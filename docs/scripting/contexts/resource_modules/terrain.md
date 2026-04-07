# Terrain Module

Access:
- `res.Terrain()`

Methods:
- `res.Terrain().brush_op(terrain, center_world, brush_size_meters, shape, op) -> Option<TerrainEditSummary>`
- `res.Terrain().raycast(terrain, origin_world, direction_world, max_distance) -> Option<TerrainRayHit>`

Current terrain model (runtime):
- Terrain is fixed-grid per chunk: `64x64` cells (`65x65` shared vertices), `1 vertex per meter`.
- Scene-authored terrain can be loaded by setting `TerrainInstance3D.terrain` to a folder path containing `.ptchunk` files.
- `.ptchunk` is key-value style, one sample per line, for example:
  - `[x,z] = y`
  - `[x,z] y`
  - optional chunk header: `chunk = [cx,cz]` (or `coord = [cx,cz]`)
- Runtime editing is height-only on existing vertices.
- Runtime does not create/remove vertices or triangles.
- Triangles use shared (deduped) grid vertices.

Brush behavior:
- `BrushOp::SetHeight { y, .. }`: sets touched vertex heights directly to `y`.
- `BrushOp::Add { delta, .. }`: raises heights with distance falloff from brush center.
- `BrushOp::Remove { delta, .. }`: lowers heights with distance falloff from brush center.
- `BrushOp::Smooth { strength, .. }`: moves heights toward local brush-average with distance falloff.
- `BrushOp::Decimate { .. }`: currently no-op in fixed-grid mode (reserved for future LOD/topology workflows).

Notes:
- `TerrainEditSummary.inserted_points` currently means "touched vertices".
- Existing `basis`/`feature_offset` brush fields are accepted for API compatibility, but no runtime topology pass is performed.
- Terrain APIs are exposed through `TerrainModule` on `ResourceContext`.

See also:
- [Resource Context](../resource_context.md)
