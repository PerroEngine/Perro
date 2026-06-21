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

## Overview

This input module belongs to `ctx.ipt` and documents actions calls.

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

