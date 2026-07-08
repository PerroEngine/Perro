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

How blending works (screen-space seam pass):

- Blend sources render fully opaque. A mask pass tags every blend
  participant with an id, then a fullscreen seam pass finds pixels near a
  boundary between two different ids (depth-continuous, so silhouettes
  against far geometry are untouched) and cross-samples the scene color
  from the other side. Both meshes melt into each other along the visible
  contact line; nothing ghosts through geometry.
- The tap pattern is rotated per pixel, so the transition reads as fine
  organic grain instead of banding.
- Noise is anchored to the surface in world space, so the pattern stays put
  when the camera moves. `noise_scale` is the world-space noise tile size
  times 20 (scale 10 = 0.5 world units per tile).
- Blend width (`blend_distance`, world units) is distance-compensated in
  screen space and capped at ~20 px so it stays a seam, not a smear.
- MSAA renders resolve to the single-sample scene target before the seam pass,
  so sample count does not change the feature class.
- `MultiMeshInstance3D` batches write stable participant ids into the same
  mask path as `MeshInstance3D`. The legacy one-sided depth fade remains a
  compatibility fallback when the screen seam path is disabled.

Why scene works this way:

- Grid has four rows and five columns, plus a rock showcase row
  (`rock_a/b/c.glb` on `ground_slab.glb`).
- Top row disables blending, so baseline overlap stays hard.
- Columns use cube/sphere, pyramid/cube, prism/cone, cylinder/capsule, and cube/pyramid pairs.
- Every blend-enabled node uses the same tuning: `blend_distance = 0.6`,
  `blend_min_distance = 0.03`, default noise (0.35 @ scale 14), so all pairs
  and the rocks show one consistent look.
- Target shapes tag receiver layer `1` but do not fade.
- Inserted shapes use `blend_mask = none`, so they can fade against any explicit receiver layer.
- Runtime uses source blend tuning for the contact. `MultiMeshInstance3D`
  sources use the same tuning and mask rules.
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
| 5 | `RockSeparate*` / `RockHard*` / `RockBlend*` | Generated rock glbs: floating, sunk with blending off, sunk with blending on. |

Comparison captures live in `docs/images/` (`rocks_blend_off.png` vs
`rocks_blend_on.png`).

Controls:

| Input | Action |
| ----- | ------ |
| Mouse | Look |
| `W` `A` `S` `D` | Move |
| `Space` / `Shift` | Up / down |
| `Esc` | Pause |
