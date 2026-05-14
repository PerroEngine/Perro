# MultiMesh Demo

Scene:

- `res://scenes/demos/multimesh.scn`

Shows:

- `MultiMeshInstance3D`
- repeated cube/sphere batches
- per-instance positions
- per-instance rotations
- meshlet option on batches

Why scene works this way:

- One node draws many instances with same mesh/material.
- Instance arrays make placement data visible in scene file.
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
