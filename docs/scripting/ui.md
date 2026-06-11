# UI Nodes

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `UI Nodes` when this feature, type group, file format, or workflow appears in game code or assets.

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

# UI Nodes

UI nodes are data-only scene nodes backed by `perro_ui`.
They use `UiBox` as their base node type.

## Tree

```text
UiBox
- UiPanel
- UiButton
- UiImage
- UiImageButton
- UiNineSlice
- UiAnimatedImage
- UiLabel
- UiScrollContainer
- UiLayout
- UiHLayout
- UiVLayout
- UiGrid
- UiList
- UiListIndent
```

## Nodes

`UiBox`

- Invisible UI container.
- Holds `layout`, `visible`, `input_enabled`, and `mouse_filter`.
- Holds `modulate`, `self_modulate`, and `children_modulate` RGBA multipliers like `Node2D`.
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
- Emits default `<node_name>_<event>` signals plus extra event signal lists.

`UiImage`

- Drawn image node.
- Holds `texture`, `texture_region`, `tint`, `scale_mode`, alignment, and `aspect_ratio`.
- Use it for icon, portrait, inventory, and image-heavy UI.

`UiImageButton`

- Clickable image node.
- Holds image fields plus button input, hover, pressed, and extra click signal fields.
- `hover` and `pressed` can override layout / transform fields and `tint`.
- Emits default `<node_name>_<event>` signals plus extra event signal lists.
- Use it for icon buttons, irregular-looking buttons, HUD slots, and image-only controls.

`UiNineSlice`

- Scalable image panel.
- Holds `texture`, `texture_region`, `margins`, and `tint`.
- Keeps corners fixed, stretches edges, and stretches center.
- Use it as a child/background for UI containers and buttons.

`UiAnimatedImage`

- Animated image node for UI space.
- Holds `texture`, named sprite-sheet `animations`, playback fields, `tint`, `scale_mode`, alignment, and `aspect_ratio`.
- Uses same strip/grid animation data shape as `AnimatedSprite2D`.
- Use it for animated icons, portraits, cooldowns, indicators, and HUD effects.

`UiLabel`

- Text visual.
- Holds `text`, `color`, `text_size_ratio`, and text alignment.
- Can have children, but usually should not.

`UiScrollContainer`

- Invisible clipped UI container with scroll offset.
- Holds `scroll` / `scroll_x` / `scroll_y`.
- Offsets child content and clips descendants to its rect.
- Wheel targets hovered scroller.
- Keyboard targets focused-node ancestor scroller or sole visible root scroller.
- Key map: `ArrowUp`, `ArrowDown`, `PageUp`, `PageDown`, `Home`, `End`.
- Current v1 target = desktop wheel + keyboard.
- Touch / drag scroll path defer.

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

`UiList`

- Invisible vertical list layout container.
- Direct child UI nodes become rows and get list spacing.
- Grandchildren of a row are not list rows.
- `UiListIndent` does not render as a row.
- Direct children of `UiListIndent` become rows at one deeper indent.
- Uses `indent` and `v_spacing`.

`UiListIndent`

- Invisible grouping marker inside `UiList`.
- Its children render one indent level deeper.

Normal list:

```text
[rows]
    [UiList]
        size_ratio = (1.0, 1.0)
        v_spacing = 0.006
    [/UiList]
[/rows]

[row_a]
parent = @rows
    [UiPanel]
        size_ratio = (1.0, 0.08)
    [/UiPanel]
[/row_a]

[row_a_label]
parent = @row_a
    [UiLabel]
        size_ratio = (1.0, 1.0)
        text = "Row A child content"
    [/UiLabel]
[/row_a_label]

[row_b]
parent = @rows
    [UiPanel]
        size_ratio = (1.0, 0.08)
    [/UiPanel]
[/row_b]
```

Tree-like list:

```text
[scene_rows]
    [UiList]
        size_ratio = (1.0, 1.0)
        indent = 18.0
        v_spacing = 0.006
    [/UiList]
[/scene_rows]

[node3d_row]
parent = @scene_rows
    [UiPanel]
        size_ratio = (1.0, 0.075)
    [/UiPanel]
[/node3d_row]

[node3d_label]
parent = @node3d_row
    [UiLabel]
        size_ratio = (1.0, 1.0)
        text = "Node3D"
    [/UiLabel]
[/node3d_label]

[node3d_children]
parent = @scene_rows
    [UiListIndent]
    [/UiListIndent]
[/node3d_children]

[mesh_row]
parent = @node3d_children
    [UiPanel]
        size_ratio = (1.0, 0.075)
    [/UiPanel]
[/mesh_row]
```

