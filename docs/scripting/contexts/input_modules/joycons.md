# Joycons Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `all` | [`all`](#all) |
| `get` | [`get`](#get) |
| `set_rumble` | [`set_rumble`](#set_rumble) |
| `set_indicator` | [`set_indicator`](#set_indicator) |
| `set_indicator_slot` | [`set_indicator_slot`](#set_indicator_slot) |

## Overview

This input module belongs to `ctx.ipt` and documents joycons calls.

## Provenance

Joy-Con support here exists for PC input and for Perro's abstract input API.

Tiernan DeFranco, lead developer of Perro, built the first version as a standalone C++ research test project in Summer 2025, then ported it to Rust and Perro.

This code comes from reading Bluetooth HID and BLE GATT raw bytes from Joy-Con devices on PC, then mapping those bytes into Perro controls.

Public open source projects, including JoyconPython, helped explain control reads and mappings.

This code does not use Nintendo SDK code, private Nintendo internals, or NDA material. Tiernan does not have access to those materials at the time this PC backend was written.

If Tiernan later gains private Nintendo SDK access through Perro or other ventures, he will not use that access to update this public PC backend.

Nintendo Switch or Switch 2 game builds will use a separate private implementation that calls the official SDK directly. That implementation is not part of this open source PC Joy-Con backend.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.JoyCons()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `all`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn all(&self) -> &'ipt [JoyConState]` |
| Params | `&self` |
| Returns | `&'ipt [JoyConState]` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn get(&self, index: usize) -> Option<&'ipt JoyConState>` |
| Params | `&self, index: usize` |
| Returns | `Option<&'ipt JoyConState>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32)` |
| Params | `&self, index: usize, low_frequency: f32, high_frequency: f32` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_indicator`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_indicator(&self, index: usize, indicator: u8)` |
| Params | `&self, index: usize, indicator: u8` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_indicator_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_indicator_slot(&self, index: usize, slot: u8)` |
| Params | `&self, index: usize, slot: u8` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `joycon_accel`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_accel!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_calibrated`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_calibrated!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_calibrating`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_calibrating!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_calibration_bias`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_calibration_bias!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_connected`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_connected!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_down`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_down!(ctx.ipt, 0, JoyConButton::Top)` |
| Params | `ctx.ipt, 0, JoyConButton::Top` |
| Returns | `bool` |
| Use when | Use when gameplay needs held input state, such as movement, aim, charge, or drag. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_get!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Option` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_gyro`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_gyro!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector3` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_list`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_list!(ctx.ipt)` |
| Params | `ctx.ipt` |
| Returns | `slice` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_needs_calibration`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_needs_calibration!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `bool` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_pressed`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_pressed!(ctx.ipt, 0, JoyConButton::Top)` |
| Params | `ctx.ipt, 0, JoyConButton::Top` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_released`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_released!(ctx.ipt, 0, JoyConButton::Top)` |
| Params | `ctx.ipt, 0, JoyConButton::Top` |
| Returns | `bool` |
| Use when | Use when gameplay needs a one-frame input edge, such as jump, confirm, cancel, or release. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_request_calibration`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_request_calibration!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_set_indicator`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_set_indicator!(ctx.ipt, 0, 1)` |
| Params | `ctx.ipt, 0, 1` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_set_rumble!(ctx.ipt, 0, 0.5, 0.5)` |
| Params | `ctx.ipt, 0, 0.5, 0.5` |
| Returns | `()` |
| Use when | Use when code must queue an input device, cursor, rumble, indicator, or calibration command. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_side`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_side!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Option<JoyConSide>` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

### `joycon_stick`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `joycon_stick!(ctx.ipt, 0)` |
| Params | `ctx.ipt, 0` |
| Returns | `Vector2` |
| Use when | Use when code needs current input device data without storing platform input state itself. |
| Fails when / edge behavior | Missing device slots return `None`, `false`, or a zero vector depending on the macro return type. Command macros queue work when an input command buffer exists. |

