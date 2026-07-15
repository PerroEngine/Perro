# UI `.scn` Node Fields

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use this page as a copy-paste field reference for UI nodes in `.scn` scene files.

Use [Node Collections](../node_collections.md) for runtime Rust-built node trees.

## Use Cases

- Author HUDs, menus, and dialogs directly in `.scn`.
- Look up the full field list and defaults for one UI node type.
- Copy a template block, then trim fields you do not set.
- Keep scene field names and value forms consistent across a project.

## Example

A minimal panel with a centered button:

```text
[panel]
[UiPanel]
    anchor = "center"
    size_ratio = (0.4, 0.25)
    style = { fill = "#20242C" stroke = "#586070" radius = 0.12 }
[/UiPanel]
[/panel]

[ok_button]
parent = @panel
[UiButton]
    size_ratio = (0.5, 0.3)
    clicked_signals = ["ok_clicked"]
[/UiButton]
[/ok_button]
```

## Reference

[Back to index](index.md)

## UI Templates

`UiHBox` and `UiVBox` also work as aliases for `UiHLayout` and `UiVLayout`.
`hover` and `pressed` on `UiButton` accept any `UiNode` field plus style fields.
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
[ui_node]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiNode]
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
    [/UiNode]
[/ui_node]

[ui_panel]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiPanel]
        style = {
            fill_kind = "linear"
            fill = (0.11, 0.12, 0.14, 0.92)
            gradient = { start_color = (0.16, 0.18, 0.23, 0.98) end_color = (0.08, 0.09, 0.12, 0.98) vector = (0, -1) }
            stroke = (0.22, 0.24, 0.28, 1.0)
            stroke_width = 1.0
            corner_radii = (0.28, 0.28, 0.14, 0.14)
            outer_shadow = { color = (0, 0, 0, 0.35) distance = 8 falloff = 12 vector = (1, -1) size = 1.4 }
            inner_highlight = { color = (1, 1, 1, 0.16) distance = 1 falloff = 3 vector = (-1, 1) size = 1 }
        }
        [UiNode]
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
        [/UiNode]
    [/UiPanel]
[/ui_panel]

[ui_shape]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiShape]
        # shape: "rect" | "circle" | "triangle"
        shape = "rect"
        fill = (1, 1, 1, 1)
        stroke = (0, 0, 0, 0)
        stroke_width = 0.0
        [UiNode]
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
        [/UiNode]
    [/UiShape]
[/ui_shape]

[ui_button]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiButton]
        disabled = false
        # Default is "pointer"; override with any supported cursor icon.
        cursor_icon = "pointer"
        # Extra signals only. Default ui_button_<event> signals still emit.
        hover_signals = []
        hover_exit_signals = []
        pressed_signals = []
        released_signals = []
        clicked_signals = []
        style = {
            fill_kind = "linear"
            fill = (0.18, 0.20, 0.24, 1.0)
            gradient = { start_color = (0.28, 0.31, 0.37, 1.0) end_color = (0.12, 0.14, 0.18, 1.0) vector = (0, -1) }
            stroke = (0.32, 0.35, 0.40, 1.0)
            stroke_width = 1.0
            corner_radii = (0.45, 0.45, 0.45, 0.45)
            outer_shadow = { color = (0, 0, 0, 0.28) distance = 6 falloff = 10 vector = (1, -1) size = 1.2 }
            inner_shadow = { color = (0, 0, 0, 0.18) distance = 2 falloff = 5 vector = (0, -1) size = 1 }
            inner_highlight = { color = (1, 1, 1, 0.18) distance = 1 falloff = 3 vector = (-1, 1) size = 1 }
        }
        # Or load the style from a resource:
        # style = "res://ui/button.uistyle"
        hover = {
            style = { fill_kind = "linear" fill = (0.24, 0.27, 0.32, 1.0) gradient = { start_color = (0.30, 0.34, 0.40, 1.0) end_color = (0.18, 0.21, 0.27, 1.0) vector = (0, -1) } stroke = (0.42, 0.46, 0.54, 1.0) stroke_width = 1.0 radius = 0.45 }
            # Or: style = "res://ui/button_hover.uistyle"
        }
        pressed = {
            style = { fill_kind = "linear" fill = (0.12, 0.14, 0.18, 1.0) gradient = { start_color = (0.10, 0.12, 0.16, 1.0) end_color = (0.18, 0.20, 0.25, 1.0) vector = (0, 1) } stroke = (0.42, 0.46, 0.54, 1.0) stroke_width = 1.0 radius = 0.45 }
            # Or: style = "res://ui/button_down.uistyle"
        }
        [UiNode]
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
        [/UiNode]
    [/UiButton]
