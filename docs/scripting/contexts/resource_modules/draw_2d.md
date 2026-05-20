# Draw 2D Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
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

## Overview

This resource module belongs to `ctx.res` and documents draw 2d calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Draw2D()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `push`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn push(&self, shape: DrawShape2D, position: Vector2)` |
| Params | `&self, shape: DrawShape2D, position: Vector2` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().push(Default::default(), Vector2::new(0.0, 0.0));
        let _ = value;
    }
});
```

### `circle`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn circle(&self, center: Vector2, radius: f32, color: [f32; 4])` |
| Params | `&self, center: Vector2, radius: f32, color: [f32; 4]` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().circle(Vector2::new(0.0, 0.0), 1.0, [1.0, 1.0, 1.0, 1.0]);
        let _ = value;
    }
});
```

### `ring`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn ring(&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32)` |
| Params | `&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().ring(Vector2::new(0.0, 0.0), 1.0, [1.0, 1.0, 1.0, 1.0], 1.0);
        let _ = value;
    }
});
```

### `rect`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn rect(&self, center: Vector2, size: Vector2, color: [f32; 4])` |
| Params | `&self, center: Vector2, size: Vector2, color: [f32; 4]` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().rect(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0), [1.0, 1.0, 1.0, 1.0]);
        let _ = value;
    }
});
```

### `rect_stroke`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn rect_stroke(&self, center: Vector2, size: Vector2, color: [f32; 4], thickness: f32)` |
| Params | `&self, center: Vector2, size: Vector2, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().rect_stroke(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0), [1.0, 1.0, 1.0, 1.0], 1.0);
        let _ = value;
    }
});
```

### `line`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn line(&self, start: Vector2, end: Vector2, color: [f32; 4], thickness: f32)` |
| Params | `&self, start: Vector2, end: Vector2, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().line(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0), [1.0, 1.0, 1.0, 1.0], 1.0);
        let _ = value;
    }
});
```

### `polyline`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn polyline(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().polyline(vec![Vector2::new(0.0, 0.0)], [1.0, 1.0, 1.0, 1.0], 1.0);
        let _ = value;
    }
});
```

### `polygon`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn polygon(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().polygon(vec![Vector2::new(0.0, 0.0)], [1.0, 1.0, 1.0, 1.0], 1.0);
        let _ = value;
    }
});
```

### `path`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn path(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().path(vec![Vector2::new(0.0, 0.0)], [1.0, 1.0, 1.0, 1.0], 1.0);
        let _ = value;
    }
});
```

### `sprite`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn sprite(&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4])` |
| Params | `&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4]` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().sprite(Vector2::new(0.0, 0.0), Default::default(), Vector2::new(0.0, 0.0), [1.0, 1.0, 1.0, 1.0]);
        let _ = value;
    }
});
```

### `atlas_sprite`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn atlas_sprite( &self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4], texture_region: [f32; 4], )` |
| Params | `&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4], texture_region: [f32; 4],` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().atlas_sprite(Vector2::new(0.0, 0.0), Default::default(), Vector2::new(0.0, 0.0), [1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
        let _ = value;
    }
});
```

### `sprite_path`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn sprite_path(&self, center: Vector2, source: &str, size: Vector2, tint: [f32; 4])` |
| Params | `&self, center: Vector2, source: &str, size: Vector2, tint: [f32; 4]` |
| Returns | `()` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Draw2D().sprite_path(Vector2::new(0.0, 0.0), "name", Vector2::new(0.0, 0.0), [1.0, 1.0, 1.0, 1.0]);
        let _ = value;
    }
});
```

### `draw`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `draw!(ctx.res.res, shape, position)` |
| Params | `ctx.res, shape, position` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = draw!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```
