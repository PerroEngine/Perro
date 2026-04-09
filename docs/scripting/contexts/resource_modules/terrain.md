# Terrain Module

Access:

- `res.Terrain()`

Methods:

- `res.Terrain().brush_op(terrain, center_world, brush_size_meters, shape, op) -> Option<TerrainEditSummary>`
- `res.Terrain().raycast(terrain, origin_world, direction_world, max_distance) -> Option<TerrainRayHit>`

Current terrain model (runtime):

- Chunk span defaults to `512m`.
- Terrain chunks can be either:
  - grid chunks (default flat chunk generation uses `1 vertex per meter`)
  - arbitrary mesh chunks (vertices/triangles are authored or imported)
- Terrain mesh chunks must remain heightfield-like (`1 height per x,z`) for terrain workflows.
- Scene-authored terrain can be loaded by setting `TerrainInstance3D.terrain` to a terrain folder.
- Terrain folders can contain chunk files (`.ptchunk`) and/or an authoring mesh (`terrain.glb` or `terrain.gltf`).
- Terrain folders may also include layer/map assets used by terrain tooling workflows.
- `.ptchunk` filenames must be chunk-space coordinates: `<chunk_x>_<chunk_z>.ptchunk` (for example `0_0.ptchunk`, `0_1.ptchunk`, `-1_2.ptchunk`).
- `.ptchunk` is key-value style, one sample per line, for example:
  - `[x,z] = y`
  - `[x,z] y`
  - optional chunk header: `chunk = [cx,cz]` (or `coord = [cx,cz]`)
- `[x,z] = y` accepts arbitrary decimal coordinates and preserves exact sample positions (no grid snap).
- `.ptchunk` also supports explicit mesh payload:
  - `vertex = [x,y,z]`
  - `tri = [a,b,c]`
- Any missing vertex samples default to `0.0` height.
- An empty valid chunk file (for example `0_0.ptchunk`) loads as a completely flat chunk.
- Runtime editing is height-only on existing vertices.
- Runtime does not create/remove vertices or triangles.
- Seam syncing for cross-chunk brush edits applies to compatible grid chunks.

Brush behavior:

- `BrushOp::SetHeight { y, .. }`: sets touched vertex heights directly to `y`.
- `BrushOp::Add { delta, .. }`: raises heights with distance falloff from brush center.
- `BrushOp::Remove { delta, .. }`: lowers heights with distance falloff from brush center.
- `BrushOp::Smooth { strength, .. }`: moves heights toward local brush-average with distance falloff.
- `BrushOp::Decimate { .. }`: currently no-op (reserved for future LOD/topology workflows).

Notes:

- `TerrainEditSummary.inserted_points` currently means "touched vertices".
- Existing `basis`/`feature_offset` brush fields are accepted for API compatibility, but no runtime topology pass is performed.
- Terrain APIs are exposed through `TerrainModule` on `ResourceContext`.

See also:

- [Resource Context](../resource_context.md)
