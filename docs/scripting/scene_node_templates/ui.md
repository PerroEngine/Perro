# UI Scene Node Templates

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `UI Scene Node Templates` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# UI Scene Node Templates

[Back to index](index.md)

## UI Templates

`UiHBox` and `UiVBox` also work as aliases for `UiHLayout` and `UiVLayout`.
`hover` and `pressed` on `UiButton` accept any `UiBox` field plus style fields.
`.uistyle` resources let `style`, `hover.style`, `pressed.style`, and `focused_style` use `res://path/to/style.uistyle`.

UI templates use ratio-only sizing.

- `size_ratio` = size relative to parent.
- `pivot_ratio = (0.5, 0.5)` = rotate/scale origin at node center.
- `translation_ratio` = move after layout by parent size.
- `self_translation_ratio` = move after layout by own resolved size.
- `padding` = inner child layout inset by own size ratio.
- `min_size_ratio` + `max_size_ratio` clamp relative to node base size at creation.
- Example: `size_ratio = (0.5, 0.5)` => half parent size.
- Example: `anchor = "tr"` => node top-right corner sits on parent top-right corner.
- Example: `anchor = "b"` => node bottom edge sits on parent bottom edge.
- Example: `translation_ratio = (-0.2, -0.2)` => move left/down by 20% parent size.
- Example: `self_translation_ratio = (-0.2, -0.2)` => move left/down by 20% own size.
- Example: `padding = 0.1` => inset each side by 10% own width/height.
- Example: `min_size_ratio = (1.0, 1.0)` => never shrink below creation size.
- Example: `min_size_ratio = (0.8, 0.8)` + `max_size_ratio = (1.2, 1.2)` => allow ~20% shrink/grow band.

Root UI nodes resolve against the virtual viewport.
Child UI nodes resolve against the closest UI ancestor.
Non-UI wrappers do not define UI layout size.
`visible = false` hides the UI subtree.
Showing the parent makes descendants render on the next UI extract.
`position_ratio`, `position_percent`, and `position_pct` are ignored legacy fields.

Common mistakes:

- Do not use `size`, `position`, `pivot`, or `translation` in scene UI.
- Use `size_ratio`, `pivot_ratio`, `translation_ratio`, and `self_translation_ratio`.
- Do not use `position_ratio` for UI placement.
- Use `anchor = "tl"` for top-left anchoring.
- Do not call `force_rerender` after normal scene/runtime APIs.
- Use `force_rerender` only after raw/manual data edits outside normal mutation APIs.

Anchor map:

```text
tl  t  tr
l   c  r
bl  b  br
```

`translation_ratio = (x, y)` moves after anchor placement by parent size.
`self_translation_ratio = (x, y)` moves after anchor placement by own size.
`anchor` pins the matching node edge/corner/center to the parent anchor.
`pivot_ratio` sets rotate/scale origin inside that placed node.
`pivot_ratio` does not move anchor placement.
Positive X moves right.
Positive Y moves up.
For `anchor = "b"` and node height `100`, pivot Y `0.5` is 50 above parent bottom, and pivot Y `1.0` is 100 above parent bottom.
`anchor = "c"` + `translation_ratio = (0.25, 0)` matches `anchor = "r"` + `translation_ratio = (-0.125, 0)`.
`anchor = "c"` + `translation_ratio = (0, 0.25)` matches `anchor = "t"` + `translation_ratio = (0, -0.125)`.

