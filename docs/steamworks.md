# Steamworks

Use Steamworks from game scripts through the normal Perro API prelude.

```rust
use perro_api::prelude::*;
```

## Setup

Add Steam config to `project.toml`:

```toml
[steam]
enabled = true
app_id = 480
```

Perro reads this at startup.

When `enabled = true`, Perro initializes Steam with `app_id`.

Perro also runs Steam callbacks during each runtime update.

Game scripts do not need Steam init, shutdown, or callback code.

Use your real Steam app id for shipped games.

Use `480` only for local Steamworks testing.

When Steam is disabled, Steam calls return `steam::SteamError::Disabled`.

## Achievements

Achievement ids must match ids in Steamworks portal.

```rust
steam_ach_unlock!("ACH_FIRST_WIN")?;
steam_ach_clear!("ACH_FIRST_WIN")?;
```

```rust
steam::achievements::unlock("ACH_FIRST_WIN")?;
steam::achievements::clear("ACH_FIRST_WIN")?;
```

## Unlock Many

Use this when one game event earns multiple achievements.

```rust
steam_ach_unlock!("ACH_FIRST_WIN", "ACH_NO_DAMAGE")?;

let ids = ["ACH_FIRST_WIN", "ACH_NO_DAMAGE"];
steam_ach_unlock!(&ids)?;

steam::achievements::unlock_many(ids)?;
```

## API

| Use         | Function                                | Macro                                                              |
| ----------- | --------------------------------------- | ------------------------------------------------------------------ |
| Unlock one  | `steam::achievements::unlock(id)`       | `steam_ach_unlock!(id)`                                            |
| Clear one   | `steam::achievements::clear(id)`        | `steam_ach_clear!(id)`                                             |
| Unlock many | `steam::achievements::unlock_many(ids)` | `steam_ach_unlock!("ACH_A", "ACH_B")` or `steam_ach_unlock!(&ids)` |

## Account

```rust
let my_id = steam::account::get_self_id()?;
let my_name = steam::account::get_self_name()?;
let logged_on = steam::account::logged_on()?;

let friend_name = steam::account::get_name(friend_id)?;
```

## Friends

```rust
let friends = steam::friends::list()?;

steam_rich_presence_set!(steam::RichPresenceKey::Status, "In menu")?;
steam::friends::open_overlay(steam::OverlayDialog::Friends)?;
```

Friend ids use `steam::SteamID`.

Use `SteamID::get_id()` when you need to send an id over network or write it to disk.

Use `SteamID::from_id(id)` to rebuild it.

## Lobbies

Lobby create, list, and join are async.

The function returns when Steam accepts the request.

Read final results from `steam::events::drain()?`.

```rust
steam_lobby_create!(steam::LobbyType::FriendsOnly, 4)?;

for event in steam_events!()? {
    match event {
        steam::SteamEvent::LobbyCreated { lobby } => {
            steam_lobby_data_set!(lobby, steam::LobbyDataKey::Mode, "coop")?;
            steam::friends::open_invite_dialog(lobby)?;
        }
        steam::SteamEvent::LobbyJoined { lobby } => {
            steam_lobby_chat!(lobby, "hello")?;
        }
        _ => {}
    }
}
```

```rust
let search = steam::LobbySearch {
    max_results: Some(20),
    open_slots: Some(1),
    distance: Some(steam::LobbyDistance::Default),
    string_filters: vec![steam::LobbyStringFilter::new(
        steam::LobbyDataKey::Mode,
        "coop",
        steam::LobbyStringFilterKind::Equal,
    )],
    ..Default::default()
};
steam::lobbies::request_list(search)?;
```

Lobby ids use `steam::LobbyId`.

Use `LobbyId::get_id()` and `LobbyId::from_id(id)` for save/network boundaries.

## Events

Perro pumps Steam callbacks during runtime updates.

Scripts poll queued Steam events:

```rust
for event in steam::events::drain()? {
    match event {
        steam::SteamEvent::LobbyList { lobbies } => {
            log_info!("Found {} Steam lobbies", lobbies.len());
        }
        steam::SteamEvent::LobbyChat { lobby, chat_id, .. } => {
            let bytes = steam::lobbies::read_chat(lobby, chat_id)?;
            let text = String::from_utf8_lossy(&bytes);
            log_info!("Steam lobby chat: {text}");
        }
        _ => {}
    }
}
```

## Gameplay API

| Use                | Function                                   | Macro                                   |
| ------------------ | ------------------------------------------ | --------------------------------------- |
| Own account name   | `steam::account::get_self_name()`          | `steam_account_self_name!()`            |
| User name by id    | `steam::account::get_name(id)`             | `steam_account_name!(id)`               |
| Own account id     | `steam::account::get_self_id()`            | `steam_account_self_id!()`              |
| Friend list        | `steam::friends::list()`                   | `steam_friend_list!()`                  |
| Rich presence      | `steam::friends::set_rich_presence(k, v)`  | `steam_rich_presence_set!(k, v)`        |
| Create lobby       | `steam::lobbies::create(kind, max)`        | `steam_lobby_create!(kind, max)`        |
| Join lobby         | `steam::lobbies::join(id)`                 | `steam_lobby_join!(id)`                 |
| Leave lobby        | `steam::lobbies::leave(id)`                | `steam_lobby_leave!(id)`                |
| Set lobby data     | `steam::lobbies::set_data(id, key, value)` | `steam_lobby_data_set!(id, key, value)` |
| Send lobby chat    | `steam::lobbies::send_chat(id, msg)`       | `steam_lobby_chat!(id, msg)`            |
| Drain Steam events | `steam::events::drain()`                   | `steam_events!()`                       |

## Errors

All Steam calls return `Result<T, steam::SteamError>`.

For gameplay code, treat Steam failure as non-fatal:

```rust
if let Err(err) = steam_ach_unlock!("ACH_FIRST_WIN") {
    log_warn!("Steam achievement failed: {err}");
}
```

Common errors:

| Error                       | Meaning                                                            |
| --------------------------- | ------------------------------------------------------------------ |
| `SteamError::Disabled`      | Steam is off in `project.toml`.                                    |
| `SteamError::NotReady`      | Steam did not initialize.                                          |
| `SteamError::CallFailed(_)` | Steam rejected the call. Check the achievement id and Steam setup. |