[/ui_button]

[ui_checkbox]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiCheckbox]
        # Also takes every [UiButton] field (disabled, cursor_icon, hover, pressed, *_signals).
        checked = false
        dot_fill = (1, 1, 1, 1)
        clicked_signals = []
        style = { fill = (0.18, 0.20, 0.24, 1.0) stroke = (0.32, 0.35, 0.40, 1.0) stroke_width = 1.0 radius = 0.3 }
        checked_style = { fill = (0.20, 0.40, 0.28, 1.0) stroke = (0.40, 0.70, 0.50, 1.0) stroke_width = 1.0 radius = 0.3 }
        [UiNode]
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
        [/UiNode]
    [/UiCheckbox]
[/ui_checkbox]

[ui_dropdown]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiDropdown]
        # Also takes every [UiButton] field.
        options = [
            { label = "Low" value = 0 },
            { label = "High" value = 1 },
        ]
        selected_index = 0
        open = false
        option_height = 28.0
        popup_size = (0, 0) # 0 = button width / option-list height
        popup_offset = (0, 0)
        popup_direction = "down" # down | up | left | right
        open_animation = "pop" # pop | extend
        open_animation_duration = 0.18
        selected_signals = []
        style = { fill = (0.18, 0.20, 0.24, 1.0) stroke = (0.32, 0.35, 0.40, 1.0) stroke_width = 1.0 radius = 0.2 }
        popup_style = { fill = (0.11, 0.12, 0.14, 0.98) stroke = (0.22, 0.24, 0.28, 1.0) stroke_width = 1.0 radius = 0.12 }
        option_style = { fill = (0.16, 0.18, 0.22, 1.0) stroke = (0.28, 0.30, 0.34, 1.0) stroke_width = 1.0 radius = 0.1 }
        [UiNode]
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
        [/UiNode]
    [/UiDropdown]
[/ui_dropdown]

[ui_color_picker]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiColorPicker]
        # Also takes every [UiButton] field.
        color = (1, 1, 1, 1)
        popup_open = false
        popup_size = (360, 344)
        wheel_radius = 88.0
        picker_mode = "smooth_wheel" # smooth_wheel | block_wheel | swatches
        show_selector = true
        show_hex = true
        show_rgba = true
        show_hsl = true
        color_changed_signals = []
        popup_style = { fill = (0.11, 0.12, 0.14, 0.98) stroke = (0.22, 0.24, 0.28, 1.0) stroke_width = 1.0 radius = 0.12 }
        [UiNode]
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
        [/UiNode]
    [/UiColorPicker]
[/ui_color_picker]

### `UiColorPicker` picker fields

| Field | Default | Values / behavior |
| --- | --- | --- |
| `picker_mode` | `"smooth_wheel"` | `smooth_wheel` = continuous hue/saturation; `block_wheel` = 12 hue bands and 4 saturation bands; `swatches` = fixed 6-by-4 palette |
| `show_selector` | `true` | Shows the wheel or swatch palette. When false, `picker_mode` has no visible effect. |
| `show_rgba` | `true` | Shows editable `R`, `G`, `B`, and `A` fields in the `0.0..1.0` range. |
| `show_hsl` | `true` | Shows editable hue in degrees and saturation/lightness in the `0.0..1.0` range. HSL edits preserve alpha. |
| `show_hex` | `true` | Shows `#RRGGBBAA`. Accepts `#RGB`, `#RGBA`, `#RRGGBB`, and `#RRGGBBAA`; forms without alpha preserve current alpha. |
| `popup_size` | `(360, 344)` | Minimum requested popup size. Width clamps to at least `340`; height grows to fit visible sections. |
| `wheel_radius` | `88.0` | Selector radius. Values clamp to at least `8.0`. |

Selector picks preserve current alpha. Wheel modes pick hue/saturation at full HSV value. RGBA, HSL, and hex fields refine the picked color.

Scene parser aliases:

