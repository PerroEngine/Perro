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

## Future Commit Template

## Commit: <name>

### What was added

- <bullet>

### Why it matters

- <bullet>

### Validation added

- <test name + expected behavior>
