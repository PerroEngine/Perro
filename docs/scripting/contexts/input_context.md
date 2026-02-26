# Input Context

Type:
- `ipt: &InputContext<'_, IP>`

Purpose:
- Read frame input state for gameplay and interaction logic.

Accessors:
- `ipt.Keys()`
- `ipt.Mouse()`

Macros:
- `key_down!(ipt, key) -> bool`
- `key_pressed!(ipt, key) -> bool`
- `key_released!(ipt, key) -> bool`
- `mouse_down!(ipt, button) -> bool`
- `mouse_pressed!(ipt, button) -> bool`
- `mouse_released!(ipt, button) -> bool`
- `mouse_delta!(ipt) -> Vector2`
- `mouse_wheel!(ipt) -> Vector2`

## Key Methods

### `ipt.Keys().down(key) -> bool`
### `ipt.Keys().pressed(key) -> bool`
### `ipt.Keys().released(key) -> bool`
- `key`: `KeyCode`.

## Mouse Methods

### `ipt.Mouse().down(button) -> bool`
### `ipt.Mouse().pressed(button) -> bool`
### `ipt.Mouse().released(button) -> bool`
- `button`: `MouseButton`.

### `ipt.Mouse().delta() -> Vector2`
### `ipt.Mouse().wheel() -> Vector2`

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
