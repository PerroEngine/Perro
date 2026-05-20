# Resource API

## Page Map

| Header | Link |
| --- | --- |
| Resource Window | [Resource Window](#resource-window) |
| Resource Modules | [Resource Modules](#resource-modules) |
| Example | [Example](#example) |

## Resource Window

Use `ctx.res` for resources and renderer-facing resource commands. Resource calls are shared/read-oriented at script level; many return stable IDs while upload or decode completes later.

## Resource Modules

| Module | Page | Ctx |
| --- | --- | --- |
| Animations | [animations](resource_modules/animations.md) | `ctx.res.Animations() / ctx.res.AnimationTrees()` |
| Audio | [audio](resource_modules/audio.md) | `ctx.res.Audio()` |
| Csv | [csv](resource_modules/csv.md) | `ctx.res.Csv()` |
| Draw 2D | [draw_2d](resource_modules/draw_2d.md) | `ctx.res.Draw2D()` |
| Localization | [localization](resource_modules/localization.md) | `ctx.res.Localization()` |
| Materials | [materials](resource_modules/materials.md) | `ctx.res.Materials()` |
| Meshes | [meshes](resource_modules/meshes.md) | `ctx.res.Meshes()` |
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
