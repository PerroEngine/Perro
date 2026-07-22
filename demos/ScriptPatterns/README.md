# ScriptPatterns

Small verified project for Perro script ownership and communication.

## Feature Flow

```text
named timer -> controller -> call player method -> score reply
player -> score_changed signal -> HUD + audit
controller -> adapter method -> get/set runtime-selected player var
scene -> fixed NodeID refs + typed TextureID path
```

## Why Each Path Fits

- `NodeID` fields: scene owns fixed wiring.
- `TextureID`: scene chooses a per-instance asset before `on_init`.
- `call_method!`: controller targets one script and may use its reply.
- signal: player announces a fact to two independent listeners.
- `get_var!` / `set_var!`: adapter receives the member name at runtime.
- named timer: work occurs after a delay; no per-frame progress is needed.
- `with_node_mut!`: player and HUD know the concrete node types.
- short closures: values leave state borrows before another runtime call.

Start with [script authoring concepts](../../docs/scripting/authoring/index.md).

## Verified Source Map

- [scene wiring](res/main.scn): fixed `NodeID` refs + `TextureID` path injection
- [controller](res/scripts/controller.rs): lifecycle, timer, typed asset handoff, targeted methods
- [player](res/scripts/player.rs): typed state/node access, asset use, signal emit
- [HUD](res/scripts/hud.rs): signal listener + typed self-node edit
- [audit listener](res/scripts/audit.rs): second listener on same signal
- [dynamic adapter](res/scripts/adapter.rs): runtime-selected `get_var!` / `set_var!`

The scene path becomes `TextureID` before controller `on_init`. Controller sends
that typed ID to player. Player applies it to its fixed sprite ref. This proves
injection and use, rather than only storing the ID.
