# Visual Accessibility

Access:

- Direct `ResourceContext` methods/macros (no module accessor).

Macros:

- `enable_colorblind_filter!(res, filter, strength)`
- `disable_colorblind_filter!(res)`

Methods:

- `res.enable_colorblind_filter(filter, strength)`
- `res.disable_colorblind_filter()`

Current settings:

- `ColorBlindFilter::Protan`
- `ColorBlindFilter::Deuteran`
- `ColorBlindFilter::Tritan`
- `ColorBlindFilter::Achroma`

Notes:

- `Protan`/`Deuteran`/`Tritan` are correction modes.
- `Achroma` is a luminance contrast-assist mode (not color restoration).

Behavior:

- Visual accessibility is global (not per camera).
- Only one color-blind filter can be active at a time.
- Visual accessibility is applied after camera post-processing as the final render pass.

Example:

```rust
enable_colorblind_filter!(res, ColorBlindFilter::Tritan, 0.75);

// Replace mode:
enable_colorblind_filter!(res, ColorBlindFilter::Protan, 0.9);

// Disable:
disable_colorblind_filter!(res);
```
