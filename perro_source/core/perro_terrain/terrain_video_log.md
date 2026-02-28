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

- Editing now happens at terrain level rather than per isolated chunk.
- Chunk storage is spatially local and aligns with centered terrain coordinates (`(0,0)` center chunk, rings around it).
- Cross-chunk brushes can modify both chunks and maintain seam consistency.
- Keeps chunks logically separate while enforcing matching shared-edge geometry.

### Validation added

- Added `terrain.rs` tests:
  - brush spanning boundary touches both chunks
  - brush 3m from boundary with radius >3m spans both chunks
  - seam vertices align after cross-chunk edit

## Future Commit Template

## Commit: <name>

### What was added

- <bullet>

### Why it matters

- <bullet>

### Validation added

- <test name + expected behavior>
