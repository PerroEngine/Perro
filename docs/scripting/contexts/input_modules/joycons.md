# Joycons Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Provenance | [Provenance](#provenance) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `all` | [`all`](#all) |
| `get` | [`get`](#get) |
| `set_rumble` | [`set_rumble`](#set_rumble) |
| `set_indicator` | [`set_indicator`](#set_indicator) |
| `set_indicator_slot` | [`set_indicator_slot`](#set_indicator_slot) |
| `ensure_calibration` | [`ensure_calibration`](#ensure_calibration) |
| Macros | [Macros](#macros) |

## Purpose

The Joy-Con module exposes each connected Joy-Con as its own slot with buttons,
a stick, gyro/accel motion, HD rumble, a player LED indicator, and (on Joy-Con 2)
a mouse sensor. Because each Joy-Con is a self-contained controller, a single
pair can serve one player held sideways per hand, which makes Joy-Cons a natural
fit for pick-up-and-play local multiplayer. Stick drift is handled through an
opt-in calibration flow the game triggers and persists.

## Use Cases

- Two-player-from-one-pair co-op: give each player one Joy-Con and read it by
  slot with `joycon_get!(ctx.ipt, 0)` / `joycon_get!(ctx.ipt, 1)`.
- Motion aiming and steering: read tilt from `joycon_gyro!(ctx.ipt, 0)` and
  shake gestures from `joycon_accel!`.
- HD rumble feedback: pulse on hit with `joycon_set_rumble!(ctx.ipt, 0, 0.5, 0.5)`.
- Player-color LEDs: light the indicator for a player slot with
  `joycon_set_indicator!(ctx.ipt, 0, 1)` so players know which controller is theirs.
- Joy-Con 2 pointer input: read the mouse-style sensor with
  `joycon_mouse_sensor!(ctx.ipt, 0)` for cursor or aim deltas.
- Drift calibration: detect a controller that needs it with
  `joycon_needs_calibration!` and start it once with
  `joycon_ensure_calibration!(ctx.ipt, 0)`, then save the result.

## Provenance

Joy-Con support here exists for PC input and for Perro's abstract input API.

Tiernan DeFranco, lead developer of Perro, built the first version as a standalone C++ research test project in Summer 2025, then ported it to Rust and Perro.

This code comes from reading Bluetooth HID and BLE GATT raw bytes from Joy-Con devices on PC, then mapping those bytes into Perro controls.

Public open source projects, including JoyconPython and joycon2cpp, helped explain control reads, mappings, player LEDs, and Joy-Con 2 rumble writes.

joycon2cpp documents Joy-Con 2 BLE notification offsets for buttons, sticks, mouse data, battery, temperature, accel, gyro, and analog triggers, plus observed pairing cooldown behavior.

Perro does not claim Joy-Con 2 decryption work here. The current PC backend reads BLE reports after normal OS pairing and uses observed public report layouts and command packets.

This code does not use Nintendo SDK code, private Nintendo internals, or NDA material. Tiernan does not have access to those materials at the time this PC backend was written.

If Tiernan later gains private Nintendo SDK access through Perro or other ventures, he will not use that access to update this public PC backend.

Nintendo Switch or Switch 2 game builds will use a separate private implementation that calls the official SDK directly. That implementation is not part of this open source PC Joy-Con backend.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.JoyCons()`
- `JoyConButton` maps per side (for example `Top` = Up on the left Joy-Con, X on the right); `JoyConSide` tells you left or right.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        // Light player 1's LED and calibrate if needed.
        joycon_set_indicator!(ctx.ipt, 0, 1);
        joycon_ensure_calibration!(ctx.ipt, 0);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Single Joy-Con held sideways: stick for movement, motion for aim.
        let move_dir = joycon_stick!(ctx.ipt, 0);
        let tilt = joycon_gyro!(ctx.ipt, 0);

        if joycon_pressed!(ctx.ipt, 0, JoyConButton::Bottom) {
            joycon_set_rumble!(ctx.ipt, 0, 0.5, 0.5);
        }
        let _ = (move_dir, tilt);
    }
});
```

## API Reference

### `all`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn all(&self) -> &'ipt [JoyConState]` |
| Params | `&self` |
| Returns | `&'ipt [JoyConState]` |
| Use when | Enumerate connected Joy-Cons; each entry is one controller. |
| Edge behavior | May be empty when none are connected. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn get(&self, index: usize) -> Option<&'ipt JoyConState>` |
| Params | `&self, index: usize` |
| Returns | `Option<&'ipt JoyConState>` |
| Use when | Read one Joy-Con's buttons, stick, motion, and side by slot. |
| Edge behavior | Returns `None` when the slot is empty. |

### `set_rumble`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_rumble(&self, index: usize, low_frequency: f32, high_frequency: f32)` |
| Params | `&self, index: usize, low_frequency: f32, high_frequency: f32` |
| Returns | `()` |
| Use when | HD rumble feedback; set both to `0.0` to stop. |
| Edge behavior | Queues a command when a command buffer exists; missing slots are ignored. |

### `set_indicator`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_indicator(&self, index: usize, indicator: u8)` |
| Params | `&self, index: usize, indicator: u8` |
| Returns | `()` |
| Use when | Set the player LED by slot number or raw lamp bit pattern. |
| Edge behavior | Ignored when the value is not a valid slot or lamp pattern; otherwise queued. |

### `set_indicator_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn set_indicator_slot(&self, index: usize, slot: u8)` |
| Params | `&self, index: usize, slot: u8` |
| Returns | `()` |
| Use when | Set the player LED by zero-based slot only. |
| Edge behavior | Ignored when the slot is out of range; otherwise queued. |

### `ensure_calibration`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.JoyCons()` |
| Signature | `pub fn ensure_calibration(&self, index: usize) -> bool` |
| Params | `&self, index: usize` |
| Returns | `bool` |
| Use when | Queue calibration only if the indexed Joy-Con currently needs it. |
| Edge behavior | Returns `false` for missing slots. Backend maps index to the connected serial and stores calibration in the global Perro calibration folder. |

## Macros

For a missing slot, button reads return `false`, side/state reads return `None`,
stick reads return `Vector2::ZERO`, and gyro/accel reads return `Vector3::ZERO`.
Command macros queue work only when an input command buffer exists.

| Macro | Signature | Returns |
| --- | --- | --- |
| `joycon_list!` | `joycon_list!(ctx.ipt)` | `&[JoyConState]` |
| `joycon_get!` | `joycon_get!(ctx.ipt, 0)` | `Option<&JoyConState>` |
| `joycon_side!` | `joycon_side!(ctx.ipt, 0)` | `Option<JoyConSide>` |
| `joycon_down!` | `joycon_down!(ctx.ipt, 0, JoyConButton::Top)` | `bool` |
| `joycon_pressed!` | `joycon_pressed!(ctx.ipt, 0, JoyConButton::Top)` | `bool` |
| `joycon_released!` | `joycon_released!(ctx.ipt, 0, JoyConButton::Top)` | `bool` |
| `joycon_stick!` | `joycon_stick!(ctx.ipt, 0)` | `Vector2` |
| `joycon_gyro!` | `joycon_gyro!(ctx.ipt, 0)` | `Vector3` |
| `joycon_accel!` | `joycon_accel!(ctx.ipt, 0)` | `Vector3` |
| `joycon_mouse_sensor!` | `joycon_mouse_sensor!(ctx.ipt, 0)` | `JoyConMouseSensor` (delta, extra axis, distance; zeroed on Joy-Con 1) |
| `joycon_connected!` | `joycon_connected!(ctx.ipt, 0)` | `bool` |
| `joycon_calibrated!` | `joycon_calibrated!(ctx.ipt, 0)` | `bool` |
| `joycon_calibrating!` | `joycon_calibrating!(ctx.ipt, 0)` | `bool` |
| `joycon_needs_calibration!` | `joycon_needs_calibration!(ctx.ipt, 0)` | `bool` |
| `joycon_calibration_bias!` | `joycon_calibration_bias!(ctx.ipt, 0)` | `Vector3` |
| `joycon_request_calibration!` | `joycon_request_calibration!(ctx.ipt, 0)` | `()` (always queues a calibration request) |
| `joycon_ensure_calibration!` | `joycon_ensure_calibration!(ctx.ipt, 0)` | `bool` (queues calibration only if needed; stores result for all Perro projects) |
| `joycon_set_rumble!` | `joycon_set_rumble!(ctx.ipt, 0, 0.5, 0.5)` | `()` |
| `joycon_set_indicator!` | `joycon_set_indicator!(ctx.ipt, 0, 1)` | `()` |