## Layout Fields

Common fields live on `UiBox` data and all UI nodes inherit them:

- `anchor`
- `size_percent`
- `size_ratio`
- `pivot_percent`
- `pivot_ratio`
- `translation_percent`
- `translation_ratio`
- `self_translation_percent`
- `self_translation_ratio`
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
- `clip_children`

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
Default `translation_ratio` and `self_translation_ratio` are `(0.0, 0.0)`.
Default `h_align` and `v_align` are `center`.
Default label text align is `center`.
Default `clip_children` is `false` (children may render outside parent bounds).
Set `clip_children = true` to clip descendants to that node rect.
`corner_radius` and `radius` use `0.0..1.0`.
`0.0` means square corners.
`1.0` means half of the shortest side.
`"full"` also means `1.0`.
`shadow` and `highlight` add depth to `UiPanel`, `UiButton`, `UiTextBox`, and `UiTextBlock`.
Both accept `color`, `distance`, `falloff`, `vector`, and `size`.
`shadow` draws outside the panel.
`highlight` draws an inner bevel-like stroke.
`vector` is the light/shadow direction in UI space.
`falloff` approximates blur with layered alpha.
`size` is relative to the panel/button size.
`size = 1` matches the panel/button.
`size = 2` doubles it.
`size = 0.5` halves it.

## Layout Mental Model

UI layout always resolves from a parent rect.

- Root UI node parent = virtual viewport.
- Child UI node parent = closest UI ancestor.
- Non-UI wrapper nodes do not create UI layout space.
- `visible = false` hides the node and its UI descendants.
- Showing the parent again makes descendants render on the next UI extract.
- `create_node!`, `create_nodes!`, and `reparent!` mark UI layout/render dirty.

Use `force_rerender!` only when code bypasses the normal mutation APIs and edits hidden/visible state directly.
Normal `with_node_mut!`, `with_base_node_mut!`, `create_node!`, `create_nodes!`, and `reparent!` calls do not need it.

## Ratio Guide

Scene UI uses ratio/percent fields.
Think parent first, then node.

- `size_ratio = (0.5, 0.25)` => node size = 50% parent width and 25% parent height.
- `pivot_ratio = (0.5, 0.5)` => pivot at node center.
- `pivot_ratio = (0.0, 1.0)` => pivot at node top-left.
- `translation_ratio = (1.0, 0.0)` => move right by one parent width after layout.
- `translation_ratio = (0.0, 1.0)` => move up by one parent height after layout.
- `translation_ratio = (0.0, -0.5)` => move down by half parent height after layout.
- `self_translation_ratio = (1.0, 0.0)` => move right by one own width after size resolves.
- `self_translation_ratio = (0.0, 1.0)` => move up by one own height after size resolves.

Anchor pins the matching node edge/corner/center to the parent anchor.
Pivot chooses rotate/scale origin inside that already placed node.
Pivot does not choose where the node is placed.
Translation moves after layout by parent size.
Self translation moves after layout by node size.
Scene `position_ratio`, `position_percent`, and `position_pct` are ignored legacy fields.

Common anchor results:

- `anchor = "center"` + no translation => centered.
- `anchor = "tr"` + no translation => node top-right corner sits on parent top-right corner.
- `anchor = "bl"` + no translation => node bottom-left corner sits on parent bottom-left corner.
- `anchor = "b"` + no translation => node bottom edge sits on parent bottom edge.
- `anchor = "top"` + `self_translation_ratio = (0.0, -0.5)` => move down by half node height after top edge placement.

Pivot example:

- `anchor = "b"` + `pivot_ratio = (0.5, 0.5)` + node height `100` => pivot is 50 above parent bottom.
- `anchor = "b"` + `pivot_ratio = (0.5, 1.0)` + node height `100` => pivot is 100 above parent bottom.
- In both cases, no translation means the node bottom edge stays on the parent bottom edge.

## Anchor Placement

Use one of 9 anchors for base placement.
Then use `translation_ratio = (x, y)` for parent-space movement after layout.
Use `self_translation_ratio = (x, y)` for own-size movement after layout.
`x > 0` moves right.
`x < 0` moves left.
`y > 0` moves up.
`y < 0` moves down.

```text
tl  t  tr
l   c  r
bl  b  br
```

