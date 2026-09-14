# Visual parity audit — 2026-09-13

Follow-up fix: SVG decode now uses straight alpha; UI normalizes managed/user alpha conventions; display UI + splash composite after scene tone map. Keep existing sRGB surface selection + HDR auto. Move accessibility to final display pass. Global art fx stay on scene. Original findings below describe pre-fix behavior; final test results belong to follow-up report.

Follow-up checks: pass 561 graphics tests on Windows, then pass new managed/user UI translucency + opaque gray GPU checks. Pass SVG SDR/HDR readback checks, native-sRGB vs sRGB-view alias checks, shader validation, loose/PTEX alpha parity, and Cargo check. No native macOS/Linux run; no generic gamma or Metal-specific output change.

Scope: current worktree; generated desktop games + dev path.
Use 2 read-only agent reviews + root splash trace + local tests.
Keep engine code intact.

## Result

- Reject claim of proven Win/macOS/Linux visual parity: no three-OS capture run in this audit.
- Find shared source-color shifts independent of OS: full-frame ACES + SVG alpha mismatch.
- Find display-dependent output: default HDR auto + headroom-dependent curve.
- Find existing fix for common plain-vs-sRGB surface view mismatch.

## 1. Splash + UI pass through ACES

Trace `perro_source/render_stack/perro_app/src/winit_runner/splash.rs`: white sprite tint; alpha=1 during hold.
Trace `perro_source/render_stack/perro_graphics/src/gpu/frame.rs:1948`: draw late overlay into composite.
Trace same file `:1990`: apply global post fx + accessibility after overlay.
Trace same file `:2016`: send full composite to final present.
Trace `perro_source/render_stack/perro_graphics/src/gpu/present.rs:610`: unconditional ACES curve, SDR included.
Use default exposure=0 (`:510`); still apply curve.

Calculate opaque, uniform texels with shader formula + sRGB decode/encode; no GPU measurement:

| Source RGB | Default SDR output, approx |
| --- | --- |
| 255,255,255 | 232,232,232 |
| 128,128,128 | 154,154,154 |
| 240,128,64 | 228,154,61 |

Explain full-logo color shift even with global post fx off.
Avoid claim of universal desaturation: direction depends on source color.
Fix direction: tone-map scene before source-faithful UI/splash composite; define separate HDR UI reference-white policy.
Keep accessibility behavior explicit when moving splash stage.

## 2. SVG alpha contract mismatch

Trace `perro_source/render_stack/perro_graphics_assets/src/texture.rs:292`: `pixmap.take()` retains premultiplied RGBA.
Trace `perro_source/render_stack/perro_graphics/src/two_d/shaders/sprite_instanced.wgsl:53`: sample * tint; no unpremultiply.
Trace `perro_source/render_stack/perro_graphics/src/two_d/gpu/helpers.rs:406`: straight-alpha `ALPHA_BLENDING`.
Trace `perro_source/render_stack/perro_graphics/src/shared_textures.rs:41`: sRGB texture format.

Result: SVG translucent pixels contain RGB * alpha before sRGB decode; blend multiplies alpha again.
Expect dark translucent fills + antialias edges; no effect on fully opaque interior texels from this bug alone.
Find additional UI contract mismatch: `perro_source/render_stack/perro_graphics/src/ui/gpu.rs:385` uses premultiplied blending; `ui/gpu/shaders.rs:48` samples * tint without explicit premultiply. PNG decode supplies straight RGBA; SVG supplies premultiplied RGBA. Gamma-space premultiplication also differs from linear-space premultiplication after texture sampling.
Fix direction: canonical straight sRGBA at asset boundary; demultiply SVG before sRGB upload; retain straight-alpha 2D pipeline; premultiply final linear RGB by final alpha for premultiplied UI pipeline.
Rebuild PTEX assets after decoder fix.

Trace `perro_source/build_pipeline/perro_static_pipeline/src/textures.rs:79`: pack uses shared SVG decode.
Trace `perro_source/render_stack/perro_graphics/src/backend/asset_load.rs:253`: static PTEX / loose source load paths.
Result: same alpha defect in generated + dev games; packed bytes freeze decoder output at build time.

## 3. HDR auto changes output by capability + display

Trace `perro_source/runtime_project/perro_project/src/config.rs:400`: default `HdrMode::Auto`.
Trace `perro_source/render_stack/perro_graphics/src/gpu/present.rs:227`: choose float extended-linear output when available.
Use reported headroom (`:241`); apply `ACES(color / headroom) * headroom` (`:621`).
Result: different headroom -> different mapping of same source texel.
Classify as deliberate adaptive output, not proof of backend error.
Use HDR off for initial SDR parity comparison; still fix ACES/alpha source-color issues separately.

## 4. Common sRGB surface workaround already present

Trace `perro_source/render_stack/perro_graphics/src/gpu/present.rs:2154`: map plain RGBA/BGRA 8-bit formats to sRGB views.
Prefer sRGB surface format (`:260`); use linear intermediate target (`:2129`).
Verify alias declaration in `gpu/lifecycle.rs:443`, acquired texture view format at `:128`, and matching present pipeline format in `gpu/present.rs:930`.
Find helper in git history at commit `2276d95c`; do not infer original reported Mac/Windows incident from commit alone.
Require exactly one final sRGB encode. Adding another manual gamma encode on this normal SDR path risks double encode.

Use [wgpu format docs](https://docs.rs/wgpu/latest/wgpu/enum.TextureFormat.html) + [surface color-space docs](https://docs.rs/wgpu/latest/x86_64-apple-darwin/wgpu/enum.SurfaceColorSpace.html): sRGB view encodes on write; color-space selection alone does not encode.
Use [tiny-skia Pixmap docs](https://docs.rs/tiny-skia/latest/tiny_skia/struct.Pixmap.html): premultiplied pixels + demultiplied extraction API.

## Checks + limits

- Pass 23 `cargo test -p perro_graphics --lib gpu::present::tests -- --test-threads=1` tests.
- Pass 13 `cargo test -p perro_graphics_assets --lib texture::tests -- --test-threads=1` tests; skip 1 bench.
- Run on Windows only; these checks validate CPU policy + shader syntax, not three-OS displayed pixels.
- Find Win/macOS/Linux CI test matrix in `.github/workflows/ci.yml`; no full generated-game screenshot parity proof from that matrix.
- Find headless GPU checks at `perro_source/render_stack/perro_graphics/tests/unit/present_gpu_tests.rs:13`; adapter uses `compatible_surface: None`, so checks do not cover OS swapchain or display color management.
- Do not run generated binaries or inspect actual Mac/Linux screens in this audit.

## Next verification

1. Add translucent SVG pixel regression + loose/PTEX byte parity check.
2. Add GPU readback swatches for PNG, SVG, splash, UI, gray ramp, transparent edges.
3. Build same fixture for Win DX12/Vulkan, macOS Metal, Linux Vulkan.
4. Compare SDR first; log adapter, backend, surface/view format, color space, HDR state, headroom, scale, exposure.
5. Compare HDR separately with explicit reference-white target; distinguish GPU bytes from OS/display color management.
