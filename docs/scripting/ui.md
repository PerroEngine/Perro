# UI Nodes

UI nodes are data-only scene nodes backed by `perro_ui`.
They use `UiBox` as their base node type.

## Tree

```text
UiBox
- UiPanel
- UiButton
- UiLabel
- UiLayout
- UiHLayout
- UiVLayout
- UiGrid
```

## Nodes

`UiBox`

- Invisible UI container.
- Holds `layout`, `visible`, `input_enabled`, and `mouse_filter`.
- Can have UI children.
- Use it to group children, move them together, or create a padded child area.

`UiPanel`

- Drawn rect.
- Holds `style`.
- Can have children.

`UiButton`

- Button rect + text state.
- Holds normal, hover, and pressed styles.
- Can have children.

`UiLabel`

- Text visual.
- Holds `text`, `color`, `font_size`, and text alignment.
- Can have children, but usually should not.

`UiLayout`

- Invisible layout container.
- Uses `mode = "h" | "v" | "grid"`.
- Also accepts `horizontal`, `vertical`, `row`, `column`, and `g`.

`UiHLayout`

- Invisible horizontal layout container.
- Presets `mode = "h"`.

`UiVLayout`

- Invisible vertical layout container.
- Presets `mode = "v"`.

`UiGrid`

- Invisible grid layout container.
- Uses `columns`, `h_spacing`, and `v_spacing`.

## Layout Fields

Common fields live on `UiBox` data and all UI nodes inherit them:

- `anchor`
- `position`
- `position_percent`
- `position_ratio`
- `size`
- `size_percent`
- `size_ratio`
- `pivot`
- `pivot_percent`
- `pivot_ratio`
- `translation`
- `scale`
- `h_size`
- `v_size`
- `h_align`
- `v_align`
- `min_size`
- `max_size`
- `min_w`
- `min_h`
- `max_w`
- `max_h`
- `padding`
- `margin`
- `z_index`
- `visible`
- `input_enabled`
- `mouse_filter`

Anchors:

```text
c center
l left
r right
t top
b bottom
tl top_left
tr top_right
bl bottom_left
br bottom_right
```

Default anchor is `center`.
Default position is `position_ratio = (0.5, 0.5)`.
Default `h_align` and `v_align` are `center`.
Default label text align is `center`.
`corner_radius = "full"` makes the radius half of the shortest side.

## Coordinate Space

UI space uses center origin.
Top-level UI nodes use the virtual viewport as parent.
Children use parent UI rect as parent.

`position_ratio = (0.5, 0.5)` means no offset from the anchor.
`pivot_ratio = (0.5, 0.5)` means pivot at node center.
`translation` applies after anchor / position / pivot resolve.
`scale` multiplies final clamped size.
`h_size` and `v_size` accept `fixed`, `fill`, or `fit_children`.
`h_align` accepts `start`, `center`, `end`, or `fill`.
`v_align` accepts `start`, `center`, `end`, or `fill`.
`min_size`, `max_size`, `min_w`, `min_h`, `max_w`, and `max_h` are pixel clamps after size resolve.

Example:

```text
[menu]
[UiBox]
    anchor = "tr"
    size = (300, 200)
    padding = 12
[/UiBox]
[/menu]
```

## Current Runtime Scope

Done:

- anchor / pivot / translation / size resolve through parent chain
- padding as child content inset
- margin as child outer inset
- H/V/Grid child placement
- H/V/Grid alignment
- Fill / FitChildren
- approximate text measure
- UI render commands are emitted for panel/button/label
- egui-style screen rect conversion exists in render bridge
- egui tessellation path draws panel/button/label primitives
- font atlas upload + mesh draw path exists in graphics backend

Not done:

- exact font/glyph text measure
- hit test / focus / clicks