```text
[ui_box]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiBox]
        visible = true
        input_enabled = true
        mouse_filter = "stop"
        clip_children = false
        anchor = "center"
        size_ratio = (0.5, 0.5)
        pivot_ratio = (0.5, 0.5)

        scale = (1, 1)
        rotation = 0.0
        h_size = "fixed"
        v_size = "fixed"
        h_align = "center"
        v_align = "center"
        min_size_ratio = (1.0, 1.0)
        max_size_ratio = (inf, inf)
        padding = 0
        margin = 0
        z_index = 0
    [/UiBox]
[/ui_box]

[ui_panel]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiPanel]
        fill = (0.11, 0.12, 0.14, 0.92)
        stroke = (0.22, 0.24, 0.28, 1.0)
        stroke_width = 1.0
        radius = 0.2
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiPanel]
[/ui_panel]

[ui_button]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiButton]
        disabled = false
        cursor_icon = "pointer"
        hover_signals = []
        hover_exit_signals = []
        pressed_signals = []
        released_signals = []
        click_signals = []
        style = { fill = (0.18, 0.20, 0.24, 1.0) stroke = (0.32, 0.35, 0.40, 1.0) stroke_width = 1.0 radius = 0.2 shadow = { color = (0, 0, 0, 0) distance = 0 falloff = 0 vector = (0, -1) size = 1 } highlight = { color = (0, 0, 0, 0) distance = 0 falloff = 0 vector = (0, -1) size = 1 } }
        # Planned 1.0 alternative:
        # style = "res://ui/button.uistyle"
        hover = {
            style = { fill = (0.24, 0.27, 0.32, 1.0) stroke = (0.42, 0.46, 0.54, 1.0) stroke_width = 1.0 radius = 0.2 }
            # Planned 1.0 alternative:
            # style = "res://ui/button_hover.uistyle"
        }
        pressed = {
            style = { fill = (0.12, 0.14, 0.18, 1.0) stroke = (0.42, 0.46, 0.54, 1.0) stroke_width = 1.0 radius = 0.2 }
            # Planned 1.0 alternative:
            # style = "res://ui/button_down.uistyle"
        }
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiButton]
[/ui_button]

[ui_image_button]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiImageButton]
        texture = "res://ui/icon.png"
        texture_region = (0, 0, 32, 32)
        tint = (1, 1, 1, 1)
        hover_tint = (1, 1, 1, 1)
        pressed_tint = (0.8, 0.8, 0.8, 1)
        scale_mode = "fit"
        aspect_ratio = 0.0
        disabled = false
        cursor_icon = "pointer"
        click_signals = []
        hover = { scale = (1.06, 1.06) tint = (1, 1, 1, 1) }
        pressed = { scale = (0.94, 0.94) tint = (0.8, 0.8, 0.8, 1) }
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            anchor = "center"
            size_ratio = (0.1, 0.1)
            pivot_ratio = (0.5, 0.5)
        [/UiBox]
    [/UiImageButton]
[/ui_image_button]

[ui_nine_slice]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiNineSlice]
        texture = "res://ui/panel.png"
        texture_region = (0, 0, 64, 64)
        margins = (8, 8, 8, 8)
        tint = (1, 1, 1, 1)
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "ignore"
            anchor = "center"
            size_ratio = (0.2, 0.1)
        [/UiBox]
    [/UiNineSlice]
[/ui_nine_slice]

[ui_label]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiLabel]
        text = ""
        color = (1, 1, 1, 1)
        text_size_ratio = 0.5
        font_relative = false
        font_min_scale = 0.0
        font_max_scale = inf
        text_h_align = "center"
        text_v_align = "center"
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiLabel]
[/ui_label]

[ui_camera_stream]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiCameraStream]
        camera = @CameraNode
        resolution = (512, 512)
        aspect_ratio = 0.0
        aspect_mode = "fit"
        enabled = true
        tint = (1, 1, 1, 1)
        post_processing = []
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.25, 0.25)
            pivot_ratio = (0.5, 0.5)
            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiCameraStream]
[/ui_camera_stream]

[ui_text_box]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTextBox]
        text = ""
        placeholder = ""
        color = (1, 1, 1, 1)
        placeholder_color = (0.58, 0.62, 0.70, 1.0)
        selection_color = (0.25, 0.42, 0.85, 0.55)
        caret_color = (1, 1, 1, 1)
        text_size_ratio = 0.5
        font_relative = false
        font_min_scale = 0.0
        font_max_scale = inf
        text_padding = { left = 8 top = 6 right = 8 bottom = 6 }
        editable = true
        hover_signals = []
        hover_exit_signals = []
        focused_signals = []
        unfocused_signals = []
        text_changed_signals = []
        style = { fill = (0.11, 0.12, 0.14, 0.92) stroke = (0.22, 0.24, 0.28, 1.0) stroke_width = 1.0 radius = 0.2 }
        focused_style = { fill = (0.10, 0.11, 0.13, 0.96) stroke = (0.45, 0.58, 0.85, 1.0) stroke_width = 1.0 radius = 0.2 }
        # Planned 1.0 alternatives:
        # style = "res://ui/text_box.uistyle"
        # focused_style = "res://ui/text_box_focus.uistyle"
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiTextBox]
[/ui_text_box]

[ui_text_block]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTextBlock]
        text = ""
        placeholder = ""
        color = (1, 1, 1, 1)
        placeholder_color = (0.58, 0.62, 0.70, 1.0)
        selection_color = (0.25, 0.42, 0.85, 0.55)
        caret_color = (1, 1, 1, 1)
        text_size_ratio = 0.5
        font_relative = false
        font_min_scale = 0.0
        font_max_scale = inf
        text_padding = { left = 8 top = 6 right = 8 bottom = 6 }
        editable = true
        hover_signals = []
        hover_exit_signals = []
        focused_signals = []
        unfocused_signals = []
        text_changed_signals = []
        style = { fill = (0.11, 0.12, 0.14, 0.92) stroke = (0.22, 0.24, 0.28, 1.0) stroke_width = 1.0 radius = 0.2 }
        focused_style = { fill = (0.10, 0.11, 0.13, 0.96) stroke = (0.45, 0.58, 0.85, 1.0) stroke_width = 1.0 radius = 0.2 }
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiTextBlock]
[/ui_text_block]

[ui_layout]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiLayout]
        mode = "h"
        spacing = 0.0
        h_spacing = 0.0
        v_spacing = 0.0
        columns = 1
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiLayout]
[/ui_layout]

[ui_hlayout]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiHLayout]
        spacing = 0.0
        h_spacing = 0.0
        v_spacing = 0.0
        columns = 1
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiHLayout]
[/ui_hlayout]

[ui_vlayout]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiVLayout]
        spacing = 0.0
        h_spacing = 0.0
        v_spacing = 0.0
        columns = 1
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiVLayout]
[/ui_vlayout]

[ui_grid]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiGrid]
        columns = 1
        h_spacing = 0.0
        v_spacing = 0.0
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiGrid]
[/ui_grid]

[ui_tree_list]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTreeList]
        # roots, branches, and collapsed are usually set from script with NodeID values.
        indent = 16.0
        v_spacing = 0.0
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "left"
            v_align = "top"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiTreeList]
[/ui_tree_list]
```
