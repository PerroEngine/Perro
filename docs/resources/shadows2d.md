# 2D Shadows

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Casters | [Casters](#casters) |
| Soft Penumbra | [Soft Penumbra](#soft-penumbra) |
| Example | [Example](#example) |
| Limits | [Limits](#limits) |

## Purpose

2D shadows let lights be blocked by scene geometry so a torch, lamp, or sun creates real dark areas instead of flat lighting. `RayLight2D`, `PointLight2D`, and `SpotLight2D` cast shadows when `cast_shadows = true`, and visible collision shapes (including tilemap tiles) act as the occluders. This is what sells stealth cover, dungeon atmosphere, and top-down line-of-sight lighting in a 2D scene.

## Use Cases

- Dungeon lamp with soft edges: a `PointLight2D` with `cast_shadows = true`, `shadow_softness = 0.55`, and `shadow_samples = 12` for a penumbra.
- Directional sun/moon: a `RayLight2D` casting parallel shadows across the level.
- Focused spotlight or flashlight: a `SpotLight2D` cone that only lights and shadows what it points at.
- Level geometry occluders: `CollisionShape2D` nodes and `TileMap2D` tiles whose `.ptileset` entry uses `collision = true` (with `collision_enabled = true` on the tilemap) block the light.
- Crisp retro look: set `shadow_softness = 0.0` or `shadow_samples = 1` to keep hard-edged shadows.

## Casters

Visible `CollisionShape2D` nodes cast shadows.
`TileMap2D` also casts from tiles whose `.ptileset` entry uses
`collision = true` when `collision_enabled = true` on the tilemap.

Tilemap auto rectangles use the same merged chunks as collision bake.
Explicit rectangle, circle, triangle, and convex polygon collision shapes keep
their silhouettes. Convex polygons split into triangles for shadow tests.

## Soft Penumbra

All three shadow lights use the same controls:

- `shadow_softness`: normalized `0.0..1.0`; default `0.0` keeps the hard path.
- `shadow_samples`: sample count `1..16`; default `8`.

Point and spot lights sample a source disk up to 5% of light range at softness
`1.0`. Ray lights sample up to a 2 degree source angle at softness `1.0`.
More samples smooth the penumbra and cost more fragment work.

```text
[lamp]
[PointLight2D]
    range = 480.0
    cast_shadows = true
    shadow_softness = 0.55
    shadow_samples = 12
[/PointLight2D]
[/lamp]
```

Set `shadow_softness = 0.0` or `shadow_samples = 1` for hard shadows.

## Example

A soft lamp plus a wall that blocks its light:

```text
[lamp]
[PointLight2D]
    range = 480.0
    cast_shadows = true
    shadow_softness = 0.55
    shadow_samples = 12
[/PointLight2D]
[/lamp]

[wall]
[CollisionShape2D]
    shape = { type = quad width = 96.0 height = 16.0 }
[/CollisionShape2D]
[/wall]
```

The `CollisionShape2D` silhouette casts a shadow from the `PointLight2D`. A `TileMap2D`
with `collision_enabled = true` casts from its collidable tiles the same way.

## Limits

- Renderer tests at most 128 caster primitives per light fragment.
- Camera streams collect 2D lights but do not collect separate caster sets.
- Sprite alpha does not cast. Adding texture silhouettes needs a separate
  texture-aware caster path and would break current texture batching if folded
  into the light pass.
