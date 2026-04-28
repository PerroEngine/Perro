# UI Nodes

UI nodes are data-only scene nodes backed by the `perro_ui` crate.
They are registered in `perro_nodes` with `UiRoot` as their base node type.

## Registry Tree

```text
UiRoot
- UiPanel
- UiButton
- UiLabel
- UiHBox
- UiVBox
- UiGrid
```

## Coordinate Space

UI layout resolves against the parent UI rect.
Top level UI nodes use the virtual viewport as parent.

`UiLayout::position` and `UiLayout::size` use `UiVector2`.
Each axis can be pixels or percent:

```rust
let center = UiVector2::percent(50.0, 50.0);
let fixed = UiVector2::pixels(320.0, 64.0);
let mixed = UiVector2::new(UiUnit::px(24.0), UiUnit::pct(50.0));
```

For a `1920 x 1080` virtual viewport:

```text
UiVector2::percent(50.0, 50.0) -> Vector2::new(960.0, 540.0)
```

For a child inside a `400 x 200` panel:

```text
UiVector2::percent(50.0, 50.0) -> Vector2::new(200.0, 100.0)
```

Default UI origin is centered.
`UiLayout::position` defaults to `50%, 50%`.
`UiLayout::pivot` defaults to `50%, 50%`.
So a node with no position change sits at parent center.

Use `UiLayout::translation` for a pixel bump after parent-space position is resolved.
This lets container children offset themselves without changing their slot.

`padding` belongs to the parent content area.
`margin` belongs to the child outer area.

## Core Types

`UiRoot`

- Base UI state.
- Holds `layout`, `visible`, `input_enabled`, and `mouse_filter`.
- Sibling base to `Node2D` and `Node3D`.

`UiPanel`

- Rect visual.
- Holds a `UiStyle`.
- Derefs to `UiRoot`.

`UiButton`

- Pressable rect + text state.
- Holds normal, hover, and pressed styles.
- Derefs to `UiRoot`.

`UiLabel`

- Text visual.
- Holds `text`, `color`, `font_size`, and text alignment.
- Derefs to `UiRoot`.

`UiHBox`

- Lays children left to right.
- Derefs to `UiRoot`.

`UiVBox`

- Lays children top to bottom.
- Derefs to `UiRoot`.

`UiGrid`

- Lays children into a grid.
- Uses `columns`, `h_spacing`, and `v_spacing`.
- Derefs to `UiRoot`.

All UI nodes can have children.
Containers only add automatic child placement.
`UiPanel`, `UiButton`, and `UiLabel` can still hold children; their children use the node rect as parent space.

## Current Scope

The first implementation adds crate types and node registry support.
Layout pass, hit testing, text shaping, and render submission are next runtime layers.
