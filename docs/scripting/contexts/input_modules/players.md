# Players Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |

## Overview

This input module belongs to `ctx.ipt` and documents players calls.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Players()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

No standalone public macro or method is defined for this helper page.

### `player_get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `player_get!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Option` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `player_list`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `player_list!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `slice` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

