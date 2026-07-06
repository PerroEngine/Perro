# Decals Demo

Scene:

- `res://scenes/demos/decals.scn`

Script:

- `res://scripts/decals_demo.rs`

Shows:

- `Decal3D` projected onto lit floor, wall, ramp, and pillar
- albedo decals (arrow, paint splat, hazard ring)
- normal-map decal (rivets) that catches the key light
- emissive decal (glowing rune) added on top of lit surfaces
- `sort_priority` overlap order (arrow paints over splat)
- `normal_fade` grazing rejection and `distance_fade_*` on the wall hazard
- scripted slide, spin, and opacity pulse

Why scene works this way:

- Decals project along local -Z, so floor decals pitch `rotation_deg = (-90, 0, 0)` to aim straight down; wall decals keep default orientation to aim into the wall.
- One neutral gray floor and wall make projected color obvious.
- The key `RayLight3D` grazes the wall so the rivet normal decal shows raised bumps.
- The ramp shows a decal wrapping across an angled surface.
- Decals patch albedo/normal/emission before lighting, so they receive the same shadows and lights as the surface under them.

Scene map:

| Node          | Role                                                        |
| ------------- | ----------------------------------------------------------- |
| `DemoCamera`  | Shared freecam.                                             |
| `Ambient`     | Base fill so decals read in shadow.                         |
| `KeyRay`      | Grazing key light; makes the rivet normals pop.             |
| `FillRay`     | Cool back fill.                                             |
| `Floor`       | Gray slab catching the down-projected decals.               |
| `BackWall`    | Catches the rivet and hazard wall decals.                   |
| `Ramp`        | Tilted slab showing projection onto angled geometry.        |
| `Pillar`      | Rounded catcher near the wall.                              |
| `PulseSplat`  | Crimson splat; script pulses its `modulate` alpha.          |
| `FloorArrow`  | Amber arrow, high `sort_priority`, paints over the splat.   |
| `FloorHazard` | Hazard ring projected onto the floor.                       |
| `FloorGlyph`  | Emissive rune, no albedo, glows on the floor.               |
| `RampSplat`   | Splat wrapping over the ramp edge (`normal_fade = 0`).      |
| `FloorLogo`   | Project icon (`res://perro.svg`) stamped on the floor.      |
| `WallRivets`  | Normal-only decal; perturbs wall normals under the key ray. |
| `WallHazard`  | Wall hazard with `distance_fade_begin/length`.              |
| `SliderRune`  | Emissive rune sliding across the floor (moving projector).  |
| `SpinnerHazard` | Hazard ring spun about its projection axis by the script. |

Key fields:

- `size` — box extents; z is the projection depth. Make it deep enough to span the surface.
- `albedo_texture` / `normal_texture` / `emission_texture` — any slot may be empty.
- `modulate` — tint; alpha scales overall opacity.
- `albedo_mix` — 0..1 blend of decal albedo over the surface.
- `normal_strength` — scales the normal-map perturbation.
- `normal_fade` — 0..1 rejection of surfaces facing away from the projection axis.
- `distance_fade_begin` / `distance_fade_length` — camera-distance fade; begin `0` disables.
- `sort_priority` — higher draws over lower where decals overlap.

Notes:

- Unlit materials ignore decals; standard, toon, and multimesh surfaces receive them.
- Decal textures live in `res://textures/decal_*.png` (RGBA; alpha masks the shape).
