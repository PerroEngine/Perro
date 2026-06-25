# Resource API

## Page Map

| Header | Link |
| --- | --- |
| Resource Window | [Resource Window](#resource-window) |
| Resource Modules | [Resource Modules](#resource-modules) |
| Example | [Example](#example) |

## Resource Window

Use `ctx.res` for resources and renderer-facing resource commands. Render resource loads return stable IDs immediately. Decode and upload can finish later without blocking the frame; renderer uses the ID once data is ready.

For lifetime rules, auto load, auto drop, and ref-count behavior, see [Resource Management](../../resources/resource_management.md).

## Resource Modules

| Module | Page | Ctx |
| --- | --- | --- |
| Animations | [animations](resource_modules/animations.md) | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Audio | [audio](resource_modules/audio.md) | `ctx.res.Audio()` |
| Csv | [csv](resource_modules/csv.md) | `ctx.res.Csv()` |
| Draw 2D | [draw_2d](resource_modules/draw_2d.md) | `ctx.res.Draw2D()` |
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

## Example

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let texture = texture_load!(ctx.res, "res://textures/player.png");
        let ready = texture_is_loaded!(ctx.res, texture);
        let _ = ready;
    }
});
```
