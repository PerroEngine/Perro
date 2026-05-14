# Mesh Blending Demo

Scene:

- `res://scenes/demos/mesh_blending.scn`

Shows:

- `blend_enabled`
- `blend_layers`
- `blend_mask`
- `blend_distance`
- `noise`
- `noise_scale`
- multiple mesh shapes blending into same view
- side-by-side masked vs blended mesh pairs

Why scene works this way:

- Left pair uses `blend_mask = all`, so cube/sphere/ground intersections stay hard.
- Right pair uses `blend_mask = none`, so same layout shows proper intersection fade.
- Cube corner intersects sphere enough to show blend falloff without visual clutter.
- Each object uses different layer to show independent blend groups.
- Planes use same blend setup as their objects, so object-ground transitions compare clearly.
- Inline materials color-code each test mesh.
- Moderate blend distance and light noise make material fade easier to see.

Scene map:

| Node                   | Role                              |
| ---------------------- | --------------------------------- |
| `MaskedPlane`          | Left fully masked base surface.   |
| `MaskedSphere`         | Left fully masked red base shape. |
| `MaskedCube`           | Left fully masked blue insert.    |
| `BlendPlane`           | Right blend-enabled base surface. |
| `BlendSphere`          | Right red base shape.             |
| `BlendCube`            | Right blue corner insert.         |
| `Ambient` / `KeyLight` | Stable lighting.                  |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
