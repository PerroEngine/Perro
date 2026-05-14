# Mesh Blending Demo

Scene:

- `res://scenes/demos/mesh_blending.scn`

Shows:

- `blend_enabled`
- `blend_layers`
- `blend_mask`
- `blend_distance`
- multiple mesh shapes blending into same view

Why scene works this way:

- All blend objects sit close together so blend falloff is visible.
- Each object uses different layer to show independent blend groups.
- Plane uses blend too, so object-ground transitions are visible.
- Inline materials color-code each test mesh.

Scene map:

| Node                   | Role                        |
| ---------------------- | --------------------------- |
| `BlendPlane`           | Blend-enabled base surface. |
| `BlendSphereA`         | Red blend source.           |
| `BlendCube`            | Blue blend source.          |
| `BlendSphereB`         | Green blend source.         |
| `Ambient` / `KeyLight` | Stable lighting.            |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
