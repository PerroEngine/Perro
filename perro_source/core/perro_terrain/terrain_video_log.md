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

### Why it matters

- Cuts unnecessary work in common non-coplanar insert paths.
- Avoids repeated local inserts in brush-heavy edits.
- Preserves topology safety while reducing optimization overhead.

### Validation added

- Re-ran release perf tests after optimization:
  - Coplanar bulk: `1600` iters, `12.924 ms`, `8.077 us/op`, final `4 verts / 2 tris`
  - Non-coplanar bulk: `1600` iters, `161.679 ms`, `101.049 us/op`, final `1604 verts / 3202 tris`
  - Circle brush bulk: `400` brushes (`2400` generated points), `105.448 ms`, `263.620 us/brush`, final `833 verts / 1660 tris`

## Future Commit Template

## Commit: <name>

### What was added

- <bullet>

### Why it matters

- <bullet>

### Validation added

- <test name + expected behavior>
