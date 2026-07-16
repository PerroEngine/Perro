# Visual Accessibility Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Filter Modes | [Filter Modes](#filter-modes) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `enable_colorblind_filter` | [`enable_colorblind_filter`](#enable_colorblind_filter) |
| `disable_colorblind_filter` | [`disable_colorblind_filter`](#disable_colorblind_filter) |

## Purpose

A global colorblind filter runs a full-screen color-correction pass so players with color vision deficiencies can tell gameplay colors apart. It is a whole-frame control on `ctx.res`, driven by an accessibility menu, with a mode for the deficiency type and a strength for how strong the correction is. Wire it to a settings toggle so the choice persists across the session.

## Use Cases

- Accessibility options menu: apply the player's chosen mode and strength with `enable_colorblind_filter!(ctx.res, mode, strength)` when the setting changes.
- Protanopia / deuteranopia / tritanopia correction: pass `ColorBlindFilter::Protan`, `Deuteran`, or `Tritan` to shift confusable red/green/blue hues apart.
- Achromatopsia support: pass `ColorBlindFilter::Achroma` for total color blindness.
- Strength slider: pass a `0.0..=1.0` strength so players tune the effect to their vision.
- Turning the filter off: `disable_colorblind_filter!(ctx.res)` when the player selects "None".

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res` (calls live directly on the resource window)
- Backing type: `perro_structs::ColorBlindFilter`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Filter Modes

`ColorBlindFilter` selects which deficiency the correction pass targets:

| Mode | Targets |
| --- | --- |
| `ColorBlindFilter::Protan` | Protanopia (red-weak) |
| `ColorBlindFilter::Deuteran` | Deuteranopia (green-weak) |
| `ColorBlindFilter::Tritan` | Tritanopia (blue-weak) |
| `ColorBlindFilter::Achroma` | Achromatopsia (total color blindness) |

`strength` is a `f32` from `0.0` (no correction) to `1.0` (full correction).

## Practical Example

Apply the filter when an options menu commits a new accessibility choice.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, signal!("accessibility_changed"), func!("apply_filter"));
    }
});

methods!({
    fn apply_filter(&self, ctx: &mut ScriptContext<'_, API>) {
        // Values would come from the settings the player picked.
        enable_colorblind_filter!(ctx.res, ColorBlindFilter::Deuteran, 0.8);
    }

    fn clear_filter(&self, ctx: &mut ScriptContext<'_, API>) {
        disable_colorblind_filter!(ctx.res);
    }
});
```

## API Reference

### `enable_colorblind_filter`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `enable_colorblind_filter!(ctx.res, mode, strength)` |
| Params | `ctx.res, mode: ColorBlindFilter, strength: f32` |
| Returns | `()` |
| Use when | Enabling or retuning the global colorblind correction pass. |
| Fails when / edge behavior | Replaces any active filter; `strength` outside `0.0..=1.0` is clamped by the render pass. |

### `disable_colorblind_filter`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `disable_colorblind_filter!(ctx.res)` |
| Params | `ctx.res` |
| Returns | `()` |
| Use when | Turning the filter off, for example when the player selects "None". |
| Fails when / edge behavior | No-op when no filter is active. |
