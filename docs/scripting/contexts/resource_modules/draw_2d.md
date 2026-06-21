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
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `circle`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn circle(&self, center: Vector2, radius: f32, color: [f32; 4])` |
| Params | `&self, center: Vector2, radius: f32, color: [f32; 4]` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `ring`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn ring(&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32)` |
| Params | `&self, center: Vector2, radius: f32, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `rect`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn rect(&self, center: Vector2, size: Vector2, color: [f32; 4])` |
| Params | `&self, center: Vector2, size: Vector2, color: [f32; 4]` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `rect_stroke`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn rect_stroke(&self, center: Vector2, size: Vector2, color: [f32; 4], thickness: f32)` |
| Params | `&self, center: Vector2, size: Vector2, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `line`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn line(&self, start: Vector2, end: Vector2, color: [f32; 4], thickness: f32)` |
| Params | `&self, start: Vector2, end: Vector2, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `polyline`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn polyline(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `polygon`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn polygon(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `path`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn path(&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32)` |
| Params | `&self, points: Vec<Vector2>, color: [f32; 4], thickness: f32` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `sprite`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn sprite(&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4])` |
| Params | `&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4]` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `atlas_sprite`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn atlas_sprite( &self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4], texture_region: [f32; 4], )` |
| Params | `&self, center: Vector2, texture: TextureID, size: Vector2, tint: [f32; 4], texture_region: [f32; 4],` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `sprite_path`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `pub fn sprite_path(&self, center: Vector2, source: &str, size: Vector2, tint: [f32; 4])` |
| Params | `&self, center: Vector2, source: &str, size: Vector2, tint: [f32; 4]` |
| Returns | `()` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `draw`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Draw2D()` |
| Signature | `draw!(ctx.res.res, shape, position)` |
| Params | `ctx.res, shape, position` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

