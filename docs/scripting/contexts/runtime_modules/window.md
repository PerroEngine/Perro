# Window Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
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
| `close_app` | [`close_app`](#close_app) |
| `window_set_title` | [`window_set_title`](#window_set_title) |
| `window_set_size` | [`window_set_size`](#window_set_size) |
| `window_set_mode` | [`window_set_mode`](#window_set_mode) |
| `window_set_frame_rate_cap` | [`window_set_frame_rate_cap`](#window_set_frame_rate_cap) |
| `window_set_frame_rate_limit` | [`window_set_frame_rate_limit`](#window_set_frame_rate_limit) |
| `window_get_active_refresh_rate` | [`window_get_active_refresh_rate`](#window_get_active_refresh_rate) |
| `close_app` | [`close_app`](#close_app-1) |

## Purpose

The window module is how a game applies its display and performance settings at
runtime. This is the code behind a video-options menu: resolution, windowed vs.
fullscreen, frame-rate caps, and the window title bar. It also owns the "quit to
desktop" request that a pause menu needs.

## Use Cases

- Apply video settings from an options menu: switch display mode with `window_set_mode!(ctx.run, WindowMode::BorderlessFullscreen)` (or `set_windowed` / `set_borderless_fullscreen`) and resolution with `window_set_size!(ctx.run, 1920, 1080)`.
- Cap FPS to save battery or reduce heat/coil whine: `window_set_frame_rate_limit!(ctx.run, 60.0)`.
- Sync frame rate to the monitor: `ctx.run.Window().set_refresh_rate_cap()`, reading the panel rate back with `window_get_active_refresh_rate!(ctx.run)`.
- Uncap for a benchmark or stress scene: `ctx.run.Window().set_unlimited_frame_rate()`.
- Show the current level or save-slot name in the title bar: `window_set_title!(ctx.run, "Perro - Level 3")`.
- Quit to desktop from a pause menu button: `close_app!(ctx.run)` queues an app-close request for the app layer.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Window()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

Apply saved video settings once at startup, then let a pause-menu handler quit
the game.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        window_set_title!(ctx.run, "Perro - Main Menu");
        ctx.run.Window().set_borderless_fullscreen();
        window_set_frame_rate_limit!(ctx.run, 144.0);
    }
});

methods!({
    // Wired to a "Quit" button's pressed signal.
    fn on_quit_pressed(&self, ctx: &mut ScriptContext<'_, API>) {
        close_app!(ctx.run);
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
| Use when | Use `set_title` to set title on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_title` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_size`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_size(&mut self, width: u32, height: u32)` |
| Params | `&mut self, width: u32, height: u32` |
| Returns | `()` |
| Use when | Use `set_size` to set size on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_size` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_mode(&mut self, mode: WindowMode)` |
| Params | `&mut self, mode: WindowMode` |
| Returns | `()` |
| Use when | Use `set_mode` to set mode on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_mode` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_windowed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_windowed(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use `set_windowed` to set windowed on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_windowed` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_borderless_fullscreen`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_borderless_fullscreen(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use `set_borderless_fullscreen` to set borderless fullscreen on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_borderless_fullscreen` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_frame_rate_cap`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_frame_rate_cap(&mut self, cap: FrameRateCap)` |
| Params | `&mut self, cap: FrameRateCap` |
| Returns | `()` |
| Use when | Use `set_frame_rate_cap` to set frame rate cap on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_frame_rate_cap` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_frame_rate_limit`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_frame_rate_limit(&mut self, fps: f32)` |
| Params | `&mut self, fps: f32` |
| Returns | `()` |
| Use when | Use `set_frame_rate_limit` to set frame rate limit on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_frame_rate_limit` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_refresh_rate_cap`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_refresh_rate_cap(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use `set_refresh_rate_cap` to set refresh rate cap on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_refresh_rate_cap` sends the command through the runtime module and the caller receives no acknowledgement. |

### `set_unlimited_frame_rate`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn set_unlimited_frame_rate(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use `set_unlimited_frame_rate` to set unlimited frame rate on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Has no failure return; `set_unlimited_frame_rate` sends the command through the runtime module and the caller receives no acknowledgement. |

### `get_active_refresh_rate`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn get_active_refresh_rate(&mut self) -> Option<f32>` |
| Params | `&mut self` |
| Returns | `Option<f32>` |
| Use when | Use `get_active_refresh_rate` to get active refresh rate on the app window; OS/backend constraints may alter a requested mode, size, or timing setting. |
| Fails when / edge behavior | Returns `None` when `get_active_refresh_rate` cannot produce a value for the supplied target or inputs. |

### `close_app`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `pub fn close_app(&mut self)` |
| Params | `&mut self` |
| Returns | `()` |
| Use when | Use `close_app` to close app on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Queues an app close request for the app layer to apply. |

### `window_set_title`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_title!(ctx.run, title)` |
| Params | `ctx, title` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `window_set_title` to window set title on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Returns `false` when `window_set_title` cannot apply to the supplied target or inputs; `true` confirms success. |

### `window_set_size`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_size!(ctx.run, width, height)` |
| Params | `ctx, width, height` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `window_set_size` to window set size on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Returns `false` when `window_set_size` cannot apply to the supplied target or inputs; `true` confirms success. |

### `window_set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_mode!(ctx.run, mode)` |
| Params | `ctx, mode` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `window_set_mode` to window set mode on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Returns `false` when `window_set_mode` cannot apply to the supplied target or inputs; `true` confirms success. |

### `window_set_frame_rate_cap`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_frame_rate_cap!(ctx.run, cap)` |
| Params | `ctx, cap` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `window_set_frame_rate_cap` to window set frame rate cap on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Returns `false` when `window_set_frame_rate_cap` cannot apply to the supplied target or inputs; `true` confirms success. |

### `window_set_frame_rate_limit`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_set_frame_rate_limit!(ctx.run, fps)` |
| Params | `ctx, fps` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `window_set_frame_rate_limit` to window set frame rate limit on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Returns `false` when `window_set_frame_rate_limit` cannot apply to the supplied target or inputs; `true` confirms success. |

### `window_get_active_refresh_rate`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `window_get_active_refresh_rate!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32 / Option<f32>` |
| Use when | Use `window_get_active_refresh_rate` to window get active refresh rate on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Returns `None` when `window_get_active_refresh_rate` cannot produce a value for the supplied target or inputs. |

### `close_app`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Window()` |
| Signature | `close_app!(ctx.run)` |
| Params | `ctx` |
| Returns | `()` |
| Use when | Use `close_app` to close app on the app window; the platform may constrain the requested mode, size, or timing value. |
| Fails when / edge behavior | Queues an app close request for the app layer to apply. |
