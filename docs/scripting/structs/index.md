# Struct Types

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| Struct Groups | [Struct Groups](#struct-groups) |
| Use Cases     | [Use Cases](#use-cases)         |
| Examples      | [Examples](#examples)           |

## Purpose

Perro exposes small typed structs to scripts, scene nodes, resources, renderer data, physics, audio, accessibility, and post-processing.

This section replaces the old math-only page because the exported structs are not only math types.

## Struct Groups

| Group           | Page                          | Use when                                                                                                                  |
| --------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 2D structs      | [2D Structs](2d.md)           | Work with 2D positions, sizes, transforms, draw shapes, and 2D-facing API params.                                         |
| 3D structs      | [3D Structs](3d.md)           | Work with 3D positions, rotations, transforms, rays, and 3D-facing API params.                                            |
| Generic structs | [Generic Structs](generic.md) | Work with matrix values, color, masks, audio material data, post-processing, accessibility, constants, IK, and packed normalized values. |

## Use Cases

- Move and place 2D nodes: `Vector2` and `Transform2D`.
- Aim, orbit, and rotate in 3D: `Vector3`, `Quaternion`, and `Transform3D`.
- Address grids and chunks with integer coordinates: `IVector2` / `IVector3` (signed) and `UVector2` / `UVector3` (unsigned).
- Color and tint UI, sprites, and lights: `Color`.
- Filter collisions, rendering, and audio by layer: `BitMask`.
- Pass packed math or shader data: `Matrix<ROWS, COLS, T>` for row-major values, `Matrix2` / `Matrix3` / `Matrix4` for fast glam-backed operations.

Some of these are everyday script-facing values; others are mostly built-in owned data. They are all documented because they can appear in public types, serialized data, node/resource state, return values, or nested structs.

## Examples

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let step = Vector2::new(120.0 * dt, 0.0);

        if let Some(pos) = get_local_pos_2d!(ctx.run, ctx.id) {
            set_local_pos_2d!(ctx.run, ctx.id, pos + step);
        }
    }
});
```

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let tint = Color::from_hex("#3A86FF").unwrap_or(Color::WHITE);
        let _rgba = tint.to_rgba();

        enable_colorblind_filter!(ctx.res, ColorBlindFilter::Protan, 0.75);
    }
});
```
