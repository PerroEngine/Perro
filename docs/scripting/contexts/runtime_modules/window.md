# Window Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `set_title` | [`set_title`](#set_title) |
| `set_size` | [`set_size`](#set_size) |
| `set_mode` | [`set_mode`](#set_mode) |
| `set_windowed` | [`set_windowed`](#set_windowed) |
| `set_borderless_fullscreen` | [`set_borderless_fullscreen`](#set_borderless_fullscreen) |
| `set_frame_rate_cap` | [`set_frame_rate_cap`](#set_frame_rate_cap) |
| `set_frame_rate_limit` | [`set_frame_rate_limit`](#set_frame_rate_limit) |
| `set_refresh_rate_cap` | [`set_refresh_rate_cap`](#set_refresh_rate_cap) |
| `set_unlimited_frame_rate` | [`set_unlimited_frame_rate`](#set_unlimited_frame_rate) |
| `get_active_refresh_rate` | [`get_active_refresh_rate`](#get_active_refresh_rate) |
| `window_set_title` | [`window_set_title`](#window_set_title) |
| `window_set_size` | [`window_set_size`](#window_set_size) |
| `window_set_mode` | [`window_set_mode`](#window_set_mode) |
| `window_set_frame_rate_cap` | [`window_set_frame_rate_cap`](#window_set_frame_rate_cap) |
| `window_set_frame_rate_limit` | [`window_set_frame_rate_limit`](#window_set_frame_rate_limit) |
| `window_get_active_refresh_rate` | [`window_get_active_refresh_rate`](#window_get_active_refresh_rate) |

## Overview

This runtime module belongs to `ctx.run` and documents window calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Window()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        window_set_title!(ctx.run, "Perro");
        window_set_frame_rate_limit!(ctx.run, 144.0);
    }
});
```

## API Reference

### `set_title`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_title(&mut self, title: impl Into<String>)` |
| Params | `&mut self, title: impl Into<String>` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_title("name");
        let _ = value;
    }
});
```

### `set_size`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_size(&mut self, width: u32, height: u32)` |
| Params | `&mut self, width: u32, height: u32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_size(0, 0);
        let _ = value;
    }
});
```

### `set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_mode(&mut self, mode: WindowMode)` |
| Params | `&mut self, mode: WindowMode` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_mode(0.1);
        let _ = value;
    }
});
```

### `set_windowed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_windowed(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_windowed();
        let _ = value;
    }
});
```

### `set_borderless_fullscreen`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_borderless_fullscreen(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_borderless_fullscreen();
        let _ = value;
    }
});
```

### `set_frame_rate_cap`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_frame_rate_cap(&mut self, cap: FrameRateCap)` |
| Params | `&mut self, cap: FrameRateCap` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_frame_rate_cap(0.1);
        let _ = value;
    }
});
```

### `set_frame_rate_limit`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_frame_rate_limit(&mut self, fps: f32)` |
| Params | `&mut self, fps: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_frame_rate_limit(1.0);
        let _ = value;
    }
});
```

### `set_refresh_rate_cap`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_refresh_rate_cap(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_refresh_rate_cap();
        let _ = value;
    }
});
```

### `set_unlimited_frame_rate`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_unlimited_frame_rate(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().set_unlimited_frame_rate();
        let _ = value;
    }
});
```

### `get_active_refresh_rate`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn get_active_refresh_rate(&mut self) -> Option<f32>` |
| Params | `&mut self` |
| Returns | `Option<f32>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Window().get_active_refresh_rate();
        let _ = value;
    }
});
```

### `window_set_title`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_title!(ctx.run, title)` |
| Params | `ctx, title` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = window_set_title!(ctx.run, "Perro");
        let _ = value;
    }
});
```

### `window_set_size`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_size!(ctx.run, width, height)` |
| Params | `ctx, width, height` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = window_set_size!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `window_set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_mode!(ctx.run, mode)` |
| Params | `ctx, mode` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = window_set_mode!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `window_set_frame_rate_cap`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_frame_rate_cap!(ctx.run, cap)` |
| Params | `ctx, cap` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = window_set_frame_rate_cap!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `window_set_frame_rate_limit`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_frame_rate_limit!(ctx.run, fps)` |
| Params | `ctx, fps` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = window_set_frame_rate_limit!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `window_get_active_refresh_rate`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_get_active_refresh_rate!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32 / Option<f32>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = window_get_active_refresh_rate!(ctx.run);
        let _ = value;
    }
});
```
