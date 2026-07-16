# Post Processing Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Effects | [Effects](#effects) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `post_processing_set` | [`post_processing_set`](#post_processing_set) |
| `post_processing_add` | [`post_processing_add`](#post_processing_add) |
| `post_processing_remove` | [`post_processing_remove`](#post_processing_remove) |
| `post_processing_clear` | [`post_processing_clear`](#post_processing_clear) |

## Purpose

Global post-processing runs full-screen image effects over the final composited frame: vignette, bloom, color grading, blur, CRT, LUTs, and more. Effects live on `ctx.res` because they change the whole picture rather than any single asset. Named effects let a script toggle or retune one look at a time (a damage flash, a poisoned tint) without disturbing the rest of the stack.

## Use Cases

- Damage-flash vignette: on a hit, `post_processing_add!(ctx.res, "hurt", PostProcessEffect::Vignette { .. })`, then `post_processing_remove!(ctx.res, name = "hurt")` when it fades.
- Underwater color grade: while submerged, add a named `PostProcessEffect::ColorGrade { .. }` with a cool temperature and reduced saturation; remove it on surfacing.
- Retro/CRT mode: apply `PostProcessEffect::Crt { .. }` for an arcade cabinet look, toggled from an options menu.
- Cinematic bloom and exposure: layer `PostProcessEffect::Bloom { .. }` and `PostProcessEffect::Exposure { .. }` for bright emissive scenes.
- Level-wide look presets: build a whole `PostProcessSet` per biome and swap it with `post_processing_set!` on scene load.
- Screenshot / cutscene reset: `post_processing_clear!(ctx.res)` to drop every effect before a clean capture.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res` (calls live directly on the resource window, not a sub-module)
- Backing types: `perro_structs::{PostProcessEffect, PostProcessSet}`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Effects

`PostProcessEffect` is an enum; each variant carries its own tuning fields:

| Variant | Key fields |
| --- | --- |
| `Blur` | `strength` |
| `Pixelate` | `size` |
| `Warp` | `waves, strength` |
| `Vignette` | `strength, radius, softness` |
| `Crt` | `scanline_strength, curvature, chromatic, vignette` |
| `ColorFilter` | `color: [f32; 3], strength` |
| `ReverseFilter` | `color: [f32; 3], strength, softness` |
| `Bloom` | `strength, threshold, radius` |
| `Exposure` | `exposure, auto_exposure, min_exposure, max_exposure, speed_up, speed_down, target_luminance` |
| `Saturate` | `amount` |
| `BlackWhite` | `amount` |
| `ColorGrade` | `exposure, contrast, brightness, saturation, gamma, temperature, tint, hue_shift, vibrance, lift, gain, offset` |
| `Lut2D` / `Lut3D` | `texture_path, size, strength` |
| `Custom` | `shader_path, params` |

Add effects by name to update or remove them individually later; add them unnamed for one-off stacking. `PostProcessSet` also offers `add`, `remove`, `rename`, and `get` when you build a stack in code before calling `post_processing_set!`.

## Practical Example

Flash a red vignette when the player takes damage and clear it a moment later.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, signal!("player_hurt"), func!("on_hurt"));
    }
});

methods!({
    fn on_hurt(&self, ctx: &mut ScriptContext<'_, API>) {
        post_processing_add!(
            ctx.res,
            "hurt",
            PostProcessEffect::Vignette { strength: 0.9, radius: 0.6, softness: 0.4 }
        );
    }

    fn on_hurt_recovered(&self, ctx: &mut ScriptContext<'_, API>) {
        let removed = post_processing_remove!(ctx.res, name = "hurt");
        let _ = removed;
    }
});
```

## API Reference

### `post_processing_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_set!(ctx.res, set)` |
| Params | `ctx.res, set: PostProcessSet` |
| Returns | `()` |
| Use when | Replacing the entire global effect stack at once, for example a per-biome preset. |
| Fails when / edge behavior | Overwrites all existing global effects. |

### `post_processing_add`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_add!(ctx.res, effect)` or `post_processing_add!(ctx.res, name, effect)` |
| Params | `ctx.res, effect: PostProcessEffect` (optional leading `name: impl Into<Cow<'static, str>>`) |
| Returns | `()` |
| Use when | Adding one effect. The named form replaces an existing effect with the same name; the unnamed form appends. |
| Fails when / edge behavior | Adding a name that already exists updates that effect in place. |

### `post_processing_remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_remove!(ctx.res, name = name)` or `post_processing_remove!(ctx.res, index = index)` |
| Params | `ctx.res, name: &str` or `index: usize` |
| Returns | `bool` |
| Use when | Removing one previously added effect by name or by index. |
| Fails when / edge behavior | Returns `false` when no effect matches the name or index. |

### `post_processing_clear`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `post_processing_clear!(ctx.res)` |
| Params | `ctx.res` |
| Returns | `()` |
| Use when | Dropping every global effect, for example before a clean screenshot or cutscene. |
| Fails when / edge behavior | No-op when the stack is already empty. |
