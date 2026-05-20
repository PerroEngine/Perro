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
- `min_size_ratio` + `max_size_ratio` clamp relative to node base size at creation.
- Example: `size_ratio = (0.5, 0.5)` => half parent size.
- Example: `min_size_ratio = (1.0, 1.0)` => never shrink below creation size.
- Example: `min_size_ratio = (0.8, 0.8)` + `max_size_ratio = (1.2, 1.2)` => allow ~20% shrink/grow band.

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
        position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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

[ui_label]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiLabel]
        text = ""
        color = (1, 1, 1, 1)
        font_size = 16.0
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
        font_size = 16.0
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
            position_ratio = (0.5, 0.5)
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
        font_size = 16.0
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
            position_ratio = (0.5, 0.5)
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
