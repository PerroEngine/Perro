# Window Module

Use `ctx.run.Window()` to queue runtime window changes.

Changes apply on the app thread after script update.

## Methods

```rust
ctx.run.Window().set_title("New Title");
ctx.run.Window().set_size(1280, 720);
ctx.run.Window().set_windowed();
ctx.run.Window().set_borderless_fullscreen();
ctx.run.Window().set_mode(WindowMode::Windowed);
ctx.run.Window().set_mode(WindowMode::BorderlessFullscreen);
ctx.run.Window().set_frame_rate_limit(144.0);
ctx.run.Window().set_refresh_rate_cap();
ctx.run.Window().set_unlimited_frame_rate();
ctx.run.Window().set_frame_rate_cap(FrameRateCap::RefreshRate);
let refresh = ctx.run.Window().get_active_refresh_rate();
```

`set_size` takes physical pixel width and height.

Zero width or height is ignored.

Frame caps accept `FrameRateCap::Unlimited`, `FrameRateCap::Fps(f32)`, or `FrameRateCap::RefreshRate`.

`get_active_refresh_rate` returns monitor Hz when known.

## Macros

```rust
window_set_title!(ctx.run, "New Title");
window_set_size!(ctx.run, 1280, 720);
window_set_mode!(ctx.run, WindowMode::BorderlessFullscreen);
window_set_frame_rate_limit!(ctx.run, 144.0);
window_set_frame_rate_cap!(ctx.run, FrameRateCap::RefreshRate);
window_get_active_refresh_rate!(ctx.run);
```
