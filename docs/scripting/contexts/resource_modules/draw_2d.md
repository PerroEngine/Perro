# Draw2D Module

Access:

- `res.Draw2D()`

Purpose:

- Submit transient 2D shapes for the current frame.
- Useful for gameplay overlays (reticles, indicators, debug markers) without retained UI nodes.

Macros:

- `draw!(res, shape, position)`

Methods:

- `res.Draw2D().push(shape, position)`
- `res.Draw2D().circle(center, radius, color)`
- `res.Draw2D().ring(center, radius, color, thickness)`
- `res.Draw2D().rect(center, size, color)`
- `res.Draw2D().rect_stroke(center, size, color, thickness)`
- `res.Draw2D().line(start, end, color, thickness)`
- `res.Draw2D().polyline(points, color, thickness)`
- `res.Draw2D().polygon(points, color, thickness)`
- `res.Draw2D().path(points, color, thickness)`
- `res.Draw2D().sprite(center, texture, size, tint)`
- `res.Draw2D().sprite_path(center, source, size, tint)`

Types:

- `DrawShape2D::Circle { radius, color, filled, thickness }`
- `DrawShape2D::Rect { size, color, filled, thickness }`
- `DrawShape2D::Line { end, color, thickness }`
- `DrawShape2D::Polyline { points, color, thickness, closed }`
- `DrawShape2D::Path { points, color, thickness }`
- `DrawShape2D::Sprite { texture, size, tint, texture_region }`

Behavior:

- Draw commands are one-frame only.
- You must submit them every frame you want them visible.
- Position is normalized screen-space (`0.0..1.0`).
- `Vector2::new(0.5, 0.5)` is the screen center.
- `x=0.0` is left, `x=1.0` is right.
- `y=0.0` is bottom, `y=1.0` is top.
- Shape size fields (`radius`, `size`, `thickness`) are still in Draw2D size units.

Examples:

```rust
use perro_structs::{DrawShape2D, Vector2};

let center = Vector2::new(0.5, 0.5);

draw!(res, DrawShape2D::circle(16.0, [1.0, 1.0, 1.0, 1.0]), center);
draw!(res, DrawShape2D::ring(24.0, [1.0, 0.2, 0.2, 1.0], 2.0), center);
res.Draw2D().line(
    Vector2::new(0.1, 0.1),
    Vector2::new(0.9, 0.1),
    [0.2, 0.8, 1.0, 1.0],
    3.0,
);

draw!(
    res,
    DrawShape2D::Rect {
        size: Vector2::new(120.0, 36.0),
        color: [0.0, 0.0, 0.0, 0.5],
        filled: true,
        thickness: 1.0,
    },
    Vector2::new(0.12, 0.08)
);
```
