# Terrain Video Log

## Purpose

Track what each terrain commit changed so video explanations are easy later.

## Commit: Terrain Init

### What was added

- Created `perro_terrain` crate.
- Added `TerrainChunk` with vertex and triangle storage.
- Added `Vertex` and `Triangle` primitives.
- Triangles store vertex indices instead of duplicated vertex data.

### Why it matters

- Avoids duplicate vertex storage per triangle.
- Starts from the minimal base chunk representation:
  - 4 vertices
  - 2 triangles
- Base chunk is centered in local space (`-32..32` for 64m).

### Validation added

- Flat chunk shape/count tests.
- Triangle index validation tests.
- Degenerate-triangle validation tests.

## Commit: Insert Vertex + Auto Optimize

### What was added

- Added `insert_vertex(...)` API.
- Insert splits impacted triangle region and retriangulates locally.
- Auto-optimization removes inserted vertex if geometry is coplanar.

### Why it matters

- Gives adaptive detail only where edits need it.
- Flat inserts do not permanently increase mesh complexity.
- Non-coplanar inserts are preserved (detail retained).

### Validation added

- Flat center insert is optimized away (returns to 4 verts / 2 tris).
- Raised center insert is retained (5 verts / 4 tris).

## Commit: Brush API

### What was added

- Added separate `brush.rs`.
- Added `BrushShape` enum and `insert_brush(center, size, shape)`.
- Square brush creates corner samples only (4 points).
- Added `Circle` and `Triangle` brush shapes.
- Circle sample count scales by size:
  - `3m` -> `5` circumference points
  - `5m` -> `6` circumference points
  - `10m` -> `8` circumference points
  - larger sizes scale up to `12/16/32` circumference points
- Detail points snap to tiered grids by brush size:
  - `>1m` -> `1.0m` grid
  - `0.5..1m` -> `0.5m` grid
  - `0.25..0.5m` -> `0.25m` grid
  - `<=0.25m` -> `0.1m` grid

### Why it matters

- Brushes centralize multi-vertex creation and optimization in one API.
- Square brush enforces size-basis density and avoids hidden over-sampling.
- Circle brush adapts detail with size, so small brushes stay cheap and larger brushes stay smooth.
- Triangle brush gives a fast low-point sculpt shape for directional edits.
- Dynamic sample counts are returned as `Vec`, avoiding fixed-size constraints.

### Validation added

- Square brush inserts exactly 4 points and stays at 4 verts / 2 tris on flat terrain.
- Square brush with size `10m` snaps sampled coordinates to non-decimal grid values.
- Circle tests verify adaptive sample counts (`3m/5m/10m` and larger sizes).
- Triangle test verifies 3-point insert behavior and coplanar optimization on flat terrain.
- Raised brush tests verify non-coplanar inserts are preserved for square/circle/triangle.

## Commit: Topology Safety

### What was added

- Hardened coplanar vertex removal with extra guards before committing simplification.
- Added boundary preservation check around the removed vertex fan.
- Added area consistency check (incident region vs replacement region).
- Added replacement normal consistency check against the base plane normal.
- Added manifold safety check on candidate triangle topology (no duplicate triangles, no edge used by more than two triangles).

### Why it matters

- Prevents optimization from changing shape unexpectedly.
- Prevents topology breaks during aggressive local simplification.
- Keeps optimization behavior predictable while still removing redundant coplanar detail.

### Validation added

- Added repeated mixed insert test that verifies resulting mesh stays valid and manifold.

## Commit: First Perf

### What was added

- Added ignored perf tests in `tests/perf.rs` for repeatable timing snapshots.
- Added three benchmark-style cases:
  - `insert_vertex` coplanar bulk
  - `insert_vertex` non-coplanar bulk
  - `insert_brush` circle bulk
- Each test prints total time, per-op/per-brush time, and final mesh counts.

### Why it matters

- Gives a baseline before further optimization work.
- Makes it easy to compare future changes against concrete numbers.
- Separates cheap coplanar paths from expensive non-coplanar topology growth paths.

