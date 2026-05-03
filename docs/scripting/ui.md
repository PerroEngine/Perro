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
- UiTreeList
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

- Clickable panel.
- Holds normal, hover, and pressed styles.
- `hover` and `pressed` can override layout / transform / style fields.
- Add text, image, layouts, grids, or panels as children.
- Emits `<node_name>_<event>` plus custom event signal lists.

`UiLabel`

- Text visual.
- Holds `text`, `color`, `text_size_ratio`, and text alignment.
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

`UiTreeList`

- Invisible tree layout container.
- Places UI nodes by referenced `NodeID`.
- Referenced rows do not need to be scene children.
- Uses `roots`, `branches`, `collapsed`, `indent`, and `v_spacing`.
- `roots` are top-level row ids.
- `branches` map a row id to child row ids.
- `collapsed` hides child branches under a row id.

## Layout Fields

Common fields live on `UiBox` data and all UI nodes inherit them:

- `anchor`
- `position_percent`
- `position_ratio`
- `size_percent`
- `size_ratio`
- `pivot_percent`
- `pivot_ratio`
- `scale`
- `h_size`
- `v_size`
- `h_align`
- `v_align`
- `min_size_ratio`
- `max_size_ratio`
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
`scale` multiplies final clamped size.
`h_size` and `v_size` accept `fixed`, `fill`, or `fit_children`.
`h_align` accepts `start`, `center`, `end`, or `fill`.
`v_align` accepts `start`, `center`, `end`, or `fill`.
`min_size_ratio` and `max_size_ratio` clamp against virtual-derived base size.
Base size = size resolved from `size_ratio` against virtual viewport parent space.
`min_size_ratio = (1.0, 1.0)` means node never shrink below virtual-derived base size.
`max_size_ratio = (2.0, 2.0)` means node never grow above 2x virtual-derived base size.
Absolute UI keys unsupported in scenes:
`position`, `pivot`, `translation`, `size`, `min_size`, `max_size`, `min_w`, `min_h`, `max_w`, `max_h`, `font_size`.
Use ratio/percent keys + `text_size_ratio`.

Example:

```text
[menu]
[UiBox]
    anchor = "tr"
    size_ratio = (0.15625, 0.185185)
    padding = 12
[/UiBox]
[/menu]
```

Button state example:

```text
[play_button]
[UiButton]
    size_ratio = (0.114583, 0.044444)
    pressed_signals = ["play_down"]
    click_signals = ["play_clicked", "any_button_clicked"]
    style = { fill = "#344E41" stroke = "#A3B18A" radius = 8 }
    hover = {
        scale = (1.02, 1.02)
        rotation = 0.02
        style = { fill = "#3A5A40" stroke = "#DAD7CD" radius = 10 }
    }
    pressed = {
        scale = (0.98, 0.98)
        rotation = -0.01
        style = { fill = "#1B4332" stroke = "#95D5B2" radius = 6 }
    }
[/UiButton]
[/play_button]
```

Old flat button color fields still work:

```text
fill = "#344E41"
hover_fill = "#3A5A40"
pressed_fill = "#1B4332"
```

`UiButton` emits these events:

```text
hover_enter
hover_exit
pressed
released
click
```

Each event always emits its named signal:

```text
<button_node_name>_hover_enter
<button_node_name>_hover_exit
<button_node_name>_pressed
<button_node_name>_released
<button_node_name>_click
```

Custom signal fields add signals on top of named signals:

```text
hover_signals = ["play_hover"]
hover_exit_signals = ["play_unhover"]
pressed_signals = ["play_down"]
released_signals = ["play_up"]
click_signals = ["play_clicked", "any_button_clicked"]
```

All handlers receive `(button: NodeID)`.

Scene example:

```text
[play_button]
[UiButton]
    size_ratio = (0.114583, 0.044444)
    hover_signals = ["menu_button_hover"]
    pressed_signals = ["play_down", "any_button_down"]
    released_signals = ["play_up"]
    click_signals = ["play_clicked", "any_button_clicked"]
[/UiButton]
[/play_button]
```

Connect to named and custom signals:

```rust
lifecycle!({
    fn on_all_init(&self, ctx, res, ipt, self_id) {
        signal_connect!(ctx, self_id, signal!("play_button_hover_enter"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_button_hover_exit"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_button_pressed"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_button_released"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_button_click"), func!("on_button"));

        signal_connect!(ctx, self_id, signal!("menu_button_hover"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_down"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("any_button_down"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_up"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("play_clicked"), func!("on_button"));
        signal_connect!(ctx, self_id, signal!("any_button_clicked"), func!("on_button"));
    }
});

methods!({
    fn on_button(&self, ctx, res, ipt, self_id, button: NodeID) {
        println!("button={button:?}");
    }
});
```

Runtime add/remove custom emits:

```rust
let _ = with_node_mut!(ctx, UiButton, play_button, |button| {
    let sig = signal!("debug_play_click");
    if !button.click_signals.contains(&sig) {
        button.click_signals.push(sig);
    }

    button.pressed_signals.retain(|s| *s != signal!("old_press_signal"));
});
```

`UiTextBox` and `UiTextBlock` emit these events:

```text
hovered
unhovered
focused
unfocused
text_changed
```

Each event always emits its named signal:

```text
<text_node_name>_hovered
<text_node_name>_unhovered
<text_node_name>_focused
<text_node_name>_unfocused
<text_node_name>_text_changed
```

Custom signal fields add signals on top of named signals:

```text
hover_signals = ["name_hover"]
hover_exit_signals = ["name_unhover"]
focused_signals = ["name_focus"]
unfocused_signals = ["name_unfocus"]
text_changed_signals = ["name_changed"]
```

Hover/focus handlers receive `(text_edit: NodeID)`.
Text change handlers receive `(text_edit: NodeID, text: String)`.

Scene example:

```text
[name_input]
[UiTextBox]
    hover_signals = ["name_hover"]
    hover_exit_signals = ["name_unhover"]
    focused_signals = ["name_focus"]
    unfocused_signals = ["name_unfocus"]
    text_changed_signals = ["name_changed"]
[/UiTextBox]
[/name_input]

[bio_input]
[UiTextBlock]
    text_changed_signals = ["bio_changed"]
[/UiTextBlock]
[/bio_input]
```
