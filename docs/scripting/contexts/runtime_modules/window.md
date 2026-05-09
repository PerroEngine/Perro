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
```

`set_size` takes physical pixel width and height.

Zero width or height is ignored.

## Macros

```rust
window_set_title!(ctx.run, "New Title");
window_set_size!(ctx.run, 1280, 720);
window_set_mode!(ctx.run, WindowMode::BorderlessFullscreen);
```
