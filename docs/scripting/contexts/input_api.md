# Input API

Type:
- `ctx: &mut ScriptContext<'_, API>`
- input window handle: `ctx.ipt`

Purpose:
- Read frame input state for gameplay and interaction logic.

Accessors:
- `ctx.ipt.Keys()`
- `ctx.ipt.Mouse()`
- `ctx.ipt.Actions()`
- `ctx.ipt.Gamepads()`
- `ctx.ipt.JoyCons()`
- `ctx.ipt.Players()`

## Input Map

Create `input_map.toml` next to `project.toml`:

```toml
[jump]
keys = ["KeySpace", "KeyUp"]
mouse = ["Left"]
gamepad = ["Bottom"]
joycon = ["Bottom"]
```

Query actions:

```rust
if action_pressed!(ctx.ipt, "jump") {
    signal_emit!(ctx.run, signal!("jump"));
}
```

Methods:
- `ctx.ipt.Actions().down("jump") -> bool`
- `ctx.ipt.Actions().pressed("jump") -> bool`
- `ctx.ipt.Actions().released("jump") -> bool`

Macros:
- `action_down!(ipt, "jump") -> bool`
- `action_pressed!(ipt, "jump") -> bool`
- `action_released!(ipt, "jump") -> bool`

Bindings:
- `keys`: `KeyCode` names, plus aliases like `KeySpace` and `KeyUp`
- `mouse`: `MouseButton` names
- `gamepad`: `GamepadButton` names, matched against any connected gamepad
- `joycon`: `JoyConButton` names, matched against any connected Joy-Con

## Input Modules

- [Keys Module](input_modules/keys.md)
- [Mouse Module](input_modules/mouse.md)
- [Gamepads Module](input_modules/gamepads.md)
- [Joy-Cons Module](input_modules/joycons.md)
- [Players Module](input_modules/players.md)

Each module page contains:
- Macro reference
- `ctx.ipt.<Module>()` methods
- Examples
- Binding notes for player and device mappings

## Simple Example

```rust
if key_pressed!(ctx.ipt, KeyCode::Space) {
    signal_emit!(ctx.run, signal!("jump"));
}

if mouse_down!(ctx.ipt, MouseButton::Left) {
    let delta = mouse_delta!(ctx.ipt);
    with_node_mut!(ctx.run, Node3D, ctx.id, |node| {
        node.rotation.y += delta.x * 0.01;
        node.rotation.x += delta.y * 0.01;
    });
}
```


