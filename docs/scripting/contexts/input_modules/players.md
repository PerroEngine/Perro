# Players Module

Access:

- `ipt.Players()`

Macros:

- `player_list!(ipt) -> &[PlayerState]`
- `player_get!(ipt, index) -> Option<&PlayerState>`
- `player_bind!(ipt, index, binding)`

Methods:

- `ipt.Players().all() -> &[PlayerState]`
- `ipt.Players().get(index) -> Option<&PlayerState>`

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
