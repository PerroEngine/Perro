# Accessibility Module

Access:

- `res.Accessibility()`

Macros:

- `enable_colorblind_filter!(res, filter, strength)`
- `disable_colorblind_filter!(res)`

Methods:

- `res.enable_colorblind_filter(filter, strength)`
- `res.disable_colorblind_filter()`
- `res.Accessibility().enable_color_blind(filter, strength)`
- `res.Accessibility().disable_color_blind()`

Current settings:

- `ColorBlindFilter::Protanopia`
- `ColorBlindFilter::Deuteranopia`
- `ColorBlindFilter::Tritanopia`

Behavior:

- Accessibility is global (not per camera).
- Only one color-blind filter can be active at a time.
- Accessibility is applied after camera post-processing as the final render pass.

Example:

```rust
enable_colorblind_filter!(res, ColorBlindFilter::Tritanopia, 0.75);

// Replace mode:
enable_colorblind_filter!(res, ColorBlindFilter::Protanopia, 0.9);

// Disable:
disable_colorblind_filter!(res);
```
