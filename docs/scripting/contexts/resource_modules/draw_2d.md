# Draw 2D Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `push` | [`push`](#push) |
| `circle` | [`circle`](#circle) |
| `ring` | [`ring`](#ring) |
| `rect` | [`rect`](#rect) |
| `rect_stroke` | [`rect_stroke`](#rect_stroke) |
| `line` | [`line`](#line) |
| `polyline` | [`polyline`](#polyline) |
| `polygon` | [`polygon`](#polygon) |
| `path` | [`path`](#path) |
| `sprite` | [`sprite`](#sprite) |
| `atlas_sprite` | [`atlas_sprite`](#atlas_sprite) |
| `sprite_path` | [`sprite_path`](#sprite_path) |
| `draw` | [`draw`](#draw) |

## Purpose

`ctx.res.Draw2D()` queues immediate-mode 2D shapes and sprites for the current frame without creating scene nodes. Each call adds one shape at a position; nothing persists, so you re-issue the draws every frame. It is the fast path for debug overlays, HUD gizmos, and simple procedural 2D visuals that would be wasteful to build as nodes.

## Use Cases

- Debug overlays: draw a hitbox with `rect_stroke`, an aggro radius with `ring`, and a velocity vector with `line` while tuning gameplay.
- Targeting and aiming: draw an aim reticle with `circle` or a trajectory preview with `polyline`.
- Minimap markers: stamp enemy and objective dots with `circle`, or icons with `sprite`.
- Selection and pathing: outline a selected unit with `rect_stroke` and its move path with `path`.
- Procedural HUD bars and shapes: draw health/stamina fills with `rect` in screen space.
- Atlas icons: draw one sprite from a packed sheet with `atlas_sprite`, giving a UV sub-rectangle.

## Ownership And Choice

`Draw2D` owns commands for the current frame only. Use it for transient debug, procedural, and overlay geometry that has no independent identity. Use scene or UI nodes when an item needs input, layout, animation, a script, or persistence. Keep source state elsewhere and re-submit only the shapes that should appear this frame.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Draw2D()`
- Immediate mode: draws last one frame; call them each frame (typically in `on_update`).
- Types: `perro_structs::{Vector2, DrawShape2D}`; colors are `[f32; 4]` RGBA.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

Draw a debug ring and a health bar every frame.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        self.debug_draw(ctx, Vector2::new(200.0, 200.0), 0.75);
    }
});

methods!({
    fn debug_draw(&self, ctx: &mut ScriptContext<'_, API>, at: Vector2, health: f32) {
        let red = [1.0, 0.2, 0.2, 1.0];
        // Aggro radius.
        ctx.res.Draw2D().ring(at, 64.0, red, 2.0);
        // Health bar fill above it.
        let bar = Vector2::new(80.0 * health, 8.0);
        ctx.res.Draw2D().rect(Vector2::new(at.x, at.y - 48.0), bar, [0.2, 1.0, 0.3, 1.0]);
    }
});
```

## API Reference

All draw calls return `()` and queue one shape for the current frame.

### `push`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn push(&self, shape: DrawShape2D, position: Vector2)` |
| Params | `shape: DrawShape2D, position: Vector2` |
| Returns | `()` |
| Use when | Queuing a pre-built `DrawShape2D`; the helpers below wrap this for common shapes. |

### `circle`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn circle(&self, center: Vector2, radius: f32, color: [f32; 4])` |
| Params | `center: Vector2, radius: f32, color: [f32; 4]` |
| Returns | `()` |
| Use when | Drawing a filled dot or marker. |

### `ring`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn ring(&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32)` |
| Params | `center: Vector2, radius: f32, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Drawing a hollow circle outline, for example an aggro or blast radius. |

### `rect`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn rect(&self, center: Vector2, size: Vector2, color: [f32; 4])` |
| Params | `center: Vector2, size: Vector2, color: [f32; 4]` |
| Returns | `()` |
| Use when | Drawing a filled box, for example a HUD bar fill. |

### `rect_stroke`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn rect_stroke(&self, center: Vector2, size: Vector2, color: [f32; 4], thickness: f32)` |
| Params | `center: Vector2, size: Vector2, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Outlining a box, for example a hitbox or selection frame. |

### `line`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn line(&self, start: Vector2, end: Vector2, color: [f32; 4], thickness: f32)` |
| Params | `start: Vector2, end: Vector2, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Drawing a single segment, for example a velocity or aim vector. |

### `polyline`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn polyline(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Drawing a connected open line strip, for example a trajectory preview. |

### `polygon`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn polygon(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Drawing a closed outlined shape. |

### `path`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn path(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Drawing a movement or route path through several points. |

### `sprite`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn sprite(&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4])` |
| Params | `center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4]` |
| Returns | `()` |
| Use when | Drawing a whole texture as a 2D image, for example a minimap icon. |

### `atlas_sprite`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn atlas_sprite(&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4], texture_region: [f32; 4])` |
| Params | `center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4], texture_region: [f32; 4]` |
| Returns | `()` |
| Use when | Drawing one cell of a packed sheet; `texture_region` is the UV sub-rectangle. |

### `sprite_path`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn sprite_path(&self, center: Vector2, source: &str, size: Vector2, tint: [f32; 4])` |
| Params | `center: Vector2, source: &str, size: Vector2, tint: [f32; 4]` |
| Returns | `()` |
| Use when | Drawing a sprite straight from a texture path; loads the texture for you. |

### `draw`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `draw!(ctx.res, shape, position)` |
| Params | `ctx.res, shape, position` |
| Returns | `()` |
| Use when | Macro form of `push`. |
