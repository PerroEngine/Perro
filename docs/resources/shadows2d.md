# 2D Shadows

`RayLight2D`, `PointLight2D`, and `SpotLight2D` cast 2D shadows when
`cast_shadows = true`.

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

## Limits

- Renderer tests at most 128 caster primitives per light fragment.
- Camera streams collect 2D lights but do not collect separate caster sets.
- Sprite alpha does not cast. Adding texture silhouettes needs a separate
  texture-aware caster path and would break current texture batching if folded
  into the light pass.
