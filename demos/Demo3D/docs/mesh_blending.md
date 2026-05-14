# Mesh Blending Demo

Scene:

- `res://scenes/demos/mesh_blending.scn`

Shows:

- `blend_enabled`
- `blend_screen`
- `blend_normals`
- `blend_layers`
- `blend_mask`
- `blend_distance`
- `blend_min_distance`
- `noise`
- `noise_scale`
- screen-space contact fade tuning
- same-material and different-material seams

Why scene works this way:

- Grid has four rows and five columns.
- Top row disables blending, so baseline overlap stays hard.
- Same-material row sweeps blend distance from tight to wide.
- Different-material row uses normal assist with same distance sweep.
- Noise row keeps distance fixed and sweeps noise strength.
- Distance sweep values are `0.30`, `0.55`, `0.85`, `1.20`, `1.65`.
- Noise row uses distance `0.95`.
- Cubes and spheres use `blend_mask = none`, so both allow the seam pair.
- The visible fade hits the farther surface in screen depth, so front faces stay solid.
- Both sides define blend distance and noise.
- Runtime averages source and target blend tuning for the contact.

Scene map:

| Row | Nodes | Test |
| --- | ----- | ---- |
| 1 | `OffTarget*` / `OffSphere*` | No blending. |
| 2 | `SameTarget*` / `SameSphere*` | Same material, blend distance sweep. |
| 3 | `MaterialTarget*` / `MaterialSphere*` | Different materials, distance sweep, normal assist. |
| 4 | `NoiseTarget*` / `NoiseSphere*` | Fixed distance, noise strength sweep, normal assist. |

Controls:

| Input | Action |
| ----- | ------ |
| Mouse | Look |
| `W` `A` `S` `D` | Move |
| `Space` / `Shift` | Up / down |
| `Esc` | Pause |
