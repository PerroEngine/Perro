# Resource API

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Resource Window | [Resource Window](#resource-window) |
| Resource Modules | [Resource Modules](#resource-modules) |
| Global Visual State | [Global Visual State](#global-visual-state) |
| Practical Example | [Practical Example](#practical-example) |

## Purpose

`ctx.res` is how a script loads and creates the assets a game is built from: textures, meshes, materials, audio, animations, spreadsheets, scene documents, and live capture from the mic or webcam. Render-facing loads return a stable ID immediately and never block the frame; decode and GPU upload finish in the background, and the renderer starts using the ID the moment its data is ready. This lets gameplay code ask for an asset the instant it is needed without stalling to disk.

## Use Cases

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Scene instance always uses one authored texture/mesh/material | typed state asset ID + scene path injection | Scene resolves a stable cached ID before `on_init` | Invalid path keeps the field default; runtime `set_var!` does not perform this coercion |
| Runtime path is selected by gameplay | resource `load`/load macro | Returns a stable ID immediately and uses normal caches | Decode/upload may still be in flight; poll readiness only when behavior requires it |
| Loading screen pins an asset | `reserve` then later `drop` | Explicit lifetime keeps it resident across short gaps | Owner must balance the reservation |
| Procedural content has no source file | create API | Builds resource data directly from Rust values | Caller owns data validation and lifetime |
| CSV/localization supplies authored game data | data-specific resource module | Parser and lookup semantics stay typed to the format | Missing rows/keys need product fallback text/data |
| Webcam/mic supplies live data | capture module | Resource API owns device/backend integration | Permission, disconnect, and unavailable-device paths are normal runtime states |

## Resource Window

Use `ctx.res` for resources and renderer-facing resource commands. Render resource loads return stable IDs immediately. Decode and upload can finish later without blocking the frame; the renderer uses the ID once data is ready.

For lifetime rules, auto load, auto drop, and ref-count behavior, see [Resource Management](../../resources/resource_management.md).

## Resource Modules

| Module | Page | Ctx |
| --- | --- | --- |
| Animations | [animations](resource_modules/animations.md) | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Audio | [audio](resource_modules/audio.md) | `ctx.res.Audio()` |
| Csv | [csv](resource_modules/csv.md) | `ctx.res.Csv()` |
| Draw 2D | [draw_2d](resource_modules/draw_2d.md) | `ctx.res.Draw2D()` |
| Display HDR | [display](resource_modules/display.md) | `ctx.res.Display()` |
| GLBs | [glbs](resource_modules/glbs.md) | `ctx.res.Glbs()` |
| Localization | [localization](resource_modules/localization.md) | `ctx.res.Localization()` |
| Materials | [materials](resource_modules/materials.md) | `ctx.res.Materials()` |
| Meshes | [meshes](resource_modules/meshes.md) | `ctx.res.Meshes()` |
| Mic | [mic](resource_modules/mic.md) | `ctx.res.Mic()` |
| Post Processing | [post_processing](resource_modules/post_processing.md) | `ctx.res` |
| Scene Docs | [scene_docs](resource_modules/scene_docs.md) | `ctx.res.SceneDocs()` |
| Skeletons | [skeletons](resource_modules/skeletons.md) | `ctx.res.Skeletons()` |
| Textures | [textures](resource_modules/textures.md) | `ctx.res.Textures()` |
| Visual Accessibility | [visual_accessibility](resource_modules/visual_accessibility.md) | `ctx.res` |
| Webcams | [webcam](resource_modules/webcam.md) | `ctx.res.Webcams()` |

## Global Visual State

A few whole-screen controls live directly on `ctx.res` rather than in a module, because they affect the final composited frame instead of a single asset.

| Call | Signature | Purpose |
| --- | --- | --- |
| Post-processing set | `ctx.res.set_global_post_processing(set)` | Replace the full global effect stack. |
| Post-processing add | `ctx.res.add_global_post_processing(effect)` | Append one effect. |
| Colorblind filter | `ctx.res.enable_colorblind_filter(mode, strength)` | Enable an accessibility simulation pass. |
| Viewport size | `ctx.res.viewport_size() -> Vector2` | Read the active viewport size in pixels. |
| HDR mode | `ctx.res.Display().set_hdr_mode(mode)` | Request auto, on, or off display HDR. |
| Locale shortcuts | `ctx.res.set_locale(...)`, `ctx.res.locale(key)` | Direct locale access without `Localization()`. |

See [Post Processing](resource_modules/post_processing.md) and [Visual Accessibility](resource_modules/visual_accessibility.md) for the full effect and filter reference.

## Practical Example

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let texture = texture_load!(ctx.res, "res://textures/player.png");
        self.hold(ctx, texture);
    }
});

methods!({
    fn hold(&self, ctx: &mut ScriptContext<'_, API>, texture: TextureID) {
        // The renderer starts using `texture` once its async decode finishes.
        let ready = texture_is_loaded!(ctx.res, texture);
        let _ = ready;
    }
});
```
