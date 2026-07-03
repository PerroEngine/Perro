# MultiMesh Demo

Scene:

- `res://scenes/demos/multimesh.scn`

Shows:

- `MultiMeshInstance3D`
- 100 clusters, 1,000 instances each (100,000 total)
- clusters float in a sphere-shell cloud centered on camera spawn
- 7 distinct primitive shapes cycled across clusters
- tall columns per cluster
- generated instance grids
- per-instance rotations from grid steps
- per-instance scale from grid scale fields
- meshlet option on batches

Why scene works this way:

- One node draws many instances with same mesh/material.
- `instance_grid` keeps large placement sets compact.
- `instance_grid.scale` sets per-instance scale.
- `instance_grid.scale_wave` varies per-instance scale.
- Each cluster uses `5 * 20 * 10 = 1,000` instances.
- 100 clusters (`Cluster001`..`Cluster100`) are placed with a
  Fibonacci sphere distribution on two shells (radius 55 / 90) so
  they surround the camera in every direction, not just on the
  ground.
- Tall column shape per cluster (20 layers on the Y axis) instead of
  a flat grid.
- No floor mesh — camera spawns at the center of the cloud.
- Mesh cycles through cube, sphere, cylinder, cone, capsule, sq_pyr,
  tri_prism per cluster for visual variety.

Scene map:

| Node                       | Role                              |
| --------------------------- | ---------------------------------- |
| `Cluster001`..`Cluster100` | Sphere-cloud clusters, 7 shapes.   |
| `Sun` / `Ambient`          | Shared lighting.                   |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
