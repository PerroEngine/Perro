# 3D Shadows

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Fields | [Fields](#fields) |
| Example | [Example](#example) |
| Cost | [Cost](#cost) |
| Limits | [Limits](#limits) |

## Purpose

3D shadows let lights be occluded by meshes so a scene gains depth and readable form instead of flat lighting. `RayLight3D`, `PointLight3D`, and `SpotLight3D` each cast when `cast_shadows = true`; a mesh casts when its own `cast_shadows = true` and catches shadows when `receive_shadows = true`. Tune `shadow_strength`, `shadow_depth_bias`, and `shadow_normal_bias` to balance shadow darkness against artifacts.

## Use Cases

- Outdoor sun or moon: one `RayLight3D` with `cast_shadows = true` casting cascaded shadows across a broad scene.
- Local lamps and torches: `PointLight3D` shadows (six layers per light, so keep them scarce).
- Flashlights and stage lights: `SpotLight3D` cone shadows (one layer per shadowed light).
- Fixing shadow acne: raise `shadow_depth_bias`, or raise `shadow_normal_bias` for grazing-angle acne.
- Reattaching floating shadows: lower `shadow_depth_bias` / `shadow_normal_bias` when contact edges detach from casters.
- Per-mesh control: toggle `cast_shadows` and `receive_shadows` so, for example, a glowing emitter lights the scene without shadowing itself.

## Cost Choice

Prefer one broad directional shadow plus a small number of important local
shadow lights. Point lights cost six shadow layers; spot lights cost one. Disable
cast/receive on meshes where the result adds no readable contact or depth.

## Fields

`shadow_strength` controls final shadow opacity.

Default: `0.82`.

`shadow_depth_bias` offsets depth compare.

Default: `0.00003`.

Raise it to reduce acne.

Lower it when shadows detach from casters.

`shadow_normal_bias` offsets along receiver normal.

Default: `0.005`.

Raise it to reduce grazing-angle acne.

Lower it when contact edges float.

Aliases:

- `shadow_opacity` => `shadow_strength`
- `shadow_bias` => `shadow_depth_bias`

Nested form:

```text
shadow = { strength = 0.82 depth_bias = 0.00003 normal_bias = 0.005 }
```

## Example

```text
[sun]
[RayLight3D]
    color = (1, 0.96, 0.88)
    intensity = 2.0
    cast_shadows = true
    shadow = { strength = 0.75 depth_bias = 0.00003 normal_bias = 0.005 }
[/RayLight3D]
[/sun]

[crate]
[MeshInstance3D]
    mesh = "res://crate.pmesh"
    surfaces = ["res://crate.pmat"]
    cast_shadows = true
    receive_shadows = true
[/MeshInstance3D]
[/crate]
```

## Cost

`RayLight3D` uses cascades.

`SpotLight3D` uses one shadow layer per shadowed light.

`PointLight3D` uses six layers per shadowed light.

Keep point shadows scarce.

Prefer one ray shadow for broad outdoor scenes.

## Limits

Current renderer stores strength/bias as one frame-wide triplet.

Selection order:

1. first active shadowed ray light
2. first active shadowed spot light
3. first active shadowed point light

Per-light fields still travel through scene/runtime state.

Future renderer work can split tuning per light without scene format churn.
