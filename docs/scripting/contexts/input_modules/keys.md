# Keys Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `down` | [`down`](#down) |
| `pressed` | [`pressed`](#pressed) |
| `released` | [`released`](#released) |
| `key_down!` | [`key_down!`](#key_down) |
| `key_pressed!` | [`key_pressed!`](#key_pressed) |
| `key_released!` | [`key_released!`](#key_released) |

## Purpose

The keys module reports keyboard state for the current frame. `down` stays true
while a key is held; `pressed` and `released` are one-frame edges that fire
exactly once per keystroke. That split is what separates smooth movement (held)
from single-shot actions like jump or menu confirm (edge), and it removes any
need to track "was this key down last frame" yourself.

## Use Cases

- WASD movement: read four held keys with `key_down!(ctx.ipt, KeyCode::KeyW)`
  and friends to build a movement vector.
- Jump that never repeats: trigger on the down edge with
  `key_pressed!(ctx.ipt, KeyCode::Space)` so holding the key jumps only once.
- Charge-and-release: start a charge on `key_pressed!` and fire it on
  `key_released!(ctx.ipt, KeyCode::KeyJ)`.
- Pause / cancel: open a menu on `key_pressed!(ctx.ipt, KeyCode::Escape)`.
- Debug toggles: flip an overlay with a single `key_pressed!` check on a
  function key.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Keys()`
- `KeyCode` is the keyboard-key enum (letters, numbers, arrows, function keys, and more).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
#[State]
struct MoverState {
    #[default = false]
    jumping: bool,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Held keys drive movement.
        let mut dx = 0.0;
        if key_down!(ctx.ipt, KeyCode::KeyA) { dx -= 1.0; }
        if key_down!(ctx.ipt, KeyCode::KeyD) { dx += 1.0; }

        // Edge fires the jump exactly once per press.
        if key_pressed!(ctx.ipt, KeyCode::Space) {
            with_state_mut!(ctx.run, MoverState, ctx.id, |state| state.jumping = true);
        }
        let _ = dx;
    }
});
```

## API Reference

### `down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Keys()` |
| Signature | `pub fn down(&self, key: KeyCode) -> bool` |
| Params | `&self, key: KeyCode` |
| Returns | `bool` |
| Use when | A control must respond while the key is held, such as movement or aim. |
| Edge behavior | Stays `true` every frame the key is held. Unknown/unpressed keys return `false`. |

### `pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Keys()` |
| Signature | `pub fn pressed(&self, key: KeyCode) -> bool` |
| Params | `&self, key: KeyCode` |
| Returns | `bool` |
| Use when | An action must fire once on the down edge, such as jump or confirm. |
| Edge behavior | `true` only on the frame the key changes up to down; cleared next frame. |

### `released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Keys()` |
| Signature | `pub fn released(&self, key: KeyCode) -> bool` |
| Params | `&self, key: KeyCode` |
| Returns | `bool` |
| Use when | An action must fire on release, such as firing a charged shot. |
| Edge behavior | `true` only on the frame the key changes down to up; cleared next frame. |

### `key_down!`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `key_down!(ctx.ipt, KeyCode::KeyW) -> bool` |
| Params | `ctx.ipt, KeyCode` |
| Returns | `bool` |
| Use when | Compact held-key check; expands to `ctx.ipt.Keys().down(key)`. |
| Edge behavior | Same as `down`: `true` while held. |

### `key_pressed!`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `key_pressed!(ctx.ipt, KeyCode::Space) -> bool` |
| Params | `ctx.ipt, KeyCode` |
| Returns | `bool` |
| Use when | Compact press-edge check; expands to `ctx.ipt.Keys().pressed(key)`. |
| Edge behavior | `true` only on the down-edge frame. |

### `key_released!`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `key_released!(ctx.ipt, KeyCode::Space) -> bool` |
| Params | `ctx.ipt, KeyCode` |
| Returns | `bool` |
| Use when | Compact release-edge check; expands to `ctx.ipt.Keys().released(key)`. |
| Edge behavior | `true` only on the release-edge frame. |