### Validation added

- Ran perf tests in release mode with `--ignored --nocapture`.
- Captured first baseline:
  - Coplanar bulk: `1600` iters, `10.336 ms`, `6.460 us/op`, final `4 verts / 2 tris`
  - Non-coplanar bulk: `1600` iters, `161.638 ms`, `101.024 us/op`, final `1604 verts / 3202 tris`
  - Circle brush bulk: `400` brushes (`2400` generated points), `162.734 ms`, `406.834 us/brush`, final `2371 verts / 1638 tris`

## Commit: Perf Optimization Pass

### What was added

- Added fast-path precheck to skip coplanar optimization attempts when insert point is clearly non-coplanar with hit triangles.
- Added duplicate-insert short-circuit localized to hit triangles (prevents redundant reinsertion in dense brush overlaps).
- Replaced full-mesh manifold scan during coplanar collapse with a local manifold check on replacement triangles.
- Switched split/collapse triangle updates to local in-place patching (`swap_remove` + append replacements) instead of whole-array rebuilds.

### Why it matters

- Cuts unnecessary work in common non-coplanar insert paths.
- Avoids repeated local inserts in brush-heavy edits.
- Preserves topology safety while reducing optimization overhead.

### Validation added

- Re-ran release perf tests after optimization:
  - Coplanar bulk: `1600` iters, `14.564 ms`, `9.103 us/op`, final `4 verts / 2 tris`
  - Non-coplanar bulk: `1600` iters, `129.205 ms`, `80.753 us/op`, final `1604 verts / 3202 tris`
  - Circle brush bulk: `400` brushes (`2400` generated points), `80.233 ms`, `200.582 us/brush`, final `833 verts / 1660 tris`

## Commit: Caching + Local Spatial Fast Paths

### What was added

- Added last-hit triangle cache for point insert queries.
- Added strict-interior fast return when the next insert lands inside cached triangle.
- Added triangle AABB prefilter before barycentric checks.
- Removed per-triangle temporary `Vec` allocations in split candidate generation (stack array path).

### Why it matters

- Reduces repeated full-triangle scans in localized editing workflows.
- Cuts hot-path math work by skipping barycentric checks for obvious misses.
- Lowers allocator pressure during heavy insert loops.

### Validation added

- Re-ran release perf tests:
  - Coplanar bulk: `1600` iters, `7.561 ms`, `4.726 us/op`, final `4 verts / 2 tris`
  - Non-coplanar bulk: `1600` iters, `77.587 ms`, `48.492 us/op`, final `1604 verts / 3202 tris`
  - Circle brush bulk: `400` brushes (`2400` generated points), `60.620 ms`, `151.551 us/brush`, final `833 verts / 1660 tris`
  - 4096 single-plane coplanar: `23.373 ms`, `5.706 us/op`, final `4 verts / 2 tris`
  - 4096 piecewise-planar: `352.794 ms`, `86.131 us/op`, final `3350 verts / 6694 tris`

## Commit: Coplanar No-Op Fast Path

### What was added

- Added early-return path in `insert_vertex` for strictly interior coplanar inserts.
- If point is already on the local hit-triangle plane and would optimize away, the system now skips:
  - vertex allocation
  - triangle split
  - retriangulation
  - collapse checks
- Added local-triangle return ID fallback to keep API contract intact.

### Why it matters

- Removes unnecessary topology work for no-op edits.
- Greatly reduces overhead in flat/coplanar workloads and dense coplanar stamping.
- Keeps non-coplanar edit behavior unchanged.

### Validation added

- Re-ran release perf tests:
  - Coplanar bulk: `1600` iters, `1.305 ms`, `0.816 us/op`, final `4 verts / 2 tris`
  - 4096 single-plane coplanar: `4.344 ms`, `1.061 us/op`, final `4 verts / 2 tris`
  - Circle brush bulk: `400` brushes (`2400` generated points), `70.991 ms`, `177.477 us/brush`, final `833 verts / 1660 tris`
  - Non-coplanar bulk: `1600` iters, `78.644 ms`, `49.153 us/op`, final `1604 verts / 3202 tris`
  - 4096 piecewise-planar: `341.240 ms`, `83.311 us/op`, final `3350 verts / 6694 tris`