Example horizontal placement:

- `anchor = "c"` + `translation_ratio = (0.25, 0.0)` reaches midpoint between center and right edge.
- `anchor = "r"` + `translation_ratio = (-0.125, 0.0)` reaches the same point.

Example vertical placement:

- `anchor = "c"` + `translation_ratio = (0.0, 0.25)` reaches midpoint between center and top edge.
- `anchor = "t"` + `translation_ratio = (0.0, -0.125)` reaches the same point.

These pairs match because `translation_ratio` moves by the parent size.
If resolved node size changes, translation values that hit the same parent-space point stay stable.

## `.uistyle` Resources

Current inline style blocks remain the base schema:

```text
style = { fill = "#222" stroke = "#555" radius = 0.2 }
```

The same schema can load from `.uistyle`:

```text
style = "res://ui/panel.uistyle"
```

Button states should accept style resources:

```text
hover = { style = "res://ui/button_hover.uistyle" }
pressed = { style = "res://ui/button_down.uistyle" }
```

Text edit focus should accept:

```text
focused_style = "res://ui/input_focus.uistyle"
```

`.uistyle` is visual-only.
It mirrors `UiStyle` fields such as `fill`, `stroke`, `stroke_width`, `radius`, `shadow`, and `highlight`.
It does not define layout, classes, or global themes.

See [`.uistyle` Format](../resources/uistyle.md).

## Coordinate Space

UI space uses center origin.
Top-level UI nodes use the virtual viewport as parent.
Children use parent UI rect as parent.

`pivot_ratio = (0.5, 0.5)` means pivot at node center.
Pivot affects rotation/scale origin, not final anchor placement.
Anchor placement pins node edge/corner/center to the matching parent point before translation.
`translation_ratio = (x, y)` offsets by parent size.
`self_translation_ratio = (x, y)` offsets by own resolved size.
Example: `translation_ratio = (0.0, 0.5)` moves node up by half parent height.
Example: `self_translation_ratio = (-1.0, 0.0)` moves node left by one own width.
`scale` multiplies final clamped size.
`h_size` and `v_size` accept `fixed`, `fill`, or `fit_children`.
`h_align` accepts `start`, `center`, `end`, or `fill`.
`v_align` accepts `start`, `center`, `end`, or `fill`.
`size_ratio` always resolves against current parent size (or root viewport size), ensuring no matter the screen size, the game looks the same without thinking of absolute sizing.
`min_size_ratio` and `max_size_ratio` clamp against node baseline size.
Baseline size = node resolved size at spawn/creation time.
If size definition changes later (`size_ratio`, size mode), baseline rebases to new resolved size.
`min_size_ratio = (1.0, 1.0)` + `max_size_ratio = (1.0, 1.0)` locks node at spawn-relative size, since it can only be 100% of it's creation size.
`min_size_ratio = (0.8, 0.8)` + `max_size_ratio = (1.2, 1.2)` allows small dynamic scale band up and down for changing window size, but not so unruly that it compresses or enlarges.
Layout spacing keys are ratio-based:
`spacing`, `h_spacing` resolve against parent content width.
`v_spacing` resolves against parent content height.
Example: grid `size_ratio = (1, 1)` + `h_spacing = 0.25` => horizontal gap = 25% of container width.
Layout `padding` is ratio-based:
left/right resolve against own width.
top/bottom resolve against own height.
`padding = 0.5` uses half size on each side.
Absolute UI keys unsupported in scenes:
`position`, `pivot`, `translation`, `size`, `size_px`, `pixel_size`, `min_size`, `max_size`, `min_w`, `min_width`, `min_h`, `min_height`, `max_w`, `max_width`, `max_h`, `max_height`, `font_size`.
Legacy `position_ratio`, `position_percent`, and `position_pct` are accepted but ignored.
Use ratio/percent keys + `text_size_ratio`.

Example:

```text
[menu]
[UiBox]
    anchor = "tr"
    size_ratio = (0.15625, 0.185185)
    padding = 0.08
[/UiBox]
[/menu]
```

Full-screen root + centered panel:

```text
[ui_root]
[UiBox]
    anchor = "center"
    size_ratio = (1.0, 1.0)
    pivot_ratio = (0.5, 0.5)
[/UiBox]
[/ui_root]

[card]
parent = @ui_root
[UiPanel]
    anchor = "center"
    size_ratio = (0.45, 0.35)
    pivot_ratio = (0.5, 0.5)
    style = { fill = "#20242C" stroke = "#586070" radius = 0.12 }
[/UiPanel]
[/card]
```

