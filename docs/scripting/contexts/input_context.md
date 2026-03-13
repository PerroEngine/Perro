# Input Context

Type:
- `ipt: &InputContext<'_, IP>`

Purpose:
- Read frame input state for gameplay and interaction logic.

Accessors:
- `ipt.Keys()`
- `ipt.Mouse()`

## Input Modules

- [Keys Module](input_modules/keys.md)
- [Mouse Module](input_modules/mouse.md)

Each module page contains:
- Macro reference
- `ipt.<Module>()` methods
- Examples

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
