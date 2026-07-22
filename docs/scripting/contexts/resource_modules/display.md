# Display HDR

## Purpose

`ctx.res.Display()` controls real HDR display output and reports the renderer's resolved state.
The requested mode and active mode stay separate because the monitor, OS, graphics backend, and
adapter all affect support.

## Modes

| Mode | Result |
| --- | --- |
| `HdrMode::Off` | Force an SDR sRGB surface. |
| `HdrMode::Auto` | Use native linear extended-sRGB HDR when the surface and float scene path support it. |
| `HdrMode::On` | Request HDR; safely fall back to SDR when unsupported. |

## API

```rust
hdr_set!(ctx.res, HdrMode::Auto);

let status = hdr_status!(ctx.res);
let supported = hdr_supported!(ctx.res);
let active = hdr_active!(ctx.res);
```

Method syntax:

```rust
ctx.res.Display().set_hdr_mode(HdrMode::On);
let status = ctx.res.Display().hdr_status();
```

`HdrStatus` reports requested mode, support, active state, internal scene HDR, resolved color
space, live highlight headroom, optional peak nits, and any fallback reason. Queries read the last
renderer status without blocking on GPU or display calls.

Native HDR currently uses `Rgba16Float` with `ExtendedSrgbLinear` (scRGB). The renderer rechecks
the display on resize and periodically while frames render, so monitor moves and OS HDR changes
can update the active state. Web HDR and HDR10/PQ output fall back to SDR until their encoded
output paths are available.