Top-right HUD:

```text
[hud_stats]
[UiPanel]
    anchor = "tr"
    size_ratio = (0.18, 0.10)
    pivot_ratio = (0.5, 0.5)
    translation_ratio = (-0.15, -0.15)
    style = { fill = "#111827CC" stroke = "#93A4B8" radius = 0.15 }
[/UiPanel]
[/hud_stats]
```

Bottom-left button row:

```text
[quick_bar]
[UiHLayout]
    anchor = "bl"
    size_ratio = (0.28, 0.08)
    pivot_ratio = (0.5, 0.5)
    translation_ratio = (0.15, 0.15)
    h_spacing = 0.04
[/UiHLayout]
[/quick_bar]

[slot_1]
parent = @quick_bar
[UiButton]
    size_ratio = (0.22, 0.8)
    style = { fill = "#263238" stroke = "#90A4AE" radius = 0.18 }
[/UiButton]
[/slot_1]
```

Spawn UI from script:

```rust
methods!({
    fn spawn_toast(&self, ctx: &mut ScriptContext<'_, API>, parent: NodeID) {
        let panel = create_node!(ctx.run, UiPanel);

        let _ = with_node_mut!(ctx.run, UiPanel, panel, |node| {
            node.base.layout.anchor = UiAnchor::Top;
            node.base.layout.size = UiVector2::ratio(0.32, 0.08);
            node.base.transform.translation = Vector2::new(0.0, -0.75);
            node.style.fill = Color::new(0.05, 0.06, 0.08, 0.92);
        });

        let _ = reparent!(ctx.run, parent, panel);
    }
});
```

If low-level code edits UI data without `with_node_mut!`, force subtree extraction:

```rust
let _ = force_rerender!(ctx.run, ui_root);
```

Button state example:

```text
[play_button]
    [UiButton]
        size_ratio = (0.114583, 0.044444)
        # Extra signals. Default signals still emit:
        # play_button_pressed
        # play_button_clicked
        pressed_signals = ["play_down"]
        clicked_signals = ["play_clicked", "any_button_clicked"]
        style = {
            fill = "#344E41"
            stroke = "#A3B18A"
            radius = 0.3
            shadow = { color = "#00000066" distance = 10 falloff = 12 vector = (1, -1) size = 2 }
            highlight = { color = "#FFFFFF55" distance = 2 falloff = 3 vector = (-1, 1) size = 2 }
        }
    hover = {
        scale = (1.02, 1.02)
        rotation = 0.02
        style = { fill = "#3A5A40" stroke = "#DAD7CD" radius = 0.35 }
    }
    pressed = {
        scale = (0.98, 0.98)
        rotation = -0.01
        style = { fill = "#1B4332" stroke = "#95D5B2" radius = 0.25 }
    }
[/UiButton]
[/play_button]
```

Animated image example:

```text
[coin_icon]
[UiAnimatedImage]
    texture = "res://ui/coin_strip.png"
    size_ratio = (0.04, 0.07)
    scale_mode = "fit"
    animations = [
        { name = "spin" start = (0, 0) frame_size = (32, 32) frame_count = 8 columns = 8 fps = 12 },
    ]
    animation = "spin"
    playing = true
    looping = true
[/UiAnimatedImage]
[/coin_icon]
```

Image button example:

```text
[play_icon]
[UiImageButton]
    texture = "res://ui/play.png"
    size_ratio = (0.08, 0.08)
    scale_mode = "fit"
    # Extra signal. Default play_icon_clicked still emits.
    clicked_signals = ["play_clicked"]
    hover = { scale = (1.06, 1.06) tint = "#FFFFFFFF" }
    pressed = { scale = (0.94, 0.94) tint = "#CCCCCCFF" }
[/UiImageButton]
[/play_icon]
```

Nine-slice example:

```text
[dialog_panel]
[UiNineSlice]
    texture = "res://ui/panel.png"
    texture_region = (0, 0, 64, 64)
    margins = (8, 8, 8, 8)
    size_ratio = (0.45, 0.30)
    tint = "#FFFFFFFF"
[/UiNineSlice]
[/dialog_panel]
```

`margins` are left, top, right, bottom pixels inside the source texture or region.
Corners keep their source size.
Edges and center stretch to fill the resolved UI rect.

Old flat button color fields still work:

```text
fill = "#344E41"
hover_fill = "#3A5A40"
pressed_fill = "#1B4332"
```

