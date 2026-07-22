# Input

Perro input flows through `ctx.ipt`.

## Goal

Read keys, mouse, gamepads, joycons, players, and actions.

## Decision Model

Use physical keys for debug tools, editor-like controls, or a deliberately fixed
scheme. Use actions for player verbs because bindings, device type, and player
assignment can change without changing gameplay code. Read continuous axes in
`on_update`; read edge actions for one-shot commands such as jump or confirm.

Input reports intent. The player script decides game state. A HUD or audio
script should receive the resulting event/state, not poll the keyboard again.

## Keyboard

Use key helpers for direct movement and debug input:

```rust
if key_pressed!(ctx.ipt, KeyCode::Space) {
    log_info!("jump");
}
```

Use `key_down!` for held state.

Use `key_pressed!` for edge state.

## Mouse

Use mouse APIs for pointer UI, camera tools, and aim:

```rust
let pos = mouse_position!(ctx.ipt);
let _ = pos;
```

## Gamepads

Use gamepad APIs for local player input.

Prefer actions when more than one device layout matters.

## Actions

Actions map devices to game verbs.

Use actions for gameplay:

- `move`
- `jump`
- `attack`
- `pause`

Use raw key/mouse/gamepad for tools and debug panels.

## Player Devices

Use player APIs when local multiplayer matters.

Keep gameplay code player-indexed instead of device-indexed.

## Reference

- [Input API](/docs/scripting/contexts/input_api.md)
- [Actions Module](/docs/scripting/contexts/input_modules/actions.md)
- [Keys Module](/docs/scripting/contexts/input_modules/keys.md)
- [Mouse Module](/docs/scripting/contexts/input_modules/mouse.md)
- [Gamepads Module](/docs/scripting/contexts/input_modules/gamepads.md)
- [Joycons Module](/docs/scripting/contexts/input_modules/joycons.md)
- [Players Module](/docs/scripting/contexts/input_modules/players.md)
