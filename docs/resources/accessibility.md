# Accessibility

Accessibility settings are **global** and are **not** configured per camera, unlike post processing.

They run as a dedicated final fullscreen pass **after** camera post-processing:

1. Scene render (3D + particles + 2D)
2. Active camera post-processing chain (if any)
3. Global accessibility pass (if enabled)

Current built-in accessibility:

- Color-blind filter (`Protanopia`, `Deuteranopia`, `Tritanopia`) with `strength`.

## Script API

Direct ResourceContext methods:

- `res.enable_colorblind_filter(ColorBlindFilter::Deuteranopia, 0.8)`
- `res.disable_colorblind_filter()`

Macros:

- `enable_colorblind_filter!(res, ColorBlindFilter::Deuteranopia, 0.8)`
- `disable_colorblind_filter!(res)`

## Example

```rust
enable_colorblind_filter!(res, ColorBlindFilter::Deuteranopia, 0.85);

// Later
disable_colorblind_filter!(res);
```

## Relation To Post Processing

- Camera post-processing (`post_processing`) stays per-camera and ordered.
- Accessibility is separate/global and always runs last.
- See [Post Processing](postprocess.md) for per-camera effects.
