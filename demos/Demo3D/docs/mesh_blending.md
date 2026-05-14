# Mesh Blending Demo

Scene:

- `res://scenes/demos/mesh_blending.scn`

Shows:

- `blend_enabled`
- `blend_layers`
- `blend_mask`
- `blend_normals`
- `blend_distance`
- `blend_min_distance`
- `noise`
- `noise_scale`
- screen-space contact fade tuning
- same-material and different-material seams

Why scene works this way:

- Grid has four rows and five columns.
- Top row disables blending, so baseline overlap stays hard.
- Columns use cube/sphere, pyramid/cube, prism/cone, cylinder/capsule, and cube/pyramid pairs.
- Same-material row sweeps blend distance from tight to wide.
- Different-material row uses normal assist with same distance sweep.
- Noise row keeps distance fixed and sweeps noise strength.
- Distance sweep values are `0.75`, `1.10`, `1.55`, `2.10`, `2.80`.
- Noise row uses distance `2.80` and noise scale `18.0`.
- Target shapes tag receiver layer `1` but do not fade.
- Inserted shapes use `blend_mask = none`, so they can fade against any explicit receiver layer.
- Runtime uses source blend tuning for the contact.
- `blend_enabled` enables screen fade.
- `blend_normals` enables normal assist where seam smoothing needs it.
- Fade is depth-gated so only close contact seams fade.

Scene map:

| Row | Nodes | Test |
| --- | ----- | ---- |
| 1 | `OffTarget*` / `OffInsert*` | No blending. |
| 2 | `SameTarget*` / `SameInsert*` | Same material, blend distance sweep. |
| 3 | `MaterialTarget*` / `MaterialInsert*` | Different materials, distance sweep, normal assist. |
| 4 | `NoiseTarget*` / `NoiseInsert*` | Fixed distance, noise strength sweep, normal assist. |

Controls:

| Input | Action |
| ----- | ------ |
| Mouse | Look |
| `W` `A` `S` `D` | Move |
| `Space` / `Shift` | Up / down |
| `Esc` | Pause |
