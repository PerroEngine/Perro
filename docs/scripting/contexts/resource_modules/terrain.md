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
- If a terrain folder contains `terrain_map.png`, runtime terrain rendering will bind it as the default terrain base-color map.
- Terrain folders may include an optional `settings.pterr` file for mapping defaults:
  - `sample_rate = <float>` (`1`..`12`, where `1` is 1:1 map sampling at `1 pixel per meter`)
  - Layer rules (indexed by order):
    - `layer.0.match_color = #80C840` (aliases: `color`, also accepts `r,g,b` like `128,200,64`)
    - `layer.0.match_tolerance = 6` (aliases: `color_tolerance`, `tolerance`)
    - `layer.0.name = fairway`
    - `layer.0.texture = res://terrain/grass_fairway.png`
    - `layer.0.tile_meters = 5.0`
    - `layer.0.rotation_degrees = 15.0`
    - `layer.0.hard_cut = true` (or `layer.0.filter = nearest`) to disable bilinear blend at tile seams
    - `layer.0.blending = [1,2]` allows this layer to blend only with listed layer indices
    - `layer.0.friction = 0.92`
    - `layer.0.restitution = 0.03`
  - Additional layers use higher indices (`layer.1.*`, `layer.2.*`, ...).
  - Optional global blend pairs:
    - `layer_blendings = [(0,1), (1,2)]`
    - each tuple/pair is exactly two layer indices that are allowed to blend
  - Default behavior is hard layer cuts. Blending only happens for explicitly allowed pairs.
  - Runtime behavior:
    - `terrain_map.png` is treated as a layer-mask/source map.
    - First matching layer by index is selected (matching uses `color_tolerance`).
    - Visual: matching map color can be replaced with layer texture sampling.
    - Physics: matching layer can override terrain collider friction/restitution.
- `TerrainInstance3D` scene fields can override folder defaults:
  - `pixels_per_meter` (legacy alias for `sample_rate`)
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