## Commit: Batch Region Inserts

### What was added

- Added `insert_vertices_batch(points, mode)` API.
- Added `BatchInsertMode`:
  - `Default` (full behavior, locality-ordered)
  - `AssumeNonCoplanar` (skips coplanar-collapse path for known non-coplanar workloads)
- Added `BatchInsertSummary` for tracking inserted/removed/skipped counts.
- Added locality ordering with Morton code for default batch mode.
- Added dedicated batch perf tests for non-coplanar bulk and 4096 piecewise-planar workloads.

### Why it matters

- Reduces per-point overhead in large edit sequences.
- Improves cache locality for default workloads.
- Allows explicit fast path when the caller knows points are non-coplanar.

### Validation added

- Re-ran release perf tests with single vs batch:
  - Non-coplanar single: `70.870 us/op`
  - Non-coplanar batch (`AssumeNonCoplanar`): `53.656 us/op`
  - 4096 piecewise-planar single: `90.763 us/op`
  - 4096 piecewise-planar batch (`Default`): `70.285 us/op`

## Commit: Perf Harness (P95)

### What was added

- Updated perf tests to run multiple samples (`9` measured runs + `1` warmup).
- Added summary output metrics for each perf case:
  - `min`
  - `p50`
  - `p95`
  - `mean`
- Metrics are printed both for total test time and per-unit latency.

### Why it matters

- Single-run perf numbers were noisy and hard to trust.
- P95 shows tail latency behavior, which is critical for editor responsiveness.
- Median + p95 makes regression tracking much more reliable.

### Validation added

- Re-ran release perf suite with the new harness.
- Example p95 per-unit latencies:
  - `insert_vertex` coplanar bulk: `0.710 us/op`
  - `insert_vertex` non-coplanar bulk: `78.884 us/op`
  - `insert_vertex` non-coplanar bulk (batch): `79.813 us/op`
  - `insert_brush` circle bulk: `263.675 us/brush`
  - `4096` piecewise-planar single: `119.798 us/op`
  - `4096` piecewise-planar batch: `90.849 us/op`

## Commit: Multi-Chunk Terrain Editing

### What was added

- Added `TerrainData` as a terrain-level owner of chunk collection with a centered, dynamically growing 2D grid.
- Added signed-coordinate to array-index mapping via origin offsets.
- Added automatic grid growth and data migration when new chunk coordinates fall outside current bounds.
- Added world-space edit entry points on terrain instance:
  - `insert_brush_world(...)`
  - `insert_vertex_world(...)`
- Added overlapped-chunk dispatch so one brush can edit multiple chunks in one operation.
- Added border seam synchronization for touched neighbor chunk pairs.
- Added border vertex reconciliation:
  - merge seam points from both sides
  - ensure both chunks contain seam points
  - align seam vertex heights to avoid cracks

### Why it matters

- Chunk storage is spatially local and aligns with centered terrain coordinates (`(0,0)` center chunk, rings around it).
- Cross-chunk brushes can modify both chunks and maintain seam consistency.
- Keeps chunks logically separate while enforcing matching shared-edge geometry.

### Validation added

- Added `terrain.rs` tests:
  - brush spanning boundary touches both chunks
  - brush 3m from boundary with radius >3m spans both chunks
  - seam vertices align after cross-chunk edit

## Commit: Brush Ops + Cross-Chunk Op Tests

### What was added

- Added `BrushOp` workflow:
  - `SetHeight { y, feature_offset }`
  - `Add { delta }`
  - `Remove { delta }`
  - `Smooth { strength }`
  - `Decimate { basis }`
