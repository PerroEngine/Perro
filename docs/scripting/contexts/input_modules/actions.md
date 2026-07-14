# Actions Module

## Page Map

| Header          | Link                              |
| --------------- | --------------------------------- |
| Overview        | [Overview](#overview)             |
| Context         | [Context](#context)               |
| API Reference   | [API Reference](#api-reference)   |
| `down`          | [`down`](#down)                   |
| `pressed`       | [`pressed`](#pressed)             |
| `released`      | [`released`](#released)           |
| `down_hash`     | [`down_hash`](#down_hash)         |
| `pressed_hash`  | [`pressed_hash`](#pressed_hash)   |
| `released_hash` | [`released_hash`](#released_hash) |
| Native rebinding | [Native Rebinding](#native-rebinding) |
| `start_rebind` | [`start_rebind`](#start_rebind) |
| `start_rebind_hash` | [`start_rebind_hash`](#start_rebind_hash) |
| `cancel_rebind` | [`cancel_rebind`](#cancel_rebind) |
| `is_rebinding` | [`is_rebinding`](#is_rebinding) |
| `rebind_result` | [`rebind_result`](#rebind_result) |
| Rebind macros | [Rebind Macros](#rebind-macros) |

## Overview

This input module belongs to `ctx.ipt` and documents actions calls.

Perro provides native live input rebinding. Start a listener for an action, and
the next new keyboard, mouse, gamepad, or Joy-Con button press replaces that
action's bindings in the active input map.

The engine applies the rebind in memory. Game code owns persistence: read the
result, save it through the project's chosen storage format, and restore the
saved bindings on a later run. Perro does not write player settings by itself.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Actions()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `down`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn down(&self, name: &str) -> bool`                                                                                                                                                                           |
| Params                     | `&self, name: &str`                                                                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `pressed`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn pressed(&self, name: &str) -> bool`                                                                                                                                                                        |
| Params                     | `&self, name: &str`                                                                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `released`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn released(&self, name: &str) -> bool`                                                                                                                                                                       |
| Params                     | `&self, name: &str`                                                                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `down_hash`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn down_hash(&self, name_hash: u64) -> bool`                                                                                                                                                                  |
| Params                     | `&self, name_hash: u64`                                                                                                                                                                                            |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `pressed_hash`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn pressed_hash(&self, name_hash: u64) -> bool`                                                                                                                                                               |
| Params                     | `&self, name_hash: u64`                                                                                                                                                                                            |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code branches on current state or a one-frame state edge.                                                                                                                                                 |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `released_hash`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.ipt.Actions()`                                                                                                                                                                                                |
| Signature                  | `pub fn released_hash(&self, name_hash: u64) -> bool`                                                                                                                                                              |
| Params                     | `&self, name_hash: u64`                                                                                                                                                                                            |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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

### `start_rebind`

| Field | Detail |
| ----- | ------ |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn start_rebind(&self, name: &str)` |
| Params | Action name |
| Returns | `()` |
| Use when | Start native live rebind by action name. |
| Edge behavior | Queued command; starts next input frame only when the action exists. |

### `start_rebind_hash`

| Field | Detail |
| ----- | ------ |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn start_rebind_hash(&self, action_hash: u64)` |
| Params | Stable action hash |
| Returns | `()` |
| Use when | Start native live rebind with a cached action hash. |
| Edge behavior | Queued command; starts next input frame only when the action exists. |

### `cancel_rebind`

| Field | Detail |
| ----- | ------ |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn cancel_rebind(&self)` |
| Returns | `()` |
| Use when | Close a rebind prompt without changing bindings. |
| Edge behavior | Queued command; input received before command application may still bind. |

### `is_rebinding`

| Field | Detail |
| ----- | ------ |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn is_rebinding(&self) -> bool` |
| Returns | `bool` |
| Use when | Show or hide a waiting-for-input prompt. |
| Edge behavior | Turns false after capture or cancellation. |

### `rebind_result`

| Field | Detail |
| ----- | ------ |
| Access | `ctx.ipt.Actions()` |
| Signature | `pub fn rebind_result(&self) -> Option<&RebindResult>` |
| Returns | Captured action name, action hash, and `InputBinding`. |
| Use when | Detect completion and save developer-owned settings. |
| Edge behavior | Remains available until another rebind starts. Perro does not save it automatically. |

### `action_down`

| Field                      | Detail                                                                                                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.ipt`                                                                                                                                                        |
| Signature                  | `action_down!(ctx.ipt, "jump")`                                                                                                                                  |
| Params                     | `ctx.ipt, "jump"`                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                           |
| Use when                   | Use when gameplay needs held input state, such as movement, aim, charge, or drag.                                                                                |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `action_pressed`

| Field                      | Detail                                                                                                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.ipt`                                                                                                                                                        |
| Signature                  | `action_pressed!(ctx.ipt, "jump")`                                                                                                                               |
| Params                     | `ctx.ipt, "jump"`                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                           |
| Use when                   | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release.                                                                       |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `action_released`

| Field                      | Detail                                                                                                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.ipt`                                                                                                                                                        |
| Signature                  | `action_released!(ctx.ipt, "jump")`                                                                                                                              |
| Params                     | `ctx.ipt, "jump"`                                                                                                                                                |
| Returns                    | `bool`                                                                                                                                                           |
| Use when                   | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release.                                                                       |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

