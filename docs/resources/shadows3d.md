# 3D Shadows

## Page Map

- [Purpose](#purpose)
- [Use Cases](#use-cases)
- [Fields](#fields)
- [Example](#example)
- [Cost](#cost)
- [Limits](#limits)

## Purpose

3D shadows use `RayLight3D`, `PointLight3D`, and `SpotLight3D`.

Meshes cast with `cast_shadows = true`.

Meshes receive with `receive_shadows = true`.

Lights cast with `cast_shadows = true`.

## Use Cases

- outdoor sun shadows from `RayLight3D`
- local lamp shadows from `PointLight3D`
- cone shadows from `SpotLight3D`
- bias tuning for acne or detached shadows

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