- Added chunk-level `apply_brush_op(...)`.
- Added terrain-level `apply_brush_op_world(...)`.
- `SetHeight` now uses structural insertion for top+base feature points.

### Why it matters

- Brush behavior is now operation-driven instead of only insert-driven.
- `SetHeight` can build platform-like geometry (including negative height).
- Same brush-op API works across chunk boundaries with seam sync.

### Validation added

- Added `brush_ops.rs` tests for set/add/remove/smooth/decimate behavior.
- Added cross-chunk brush-op seam tests in `terrain.rs`:
  - set-height spanning seam
  - add/remove spanning seam
  - decimate spanning seam

## Commit: Terrain Runtime Store + IDs

### What was added

- Replaced `TerrainInstance3D` mesh/material references with a dedicated `TerrainID`.
- Added runtime `TerrainStore` with generational IDs and slot reuse.
- Runtime now ensures terrain instances always have backing `TerrainData` before draw.
- Added default terrain allocation path (`64m` chunk with ensured `(0,0)` chunk).
- Added cleanup hooks so terrain data is removed on node deletion and cleared on scene reset.
- Added `terrain_store` unit tests for slot reuse and ID invalidation.
- Removed terrain-specific mesh/material fallback extraction in scene loader.

### Why it matters

- Terrain is now treated as first-class terrain data instead of piggybacking on mesh/material IDs.
- Generational IDs prevent stale-handle bugs after remove/reuse cycles.
- Auto-ensure before render prevents missing-data failures for terrain instances.
- Cleanup paths prevent leaked terrain allocations across runtime/scene lifecycle.
- Scene loading now keeps terrain flow separate from generic mesh ingestion assumptions.

### Validation added

- `terrain_store_reuses_slot_with_bumped_generation` verifies reused index + new generation.
- `terrain_store_clear_invalidates_existing_ids` verifies old IDs become invalid after clear.

## Commit: Terrain Node Debug Vertices + Edges

### What was added

- Added per-node terrain debug flags:
  - `show_debug_vertices`
  - `show_debug_edges`
- Added runtime debug draw emission for terrain geometry:
  - vertex markers (small cubes)
  - edge markers (thin cylinders)
- Added render bridge/debug command support for 3D point+line debug draws.
- Added scene loader parsing for terrain debug flags (runtime + static scene paths).

### Why it matters

- Terrain visualization can now be toggled per terrain node without global debug mode.
- You can inspect triangulation quality directly in-scene (vertex density + edge flow).
- Helps catch seam, over-tessellation, and topology artifacts quickly during terrain work.
- Offset explanation:
  - chunk vertices start in chunk-local space (centered around each chunk center),
  - then convert to terrain/world space using chunk coordinate + chunk size,
  - then apply the terrain node transform matrix.
  - This layered offset keeps debug geometry exactly aligned with rendered terrain, even when the terrain node is moved/rotated/scaled.

### Validation added

- Added runtime test `terrain_instance_debug_flags_emit_vertex_and_edge_commands`.
- Full runtime + graphics test suites pass after adding debug draw command paths.

## Commit: Terrain Debug Overlay Offset Fix

### What was added

- Corrected terrain debug world-position conversion for chunk centers.
- Removed the half-chunk center bias (`+ chunk_size * 0.5`) in debug overlay placement.

### Why it matters

- The terrain system treats chunk `(0,0)` as centered at world `(0,0)`.
- Debug vertices/edges were effectively treating `(0,0)` like a corner-origin path, causing a visible half-chunk offset (`32m` on `64m` chunks).
- Overlay geometry now lines up with the actual rendered terrain surface and edit locations.

### Validation added

- Re-ran `terrain_instance_debug_flags_emit_vertex_and_edge_commands` after the offset correction.
- Manual expectation: debug overlay aligns with terrain at origin with no +32 shift.

## Commit: Runtime Default Terrain SetHeight Bootstrap

### What was added

