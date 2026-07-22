# Actions Module

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
| `down_hash` | [`down_hash`](#down_hash) |
| `pressed_hash` | [`pressed_hash`](#pressed_hash) |
| `released_hash` | [`released_hash`](#released_hash) |
| Native Rebinding | [Native Rebinding](#native-rebinding) |
| `start_rebind` | [`start_rebind`](#start_rebind) |
| `start_rebind_hash` | [`start_rebind_hash`](#start_rebind_hash) |
| `cancel_rebind` | [`cancel_rebind`](#cancel_rebind) |
| `is_rebinding` | [`is_rebinding`](#is_rebinding) |
| `rebind_result` | [`rebind_result`](#rebind_result) |
| Rebind Macros | [Rebind Macros](#rebind-macros) |
| Query Macros | [Query Macros](#query-macros) |

## Purpose

Actions are named, device-independent controls. Instead of checking a specific
key, you check `"jump"` or `"fire"`, and the input map decides which keyboard,
mouse, gamepad, or Joy-Con bindings satisfy it. That indirection is what makes a
rebindable controls menu possible: the same gameplay code keeps calling
`action_pressed!(ctx.ipt, "jump")` while the player swaps the underlying binding
at runtime. Perro captures the new binding natively; your game owns saving it.

## Use Cases

- One control, many devices: bind "jump" to Space, the A button, and a Joy-Con
  button at once, and read it with `action_pressed!(ctx.ipt, "jump")`.
- Rebindable controls menu: start a live listener with
  `action_start_rebind!(ctx.ipt, "jump")`, show a "press any key" prompt while
  `action_is_rebinding!` is true, then persist `action_rebind_result!`.
- Coyote-time / jump buffering: latch the `action_pressed!(ctx.ipt, "jump")`
  edge into a short timer so a slightly-early press still triggers the jump.
- Held vs. edge intent: charge while `action_down!(ctx.ipt, "fire")` and release
  on `action_released!(ctx.ipt, "fire")`.
- Hot-path reads: cache an action hash once and poll it with
  `pressed_hash` / `down_hash` to skip re-hashing the name each frame.

## Ownership And Choice

The input map owns physical bindings; gameplay owns the meaning of an action. Use named actions for player intent that must survive rebinding and device changes. Use raw key, mouse, or gamepad reads for device-selection UI and diagnostics. Save completed rebind results in project/save data; do not make individual gameplay scripts own separate binding truth.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Actions()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

Perro provides native live input rebinding. Start a listener for an action, and
the next new keyboard, mouse, gamepad, or Joy-Con button press replaces that
action's bindings in the active input map.

The engine applies the rebind in memory. Game code owns persistence: read the
result, save it through the project's chosen storage format, and restore the
saved bindings on a later run. Perro does not write player settings by itself.

## Practical Example

```rust
#[State]
struct ControlsState {
    #[default = false]
    waiting_for_key: bool,
}

methods!({
    // Bound to a "Rebind Jump" button in the options menu.
    fn on_rebind_jump_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        action_start_rebind!(ctx.ipt, "jump");
        with_state_mut!(ctx.run, ControlsState, ctx.id, |state| state.waiting_for_key = true);
    }
});

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Gameplay reads the abstract action; bindings can change under it.
        if action_pressed!(ctx.ipt, "jump") {
            // start jump
        }

        // Detect rebind completion and hand the binding to game-owned storage.
        if let Some(result) = action_rebind_result!(ctx.ipt) {
            // result.action, result.action_hash, result.binding
            let _ = result;
            with_state_mut!(ctx.run, ControlsState, ctx.id, |state| state.waiting_for_key = false);
        }
    }
});
```

## API Reference

### `down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn down(&self, name: &str) -> bool` |
| Params | `&self, name: &str` |
| Returns | `bool` |
| Use when | Any binding for the action is held (charge, hold-to-aim). |
| Edge behavior | Hashes the name, then reads the input map; unknown actions return `false`. |

### `pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn pressed(&self, name: &str) -> bool` |
| Params | `&self, name: &str` |
| Returns | `bool` |
| Use when | Any binding for the action fires on the down edge this frame (jump, confirm). |
| Edge behavior | `true` only on the frame a binding transitions to down. |

### `released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn released(&self, name: &str) -> bool` |
| Params | `&self, name: &str` |
| Returns | `bool` |
| Use when | Any binding for the action fires on release (release a charged shot). |
| Edge behavior | `true` only on the frame a binding transitions to up. |

### `down_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn down_hash(&self, name_hash: u64) -> bool` |
| Params | `&self, name_hash: u64` |
| Returns | `bool` |
| Use when | Hot-path held check using a precomputed action hash. |
| Edge behavior | Same as `down` without re-hashing the name. |

### `pressed_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn pressed_hash(&self, name_hash: u64) -> bool` |
| Params | `&self, name_hash: u64` |
| Returns | `bool` |
| Use when | Hot-path press-edge check using a precomputed action hash. |
| Edge behavior | Same as `pressed` without re-hashing the name. |

### `released_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn released_hash(&self, name_hash: u64) -> bool` |
| Params | `&self, name_hash: u64` |
| Returns | `bool` |
| Use when | Hot-path release-edge check using a precomputed action hash. |
| Edge behavior | Same as `released` without re-hashing the name. |

## Native Rebinding

Start listening from a script callback:

```rust
ctx.ipt.Actions().start_rebind("jump");
```

The command starts at the next input frame. The next new button press becomes
the action's only binding. Held buttons and repeated key events do not count as
new presses.

Poll for completion and save the returned data:

```rust
let actions = ctx.ipt.Actions();

if let Some(result) = actions.rebind_result() {
    // result.action: action name
    // result.action_hash: stable action hash
    // result.binding: captured InputBinding
    // Save this data with the game's own settings/storage code.
}
```

Restore saved data while building the runtime input map:

```rust
input_map.set_bindings("jump", vec![saved_binding]);
input.set_input_map(input_map);
```

`InputMap::set_bindings_hash` provides the same load path for a saved action
hash. Both methods return `false` when the action does not exist.

### `start_rebind`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn start_rebind(&self, name: &str)` |
| Params | Action name |
| Returns | `()` |
| Use when | Start native live rebind by action name. |
| Edge behavior | Queued command; starts next input frame only when the action exists. |

### `start_rebind_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn start_rebind_hash(&self, action_hash: u64)` |
| Params | Stable action hash |
| Returns | `()` |
| Use when | Start native live rebind with a cached action hash. |
| Edge behavior | Queued command; starts next input frame only when the action exists. |

### `cancel_rebind`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn cancel_rebind(&self)` |
| Returns | `()` |
| Use when | Close a rebind prompt without changing bindings. |
| Edge behavior | Queued command; input received before command application may still bind. |

### `is_rebinding`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn is_rebinding(&self) -> bool` |
| Returns | `bool` |
| Use when | Show or hide a waiting-for-input prompt. |
| Edge behavior | Turns false after capture or cancellation. |

### `rebind_result`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn rebind_result(&self) -> Option<&RebindResult>` |
| Returns | Captured action name, action hash, and `InputBinding`. |
| Use when | Detect completion and save developer-owned settings. |
| Edge behavior | Remains available until another rebind starts. Perro does not save it automatically. |

### Rebind Macros

Use macro forms when script code prefers the compact input API:

```rust
action_start_rebind!(ctx.ipt, "jump");

if action_is_rebinding!(ctx.ipt) {
    // Show waiting prompt.
}

if let Some(result) = action_rebind_result!(ctx.ipt) {
    // Save result.action + result.binding.
}

action_cancel_rebind!(ctx.ipt);
```

Literal action names use a compile-time action hash. Runtime string expressions
also work and hash at the call site.

Starting another rebind clears the prior result. Rebinding an unknown action
does not start a listener. Call `cancel_rebind()` to stop a pending listener.

## Query Macros

| Macro | Signature | Returns |
| --- | --- | --- |
| `action_down!` | `action_down!(ctx.ipt, "jump")` | `bool` |
| `action_pressed!` | `action_pressed!(ctx.ipt, "jump")` | `bool` |
| `action_released!` | `action_released!(ctx.ipt, "jump")` | `bool` |
