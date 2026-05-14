# MultiMesh Demo

Scene:

- `res://scenes/demos/multimesh.scn`

Shows:

- `MultiMeshInstance3D`
- 1,000 cube instances
- stacked height layers
- generated instance grids
- per-instance rotations from grid steps
- meshlet option on batches

Why scene works this way:

- One node draws many instances with same mesh/material.
- `instance_grid` keeps large placement sets compact.
- Cube batch uses `20 * 5 * 10 = 1,000` instances.
- Cube and sphere batches show two separate draw groups.
- Large camera speed fits wider grid layout.

Scene map:

| Node              | Role                          |
| ----------------- | ----------------------------- |
| `CubeBatch`       | Batched cubes with rotations. |
| `SphereBatch`     | Batched spheres.              |
| `GridFloor`       | Scale reference.              |
| `Sun` / `Ambient` | Shared lighting.              |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
