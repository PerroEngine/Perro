# `.uistyle` Format

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

`.uistyle` stores one reusable UI style, including `fill`, `stroke`, corner radii, gradients, shadows, and highlights, in a single file that many nodes share. It solves the problem of repeating the same inline `style = { ... }` block on every panel and button and then keeping them all in sync. Point a node's `style` (or a button's `hover`/`pressed`, or a text edit's `focused_style`) at the file, and restyle the whole game by editing one place.

## Use Cases

- One dark-fantasy button style reused across every menu: `fill`, `stroke`, and `corner_radii` in a shared `res://ui/button.uistyle`.
- Themed HUD and dialog panels: gradient fills via `fill_kind = "linear"` and a `gradient = { start_color, end_color, vector }` block.
- Depth and polish on cards: `outer_shadow` / `inner_shadow` plus `outer_highlight` / `inner_highlight`.
- Button state feedback: `hover = { style = "..." }` and `pressed = { style = "..." }` pointing at separate style files.
- Focused input fields: `focused_style = "res://ui/input_focus.uistyle"` on a text edit.
- Reskin without touching scenes: repoint a node's `style` at a different `.uistyle` to swap the whole look.

## Example

`res://ui/panel.uistyle`:

```text
fill = "#20242C"
stroke = "#586070"
radius = 0.12
```

Use it from a scene node:

```text
[card]
[UiPanel]
    anchor = "center"
    size_ratio = (0.45, 0.35)
    style = "res://ui/panel.uistyle"
[/UiPanel]
[/card]
```

## Reference

`.uistyle` is the Perro UI style resource.
It uses the same schema as scene `style = { ... }` blocks.

Bare field list form:

```text
fill = "#222222DD"
stroke = "#555555FF"
stroke_width = 1
fill_kind = "linear"
gradient = { start_color = "#2A3140FF" end_color = "#12161DFF" vector = (0, -1) }
corner_radii = (0.35, 0.35, 0.18, 0.18)
outer_shadow = { color = "#00000066" distance = 10 falloff = 12 vector = (1, -1) size = 2 }
inner_shadow = { color = "#00000040" distance = 4 falloff = 8 vector = (0, -1) size = 1 }
outer_highlight = { color = "#FFFFFF22" distance = 2 falloff = 4 vector = (-1, 1) size = 1 }
inner_highlight = { color = "#FFFFFF33" distance = 2 falloff = 3 vector = (-1, 1) size = 1 }
```

Object form parses too:

```text
{
    fill = "#222222DD"
    stroke = "#555555FF"
    radius = 0.2
}
```

Both the static pipeline and dev-mode runtime accept either form.
The parser wraps a bare field list in `{ }` before parsing, so the two are equivalent.
A file with no valid style field fails the static pipeline.

## Scene Use

Inline style stays valid:

```text
style = { fill = "#222" stroke = "#555" radius = 0.2 }
```

Resource style:

```text
style = "res://ui/panel.uistyle"
```

Button states:

```text
hover = { style = "res://ui/button_hover.uistyle" }
pressed = { style = "res://ui/button_down.uistyle" }
```

Text edit focus:

```text
focused_style = "res://ui/input_focus.uistyle"
```

## Fields

Supported keys:

- `fill` or `color`
- `fill_kind`
- `gradient`
- `stroke`
- `stroke_width`
- `radius` or `corner_radius`
- `corner_radii`
- `radius_tl`, `radius_tr`, `radius_br`, `radius_bl`
- `shadow`
- `outer_shadow`
- `inner_shadow`
- `highlight` or `inner_highlight`
- `outer_highlight`
- `shadow_color`, `shadow_distance`, `shadow_falloff`, `shadow_vector`, `shadow_size`
- `highlight_color`, `highlight_distance`, `highlight_falloff`, `highlight_vector`, `highlight_size`

## Static Pipeline

Static pipeline emits `.uistyle` files into `static_assets::ui_styles`.
Scene loading uses the static lookup in build output and runtime parsing in dev mode.
