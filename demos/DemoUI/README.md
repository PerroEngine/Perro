# DemoUI

Perro UI feature demo + scene-wiring example.

Run:

```powershell
cargo run -p perro_cli -- dev --path demos/DemoUI
```

## Mental Model

The scene owns layout, widgets, fixed refs, and authored values. The showcase
script owns interaction and animation decisions. It receives scene-known
`NodeID` refs through state instead of finding widgets by name at runtime.

This keeps visual structure editable while Rust keeps behavior typed.

## Feature Map

- layout panels
- labels
- buttons
- image + image button
- text box + text block
- dropdown
- checkbox
- color picker
- shape
- generic layout
- grid
- scroll list
- tree list
- animated image
- webcam via `UiCameraStream`

Read [`res/main.scn`](res/main.scn) first to see widget topology and injected
refs. Read [`demo_ui_showcase.rs`](res/scripts/demo_ui_showcase.rs) second to
see typed state/node access and borrow-safe mutation. Run the demo last to see
the complete flow.

Script std: [`../../docs/scripting/authoring/index.md`](../../docs/scripting/authoring/index.md)

`demo_ui_showcase.rs` shows scene-known `NodeID` refs injected into state.

It copies state vals out before node access -> no nested runtime borrow.

## Why These Choices

- fixed widget targets -> state `NodeID`; layout edits do not require queries
- own script node -> `ctx.id`; no hidden self lookup
- UI state -> typed access; member names are known at compile time
- webcam output -> scene-owned node; capture remains a resource concern
- copied refs -> state borrow ends before node mutation

Use dynamic vars only for a tool/adapter whose member name arrives as data. Use
a signal when several UI elements react to one loose event. Use a method when
one known widget controller needs a targeted command or return value.