- Updated runtime default terrain creation to apply an immediate brush op on chunk `(0,0)`:
  - `BrushShape::Square`
  - center `(0,0)` in chunk-local space
  - size `10m`
  - `BrushOp::SetHeight { y: 5.0, feature_offset: 0.1 }`

### Why it matters

- New terrain instances now start with visible non-flat topology for inspection.
- Makes it easy to verify final triangulation behavior in live runtime without needing a test harness first.
- Confirms the set-height structural path is exercised in real scene startup flow.

### Validation added

- Runtime tests pass after bootstrap change (`cargo test -p perro_runtime`).
- Existing debug test path still verifies set-height feature behavior and debug draw output.

## Commit: SetHeight Topology Staging + Structural Reconcile

### What was added

- Changed square `SetHeight` construction order and staging:
  - phase 1: insert widened base ring
  - phase 2: insert raised top ring
- Kept outward base offset behavior so the base remains the larger footprint.
- Changed structural insertion reconcile path to a non-aggressive structural reconcile:
  - keeps manifold/valid cleanup
  - skips global coplanar collapse during structural feature construction
- Added constrained structural staging details:
  - intermediate retessellation of the inner base polygon region
  - top-phase inserts constrained to that base region (prevents top connections through outer ground region)
- Added planar shortest-edge edge-flip optimization during structural reconcile:
  - flips crossing/long coplanar shared diagonals when a shorter valid manifold diagonal exists

### Why it matters

- Staging the base first gives retriangulation a stable local footprint before vertical/top feature points are introduced.
- This better matches expected connectivity:
  - outer terrain should anchor into nearest base corners first
  - then top points connect inward from that base
  - avoids long fan connections from distant chunk corners into top points
- Prevents intermediate feature topology from being simplified away before the second construction phase runs.
- Makes brush-op triangulation behavior more predictable for nested/stacked set-height operations.
- Reduces illegal-looking long fan connections by preferring shorter coplanar manifold diagonals.
- Preserves the "simplest valid topology" direction and moves behavior toward the intended pattern:
  - triangulate at each structural phase
  - simplify only when safe after structure is established

### Validation added

- `set_height_square_builds_top_and_base_points` passes with:
  - 8 structural inserts
  - no coplanar-collapse removals
  - outward base radius larger than top radius
- Full `perro_terrain` test suite passes.
- Runtime terrain debug test passes:
  - `terrain_instance_debug_flags_emit_vertex_and_edge_commands`

## Commit: Fixing Inner Plane

### What was added

- Implemented centered square feature sampling for `SetHeight` (centered `-size/2..+size/2` around brush center for feature construction).
- Added explicit top-cap enforcement after structural insertion:
  - detect the 4 top-ring vertices
  - remove any existing all-top cap triangles
  - rebuild cap as exactly 2 triangles (quad cap)
- Added upward orientation fix for rebuilt cap triangles.

### Why it matters

- Prevents inner-cap collapse into asymmetric triangular-prism-like artifacts.
- Keeps set-height output aligned with the intended minimal, stable topology:
  - 8 feature vertices
  - 10 feature triangles (4 walls + top cap), no bottom cap
- Improves predictability for follow-up brush operations inside existing raised regions.

### Validation added

- `set_height_square_builds_top_and_base_points` now verifies:
  - 4 top-cap vertices exist
  - top cap is exactly 2 triangles
- Runtime debug topology test remains passing:
  - `terrain_instance_debug_flags_emit_vertex_and_edge_commands`

## Commit: Proper Terrain Rendering - but it is flipped lol

### What was added

- Replaced terrain draw fallback (`DrawTerrain`/builtin terrain mesh) with per-chunk runtime mesh submission.
- Added runtime chunk mesh build path:
  - converts each `TerrainChunk` into explicit vertex/index buffers
  - computes per-vertex normals from triangle faces
  - uploads via `ResourceCommand::CreateRuntimeMesh`
- Added runtime chunk mesh cache state keyed by `(terrain node, chunk coord)` with geometry hashing.
- Added terrain material bootstrap for runtime chunk draws.
- Added graphics backend/resource support for runtime-provided mesh payloads.

