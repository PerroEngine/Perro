# `.uistyle` Format

`.uistyle` is the Perro UI style resource.
It uses the same schema as current scene `style = { ... }` blocks.

## Example

```text
fill = "#222222DD"
stroke = "#555555FF"
stroke_width = 1
radius = 0.2
shadow = { color = "#00000066" distance = 10 falloff = 12 vector = (1, -1) size = 2 }
highlight = { color = "#FFFFFF33" distance = 2 falloff = 3 vector = (-1, 1) size = 1 }
```

Object form should also parse:

```text
{
    fill = "#222222DD"
    stroke = "#555555FF"
    radius = 0.2
}
```

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
- `stroke`
- `stroke_width`
- `radius` or `corner_radius`
- `shadow`
- `highlight` or `inner_highlight`
- `shadow_color`, `shadow_distance`, `shadow_falloff`, `shadow_vector`, `shadow_size`
- `highlight_color`, `highlight_distance`, `highlight_falloff`, `highlight_vector`, `highlight_size`

## Static Pipeline

Static pipeline emits `.uistyle` files into `static_assets::ui_styles`.
Scene loading uses the static lookup in build output and runtime parsing in dev mode.
