# Mouse Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `down` | [`down`](#down) |
| `pressed` | [`pressed`](#pressed) |
| `released` | [`released`](#released) |
| `delta` | [`delta`](#delta) |
| `wheel` | [`wheel`](#wheel) |
| `position` | [`position`](#position) |
| `viewport_size` | [`viewport_size`](#viewport_size) |
| `mode` | [`mode`](#mode) |
| `set_mode` | [`set_mode`](#set_mode) |
| `show` | [`show`](#show) |
| `hide` | [`hide`](#hide) |
| `capture` | [`capture`](#capture) |
| `confine` | [`confine`](#confine) |
| `confine_hidden` | [`confine_hidden`](#confine_hidden) |
| Macros | [Macros](#macros) |

## Purpose

The mouse module reports button edges, motion delta, wheel, cursor position, and
viewport size, and it queues cursor-mode changes. The key distinction is
position versus delta: `position` gives an absolute normalized point for UI and
world picking, while `delta` gives raw relative motion for camera look, which
keeps working even when the cursor is captured and cannot move on screen.

## Use Cases

- Click to shoot / select: fire on the press edge with
  `mouse_pressed!(ctx.ipt, MouseButton::Left)`.
- First-person / orbit camera look: capture the cursor with
  `mouse_set_mode!(ctx.ipt, MouseMode::Captured)` and rotate from
  `mouse_delta!(ctx.ipt)`.
- Drag to pan or rotate: hold-detect with `mouse_down!(ctx.ipt,
  MouseButton::Right)` and accumulate `mouse_delta!`.
- Scroll to zoom: read `mouse_wheel!(ctx.ipt).y` and adjust camera distance.
- Cursor-space UI and world picking: convert `mouse_position!(ctx.ipt)`
  (normalized viewport) against `viewport_size!(ctx.ipt)`.
- Menu vs. gameplay cursor: `mouse_show!(ctx.ipt)` in menus,
  `mouse_hide!` / `mouse_confine!(ctx.ipt)` during play.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Mouse()`
- `MouseButton` is the mouse-button enum (`Left`, `Right`, `Middle`, extras).
- `MouseMode` is the cursor-mode enum (`Visible`, `Hidden`, `Captured`, `Confined`, `ConfinedHidden`).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        // Enter mouselook: hide + lock the cursor for camera control.
        mouse_set_mode!(ctx.ipt, MouseMode::Captured);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Relative motion keeps flowing while the cursor is captured.
        let look = mouse_delta!(ctx.ipt);
        let zoom = mouse_wheel!(ctx.ipt).y;

        if mouse_pressed!(ctx.ipt, MouseButton::Left) {
            // fire / select at mouse_position!(ctx.ipt)
        }
        let _ = (look, zoom);
    }
});
```

## API Reference

### `down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn down(&self, button: MouseButton) -> bool` |
| Params | `&self, button: MouseButton` |
| Returns | `bool` |
| Use when | A control must respond while the button is held, such as drag or aim. |
| Edge behavior | `true` every frame the button is held. |

### `pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn pressed(&self, button: MouseButton) -> bool` |
| Params | `&self, button: MouseButton` |
| Returns | `bool` |
| Use when | Fire once on click, such as shoot or select. |
| Edge behavior | `true` only on the down-edge frame. |

### `released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn released(&self, button: MouseButton) -> bool` |
| Params | `&self, button: MouseButton` |
| Returns | `bool` |
| Use when | Fire on release, such as ending a drag or a charged action. |
| Edge behavior | `true` only on the up-edge frame. |

### `delta`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn delta(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Camera look and relative motion; works while the cursor is captured. |
| Edge behavior | Accumulated pixel movement for the frame; zero when the mouse did not move. |

### `wheel`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn wheel(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Zoom, weapon cycling, or list scroll; read the `y` component. |
| Edge behavior | Accumulated wheel movement for the frame; zero when no scroll occurred. |

### `position`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn position(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | UI hit-testing and world picking. |
| Edge behavior | Normalized viewport position clamped to `0..1`, with bottom-left as `(0, 0)`. |

### `viewport_size`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn viewport_size(&self) -> Vector2` |
| Params | `&self` |
| Returns | `Vector2` |
| Use when | Convert normalized `position` back to pixels, or compute aspect. |
| Edge behavior | Pixel size of the current viewport; defaults to a `1x1` fallback before the first size event. |

### `mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn mode(&self) -> MouseMode` |
| Params | `&self` |
| Returns | `MouseMode` |
| Use when | Branch on the current cursor mode. |
| Edge behavior | Reflects the last applied mode from the snapshot. |

### `set_mode`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn set_mode(&self, mode: MouseMode)` |
| Params | `&self, mode: MouseMode` |
| Returns | `()` |
| Use when | Switch cursor behavior, such as entering or leaving mouselook. |
| Edge behavior | Queues a command; applies on the next input frame when a command buffer exists. |

### `show`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn show(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Show a normal cursor, such as in menus. Queues `MouseMode::Visible`. |
| Edge behavior | Queued command; applies on the next input frame. |

### `hide`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn hide(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Hide the cursor without locking it. Queues `MouseMode::Hidden`. |
| Edge behavior | Queued command; applies on the next input frame. |

### `capture`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn capture(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Lock and hide the cursor for mouselook. Queues `MouseMode::Captured`. |
| Edge behavior | Queued command; `delta` keeps reporting motion while captured. |

### `confine`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn confine(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Keep a visible cursor inside the window. Queues `MouseMode::Confined`. |
| Edge behavior | Queued command; applies on the next input frame. |

### `confine_hidden`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Mouse()` |
| Signature | `pub fn confine_hidden(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Keep a hidden cursor inside the window. Queues `MouseMode::ConfinedHidden`. |
| Edge behavior | Queued command; applies on the next input frame. |

## Macros

Macro forms expand to the methods above and read the current input snapshot.
Its default mouse state uses unpressed buttons, zero motion/position vectors,
and `MouseMode::Visible`. Command macros queue work only when an input command
buffer exists.

| Macro | Signature | Returns |
| --- | --- | --- |
| `mouse_down!` | `mouse_down!(ctx.ipt, MouseButton::Left)` | `bool` |
| `mouse_pressed!` | `mouse_pressed!(ctx.ipt, MouseButton::Left)` | `bool` |
| `mouse_released!` | `mouse_released!(ctx.ipt, MouseButton::Left)` | `bool` |
| `mouse_delta!` | `mouse_delta!(ctx.ipt)` | `Vector2` |
| `mouse_wheel!` | `mouse_wheel!(ctx.ipt)` | `Vector2` |
| `mouse_position!` | `mouse_position!(ctx.ipt)` | `Vector2` |
| `viewport_size!` | `viewport_size!(ctx.ipt)` | `Vector2` |
| `mouse_mode!` | `mouse_mode!(ctx.ipt)` | `MouseMode` |
| `mouse_set_mode!` | `mouse_set_mode!(ctx.ipt, MouseMode::Captured)` | `()` |
| `mouse_show!` | `mouse_show!(ctx.ipt)` | `()` |
| `mouse_hide!` | `mouse_hide!(ctx.ipt)` | `()` |
| `mouse_capture!` | `mouse_capture!(ctx.ipt)` | `()` |
| `mouse_confine!` | `mouse_confine!(ctx.ipt)` | `()` |
| `mouse_confine_hidden!` | `mouse_confine_hidden!(ctx.ipt)` | `()` |