### Why it matters

- Terrain data remains owned by `TerrainData`/`TerrainChunk` and stays separate from renderer resource state.
- Render path now uses real chunk geometry, so each chunk is rendered as its own mesh unit (submesh-style chunk granularity).
- Changed chunks can be re-uploaded independently without rebuilding unrelated chunks.
- Removes the hardcoded terrain mesh dependency and aligns rendering with runtime terrain edits.

### Validation added

- Updated runtime 3D terrain test to assert `CreateRuntimeMesh` is emitted for terrain chunks.
- Full suites pass:
  - `cargo test -p perro_graphics -p perro_runtime --tests`

## Commit: Fixed Flipped Normals

### What was added

- Flipped Normals because they were inverted

### Why it matters

- Proper visuals

### Validation added

- None

## Added Secondary Brush Op

### What was added

- Second SetHeight inside of another

### Why it matters

- Prove that it properly connects

## Inset Ops

### What was added

- Showing multi op inset - secondary feature for vid

### Why it matters

- Looks cool

## Commit: Runtime Terrain Editor Suite + Resource Terrain API

### What was added

- Added `Terrain` sub-API to `ResourceContext`:
  - `res.Terrain().brush_op(terrain_id, center_world, brush_size_meters, shape, op)`
  - `res.Terrain().raycast(terrain_id, origin_world, direction_world, max_distance)`
- Added terrain raycast support in terrain core (`TerrainData::raycast_world`) and new hit payload (`TerrainRayHit`).
- Refactored runtime terrain ownership to shared terrain storage between runtime + resource API so script-issued brush ops and render path use the same terrain data.
- Added mouse absolute position + viewport size to input API:
  - `ipt.Mouse().position()`
  - `ipt.Mouse().viewport_size()`
- Wired window runner/app/runtime to feed cursor position + viewport size every frame.
- Built a full runtime terrain editor directly into `playground/ThreeDTest/res/scripts/camera.rs`:
  - raycasts mouse-to-terrain each frame
  - creates/maintains a stable preview `ParticleEmitter3D`
  - moves preview emitter to the terrain hit point
  - pushes brush params into emitter `params` (`[size, basis]`)
  - applies brush ops while mouse is held

### Editor Controls (Exact)

- `MMB + mouse drag`: camera look
- `W/A/S/D`: planar movement
- `Space`: move up
- `Shift`: move down
- `Mouse wheel` (no modifier): camera move speed
- `S + mouse wheel`: brush size
- `B + mouse wheel`: decimate basis
- `LMB (hold)`: apply active brush op at raycast hit point

Brush operation mode:

- `1`: `Add`
- `2`: `Remove`
- `3`: `Smooth`
- `4`: `Decimate`
- `5`: `SetHeight`

Brush shape:

- `6`: `Square`
- `7`: `Circle`
- `8`: `Triangle`

Preview:

- Auto-created emitter tag: `terrain_editor_preview`
- Profile path: `res://particles/test.ppart`
- Emitter follows current raycast hit point
- Emitter params are updated to `[brush_size, basis]`

### Why it matters

- Keeps `ResourceContext` immutable from script call sites while still supporting terrain edits through command-style APIs.
- Establishes terrain as a first-class editable resource surface with `TerrainID`-driven operations.
- Delivers immediate in-runtime terrain authoring workflow in `ThreeDTest` with visual hit/brush feedback.
- Sets up clean path to later unify terrain raycast into broader world/physics raycast APIs.

### Validation added

- `cargo check` passes for full workspace.
- `cargo check -p perro_runtime -p perro_resource_context -p perro_input -p perro_terrain` passes.

## Commit: Editor Sucks Less

### What was added

- Raycast offset (to make the point of the "oops" vs "thats better btu still scuks")

## Future Commit Template

## Commit: <name>

### What was added

- <bullet>

### Why it matters

- <bullet>

### Validation added

- <test name + expected behavior>
