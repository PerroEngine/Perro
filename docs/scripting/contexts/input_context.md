# Input Context

Type:
- `ipt: &InputContext<'_, IP>`

Purpose:
- Read frame input state for gameplay and interaction logic.

Accessors:
- `ipt.Keys()`
- `ipt.Mouse()`
- `ipt.Gamepads()`
- `ipt.JoyCons()`
- `ipt.Players()`

## Input Modules

- [Keys Module](input_modules/keys.md)
- [Mouse Module](input_modules/mouse.md)
- [Gamepads Module](input_modules/gamepads.md)
- [Joy-Cons Module](input_modules/joycons.md)
- [Players Module](input_modules/players.md)

Each module page contains:
- Macro reference
- `ipt.<Module>()` methods
- Examples
- Binding notes for player and device mappings

## Simple Example

```rust
if key_pressed!(ipt, KeyCode::Space) {
    signal_emit!(ctx, signal!("jump"));
}

if mouse_down!(ipt, MouseButton::Left) {
    let delta = mouse_delta!(ipt);
    with_node_mut!(ctx, Node3D, self_id, |node| {
        node.rotation.y += delta.x * 0.01;
        node.rotation.x += delta.y * 0.01;
    });
}
```
