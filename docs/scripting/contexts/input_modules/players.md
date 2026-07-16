# Players Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Player Bindings | [Player Bindings](#player-bindings) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `all` | [`all`](#all) |
| `get` | [`get`](#get) |
| Macros | [Macros](#macros) |

## Purpose

The players module maps player slots to input devices for local (couch)
multiplayer. Each player slot holds a `PlayerBinding` that says which device
drives it: keyboard+mouse, a single gamepad, a single Joy-Con, or a Joy-Con
pair. Gameplay then reads "player 0" and "player 1" without caring which
physical device is behind each one, so the same controller-assignment screen
works whether players show up with pads, a shared keyboard, or split Joy-Cons.

## Use Cases

- Controller assignment screen: bind each joined player to a device with
  `player_bind!(ctx.ipt, slot, PlayerBinding::Gamepad { index })`.
- Split Joy-Con co-op: give player 0 one Joy-Con and player 1 the other via
  `PlayerBinding::JoyConSingle { index }`, or bind both to one player with
  `PlayerBinding::JoyConPair { left, right }`.
- Mixed devices: let player 0 use `PlayerBinding::Kbm` while player 1 uses a pad.
- Per-player routing: resolve a slot's device with `player_get!(ctx.ipt, slot)`
  and its binding accessors, so each player's controller only drives their
  character.
- Roster scan: iterate `player_list!(ctx.ipt)` to see how many slots are bound.

## Context

- Script context path: `ctx.ipt`
- Module access: `ctx.ipt.Players()`
- Binding commands go through `ctx.ipt.bind_player(index, binding)` (or `player_bind!`).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Player Bindings

`PlayerBinding` selects the device for a player slot:

| Variant | Meaning |
| --- | --- |
| `PlayerBinding::None` | Slot has no device. |
| `PlayerBinding::Kbm` | Keyboard and mouse. |
| `PlayerBinding::Gamepad { index }` | Gamepad at slot `index`. |
| `PlayerBinding::JoyConSingle { index }` | One Joy-Con at slot `index`. |
| `PlayerBinding::JoyConPair { left, right }` | Left + right Joy-Cons as one controller. |

`PlayerState` resolves the bound device against the current input state:

- `get_binding() -> PlayerBinding`
- `get_kbm(keyboard, mouse) -> Option<(&KeyboardState, &MouseState)>`
- `get_gamepad(gamepads) -> Option<&GamepadState>`
- `get_joycon_single(joycons) -> Option<&JoyConState>`
- `get_joycon_pair(joycons) -> Option<(&JoyConState, &JoyConState)>`

Each accessor returns `Some` only when the slot's binding matches that device
kind, so a single match arm on `get_binding()` can branch player input by device.

## Practical Example

```rust
methods!({
    // Called from a "Player 2: Join" button in a controller-assignment screen.
    fn on_player_two_join(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        // Assign the second gamepad to player slot 1.
        player_bind!(ctx.ipt, 1, PlayerBinding::Gamepad { index: 1 });
    }
});

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Route input per player slot.
        if let Some(player) = player_get!(ctx.ipt, 1) {
            match player.get_binding() {
                PlayerBinding::Gamepad { index } => {
                    let move_dir = gamepad_left_stick!(ctx.ipt, index);
                    let _ = move_dir;
                }
                PlayerBinding::JoyConSingle { index } => {
                    let move_dir = joycon_stick!(ctx.ipt, index);
                    let _ = move_dir;
                }
                _ => {}
            }
        }
    }
});
```

## API Reference

### `all`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Players()` |
| Signature | `pub fn all(&self) -> &'ipt [PlayerState]` |
| Params | `&self` |
| Returns | `&'ipt [PlayerState]` |
| Use when | Enumerate player slots and their bindings. |
| Edge behavior | Slice covers the current player slots. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt.Players()` |
| Signature | `pub fn get(&self, index: usize) -> Option<&'ipt PlayerState>` |
| Params | `&self, index: usize` |
| Returns | `Option<&'ipt PlayerState>` |
| Use when | Read one player slot's binding and resolve its device. |
| Edge behavior | Returns `None` when the slot does not exist. |

### `bind_player`

| Field | Detail |
| --- | --- |
| Access | `ctx.ipt` |
| Signature | `pub fn bind_player(&self, index: usize, binding: PlayerBinding)` |
| Params | `&self, index: usize, binding: PlayerBinding` |
| Returns | `()` |
| Use when | Assign or clear the device for a player slot. |
| Edge behavior | Queues a command when an input command buffer exists. |

## Macros

Read macros return `None` / an empty slice for missing slots; `player_bind!`
queues a binding command.

| Macro | Signature | Returns |
| --- | --- | --- |
| `player_list!` | `player_list!(ctx.ipt)` | `&[PlayerState]` |
| `player_get!` | `player_get!(ctx.ipt, 0)` | `Option<&PlayerState>` |
| `player_bind!` | `player_bind!(ctx.ipt, 0, PlayerBinding::Kbm)` | `()` |
