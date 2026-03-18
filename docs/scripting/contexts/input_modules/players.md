# Players Module

Access:

- `ipt.Players()`

Purpose:
- Track whatever a "player" means in your game and map that player to a concrete input source.
- As long as you have a player index, you can get its binding and then read input from the right device.

Macros:

- `player_list!(ipt) -> &[PlayerState]`
- `player_get!(ipt, index) -> Option<&PlayerState>`
- `player_bind!(ipt, index, binding)`

Methods:

- `ipt.Players().all() -> &[PlayerState]`
- `ipt.Players().get(index) -> Option<&PlayerState>`

Common `PlayerState` methods:
- `state.get_binding() -> PlayerBinding`
- `state.get_kbm(keyboard, mouse) -> Option<(&KeyboardState, &MouseState)>`
- `state.get_gamepad(gamepads) -> Option<&GamepadState>`
- `state.get_joycon_single(joycons) -> Option<&JoyConState>`
- `state.get_joycon_pair(joycons) -> Option<(&JoyConState, &JoyConState)>`

How to use:
- If you already know which device a player should use, call the matching `get_*` method to access that device directly.
- If you do not know the device ahead of time, call `get_binding()` and branch on the `PlayerBinding` enum to decide what to read.

Bindings:

- `PlayerBinding::None`
- `PlayerBinding::Kbm`
- `PlayerBinding::Gamepad { index }`
- `PlayerBinding::JoyConSingle { index }`
- `PlayerBinding::JoyConPair { left, right }`

Notes:

- Bindings are developer-defined: you choose which device indices map to each player.
- For Joy-Con pairs, `left` / `right` are just slots; you can map whichever indices you want.

Source of truth:

- `perro_source/api_modules/perro_input/src/player.rs`