- `picker_mode`: `wheel_type`, `selection_mode`
- smooth wheel values: `smooth`, `wheel`
- block wheel values: `block`, `blocky`
- swatch values: `swatch`, `palette`
- `show_selector`: `selector_visible`, `wheel_visible`
- `show_rgba`: `rgba_visible`
- `show_hsl`: `hsl_visible`
- `show_hex`: `hex_visible`

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
        # Default is "pointer"; override with any supported cursor icon.
        cursor_icon = "pointer"
        # Extra signals only. Default ui_image_button_clicked still emits.
        clicked_signals = []
        hover = { scale = (1.06, 1.06) tint = (1, 1, 1, 1) }
        pressed = { scale = (0.94, 0.94) tint = (0.8, 0.8, 0.8, 1) }
        [UiNode]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            anchor = "center"
            size_ratio = (0.1, 0.1)
            pivot_ratio = (0.5, 0.5)
        [/UiNode]
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
        [UiNode]
            visible = true
            input_enabled = true
            mouse_filter = "ignore"
            anchor = "center"
            size_ratio = (0.2, 0.1)
        [/UiNode]
    [/UiNineSlice]
[/ui_nine_slice]

[ui_image]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiImage]
        texture = "res://ui/icon.png"
        texture_region = (0, 0, 32, 32)
        tint = (1, 1, 1, 1)
        # scale_mode: "stretch" | "fit" | "cover"
        scale_mode = "stretch"
        h_align = "center"
        v_align = "center"
        aspect_ratio = 0.0
        [UiNode]
            visible = true
            input_enabled = true
            mouse_filter = "ignore"
            clip_children = false
            anchor = "center"
            size_ratio = (0.2, 0.2)
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
        [/UiNode]
    [/UiImage]
[/ui_image]

[ui_animated_image]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiAnimatedImage]
        texture = "res://ui/coin_strip.png"
        animations = [
            { name = "spin" start = (0, 0) frame_size = (32, 32) frame_count = 8 columns = 8 fps = 12 },
        ]
        animation = "spin"
        current_frame = 0
        fps_scale = 1.0
        playing = true
        looping = true
        tint = (1, 1, 1, 1)
        scale_mode = "fit"
        h_align = "center"
        v_align = "center"
        aspect_ratio = 0.0
        [UiNode]
            visible = true
            input_enabled = true
            mouse_filter = "ignore"
            clip_children = false
            anchor = "center"
            size_ratio = (0.1, 0.1)
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
        [/UiNode]
    [/UiAnimatedImage]
[/ui_animated_image]

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
        [UiNode]
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
        [/UiNode]
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
        [UiNode]
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
        [/UiNode]
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
        # Or load styles from resources:
        # style = "res://ui/text_box.uistyle"
        # focused_style = "res://ui/text_box_focus.uistyle"
        [UiNode]
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
        [/UiNode]
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
        [UiNode]
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
        [/UiNode]
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
        [UiNode]
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
        [/UiNode]
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
        [UiNode]
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
        [/UiNode]
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
        [UiNode]
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
        [/UiNode]
    [/UiVLayout]
[/ui_vlayout]

[ui_grid]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiGrid]
        columns = 1
        h_spacing = 0.0
        v_spacing = 0.0
        [UiNode]
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
        [/UiNode]
    [/UiGrid]
[/ui_grid]

[ui_scroll_container]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiScrollContainer]
        scroll = (0, 0)
        # scroll_dir: "vertical" | "horizontal"
        scroll_dir = "vertical"
        # scroll_bar_side: "left" | "right" | "top" | "bottom"
        scroll_bar_side = "right"
        # -1 uses the built-in default bar padding.
        scroll_bar_padding = -1.0
        [UiNode]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            # UiScrollContainer clips its children by default.
            clip_children = true
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
        [/UiNode]
    [/UiScrollContainer]
[/ui_scroll_container]

[ui_tree_list]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTreeList]
        indent = 18.0
        row_height = 24.0
        v_spacing = 0.0
        icon_size = 16.0
        toggle_size = 12.0
        selected_index = -1
        items = [
            { id = "root", label = "Root", open = true },
            { id = "child", label = "Child", parent = 0 },
        ]
        [UiNode]
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
        [/UiNode]
    [/UiTreeList]
[/ui_tree_list]

```