`UiButton`, `UiImageButton`, `Button2D`, and `ImageButton2D` emit these events:

```text
hover_enter
hover_exit
pressed
released
clicked
```

All button nodes use pointer cursor by default while hovered.
Set `cursor_icon` or `hover_cursor_icon` to override it.
Use `"pointer"` for hand pointer.
`"hand"` is accepted as an alias.

Each event always emits its default named signal:

```text
<button_node_name>_hover_enter
<button_node_name>_hover_exit
<button_node_name>_pressed
<button_node_name>_released
<button_node_name>_clicked
```

This is the `NAME_ACTION` rule.
Node `play_button` click emits `play_button_clicked` even when `clicked_signals` is empty.

Custom signal fields are extra signals.
They add signals on top of the default named signal:

```text
hover_signals = ["play_hover"]
hover_exit_signals = ["play_unhover"]
pressed_signals = ["play_down"]
released_signals = ["play_up"]
clicked_signals = ["play_clicked", "any_button_clicked"]
```

All handlers receive `(button: NodeID)`.
Keyboard/controller focus uses the same button hover/press visual states and events.
`Tab` moves focus forward.
`Shift+Tab` moves focus backward.
Gamepad D-pad and left stick move focus toward the nearest control in that direction.
Joy-Con stick uses the same directional focus path.
`Enter`, `Space`, gamepad bottom face button, and Joy-Con right face button activate the focused button.
Buttons and text edits can filter focus/activation input by player or device id.
Use `input_only_*`/`input_allow_*` fields for allow lists.
Use `input_block_*`/`input_deny_*` fields for deny lists.
Deny wins if the same source matches both.
If any allow list is set, unmatched sources are ignored.

```text
input_only_players = [0]
input_block_gamepads = [1]
input_only_joycons = [0, 1]
input_allow_kbm = true
input_deny_kbm = true
```

Full scene mask examples:

```text
[p1_start]
[UiButton]
    text = "Start"
    input_only_players = [0]
    input_block_gamepads = [1]
[/UiButton]
[/p1_start]

[joycon_name]
[UiTextBox]
    input_only_joycons = [0, 1]
    input_block_players = [2]
[/UiTextBox]
[/joycon_name]

[keyboard_help]
[UiTextBlock]
    text = "Press Space"
    input_allow_kbm = true
    input_deny_gamepads = [0, 1, 2, 3]
[/UiTextBlock]
[/keyboard_help]
```

Runtime script mask example:

```rust
let _ = with_node_mut!(ctx.run, UiButton, play_button, |button| {
    button.input_mask.allow_players = vec![0];
    button.input_mask.deny_gamepads = vec![1];
});

let _ = with_node_mut!(ctx.run, UiTextBox, name_field, |field| {
    field.inner.input_mask.allow_joycons = vec![0, 1];
    field.inner.input_mask.deny_players = vec![2];
});

let _ = with_node_mut!(ctx.run, UiTextBlock, help_text, |text| {
    text.inner.input_mask.allow_kbm = true;
});
```

Scene example:

```text
[play_button]
[UiButton]
    size_ratio = (0.114583, 0.044444)
    # Extra signals only. Default play_button_* signals still emit.
    hover_signals = ["menu_button_hover"]
    pressed_signals = ["play_down", "any_button_down"]
    released_signals = ["play_up"]
    clicked_signals = ["play_clicked", "any_button_clicked"]
[/UiButton]
[/play_button]
```

Connect to named and custom signals:

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect_many!(
            ctx.run,
            ctx.id,
            [
                signal!("play_button_hover_enter"),
                signal!("play_button_hover_exit"),
                signal!("play_button_pressed"),
                signal!("play_button_released"),
                signal!("play_button_clicked"),
                signal!("menu_button_hover"),
                signal!("play_down"),
                signal!("any_button_down"),
                signal!("play_up"),
                signal!("play_clicked"),
                signal!("any_button_clicked"),
            ],
            [func!("on_button")]
        );
    }
});

methods!({
    fn on_button(&self, ctx: &mut ScriptContext<'_, API>, button: NodeID) {
        println!("button={button:?}");
    }
});
```

Runtime add/remove custom emits:

```rust
let _ = with_node_mut!(ctx.run, UiButton, play_button, |button| {
    let sig = signal!("debug_play_click");
    if !button.clicked_signals.contains(&sig) {
        button.clicked_signals.push(sig);
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
