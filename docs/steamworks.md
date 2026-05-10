# Steamworks

Use Steam from scripts through `perro_api::prelude::*`.

Perro wraps Steamworks.
Scripts call game actions.
Engine init, pump callbacks, + store stats/achs.

## Config

Add Steam cfg to `project.toml`:

```toml
[steam]
enabled = true
app_id = 480
```

Use `480` for local Steamworks tests.

When Steam cfg disabled, Steam calls return `Err(steam::SteamError::Disabled)`.

## Frame Model

Perro init Steam from project cfg.
Perro pump Steam callbacks each runtime update.
Perro store dirty stats/achs after callback pump.

Do not call init.
Do not call callback pump.
Do not call stat store.

Use action macros.
They queue/apply work.
Engine flushes once/frame.

## Common Use

```rust
use perro_api::prelude::*;

steam_ach_unlock!("ACH_FIRST_WIN")?;
steam_stat_set_i32!("wins", 10)?;

let wins = steam_stat_get_i32!("wins")?;
let name = steam_account_self_name!()?;

steam_rich_presence_set!(steam::RichPresenceKey::Status, "In match")?;
steam_lobby_create!(steam::LobbyType::FriendsOnly, 4)?;

for event in steam_events!()? {
    match event {
        steam::SteamEvent::LobbyCreated { lobby } => {
            steam_lobby_data_set!(lobby, steam::LobbyDataKey::Mode, "coop")?;
        }
        steam::SteamEvent::LobbyJoined { lobby } => {
            steam_lobby_chat!(lobby, "hello")?;
        }
        _ => {}
    }
}
```

## IDs

Use small typed wrappers.

```rust
let app = steam::AppID::from_id(480);
let dlc = steam::DLCID::from_id(12345);
let user = steam::SteamID::from_id(raw_user);
let lobby = steam::LobbyID::from_id(raw_lobby);
let file = steam::WorkshopFileID::from_id(raw_file);
```

## Macros

| Macro | Action |
| --- | --- |
| `steam_ach_unlock!(id)` | unlock ach + mark store dirty |
| `steam_ach_unlock!(a, b, ...)` | unlock many achs + mark store dirty |
| `steam_ach_clear!(id)` | clear ach + mark store dirty |
| `steam_account_self_name!()` | read local user name |
| `steam_account_self_id!()` | read local user id |
| `steam_account_name!(id)` | read user name |
| `steam_friend_list!()` | read friend list |
| `steam_rich_presence_set!(key, val)` | set rich presence |
| `steam_lobby_create!(kind, max)` | request lobby create |
| `steam_lobby_join!(id)` | request lobby join |
| `steam_lobby_leave!(id)` | leave lobby |
| `steam_lobby_data_set!(id, key, val)` | set lobby data |
| `steam_lobby_chat!(id, msg)` | send lobby chat |
| `steam_events!()` | drain queued Steam events |
| `steam_app_dlc_installed!(dlc)` | check DLC install |
| `steam_app_subscribed!()` | check current app subscribe |
| `steam_app_subscribed!(app)` | check app subscribe |
| `steam_stat_get_i32!(name)` | read i32 stat |
| `steam_stat_set_i32!(name, val)` | set i32 stat + mark store dirty |
| `steam_leaderboard_find!(name, cb)` | find leaderboard |
| `steam_leaderboard_create!(name, sort, display, cb)` | find or create leaderboard |
| `steam_leaderboard_upload!(board, score, cb)` | upload score, keep best |
| `steam_leaderboard_upload!(board, mode, score, details, cb)` | upload score with mode + details |
| `steam_leaderboard_entries!(board, start, end, cb)` | get global entries |
| `steam_leaderboard_entries!(board, scope, start, end, details_len, cb)` | get scoped entries |
| `steam_cloud_read!(name)` | read cloud file bytes |
| `steam_cloud_write!(name, bytes)` | write cloud file |
| `steam_workshop_subscribe!(file, cb)` | request workshop subscribe |
| `steam_workshop_download!(file, high)` | request workshop download |
| `steam_p2p_send!(user, send_type, data)` | send P2P packet |
| `steam_p2p_send!(user, send_type, data, channel)` | send P2P packet on channel |
| `steam_p2p_read!(max_size)` | read P2P packet |
| `steam_p2p_read!(max_size, channel)` | read P2P packet from channel |

## Facade Modules

`perro_api::steam` exposes safe wrapper modules:

- `account`
- `achievements`
- `apps`
- `cloud`
- `events`
- `friends`
- `leaderboards`
- `lobbies`
- `networking`
- `stats`
- `utils`
- `workshop`

No script API for Steam init.
No script API for callback pump.
No script API for stat store.

## Achievements + Stats

Achievements and stats share Steam store.

```rust
steam_ach_unlock!("ACH_WIN")?;
steam_stat_set_i32!("wins", 4)?;
```

Both mark store dirty.
Runtime stores once on next update.

Reads stay explicit:

```rust
let done = steam::stats::achievement_unlocked("ACH_WIN")?;
let wins = steam_stat_get_i32!("wins")?;
```

## Lobbies

Create/join calls return after request queue.
Results arrive as events.

```rust
steam_lobby_create!(steam::LobbyType::FriendsOnly, 4)?;

for event in steam_events!()? {
    match event {
        steam::SteamEvent::LobbyCreated { lobby } => {
            steam_lobby_data_set!(lobby, steam::LobbyDataKey::Name, "Room")?;
        }
        steam::SteamEvent::LobbyCreateFailed => {}
        _ => {}
    }
}
```

## Leaderboards

Leaderboard API uses Perro wrapper types.
No raw Steam handles in script code.

```rust
steam_leaderboard_create!(
    "wins",
    steam::LeaderboardSort::Descending,
    steam::LeaderboardDisplay::Numeric,
    |result| {
        if let Ok(Some(board)) = result {
            let _ = steam_leaderboard_upload!(&board, 10, |_| {});
            let _ = steam_leaderboard_entries!(&board, 1, 10, |entries| {
                for entry in entries.unwrap_or_default() {
                    let user = entry.user;
                    let score = entry.score;
                    let rank = entry.global_rank;
                    let _ = (user, score, rank);
                }
            });
        }
    },
)?;
```

Use:

- `steam::LeaderboardID`
- `steam::LeaderboardEntry`
- `steam::LeaderboardScoreUpload`
- `steam::LeaderboardSort`
- `steam::LeaderboardDisplay`
- `steam::LeaderboardUploadMode`
- `steam::LeaderboardEntryScope`

## Cloud

Cloud returns Perro file info structs.
No raw Steam file/platform types.

```rust
steam_cloud_write!("save.bin", b"save data")?;
let bytes = steam_cloud_read!("save.bin")?;

for file in steam::cloud::get_files()? {
    let name = file.name;
    let size = file.size;
    let _ = (name, size);
}
```

## Workshop

Workshop callbacks return `steam::SteamError`.
Workshop item data uses Perro wrappers.

```rust
let file = steam::WorkshopFileID::from_id(123);

steam_workshop_subscribe!(file, |result| {
    if result.is_ok() {
        // subscribed
    }
})?;

let state = steam::workshop::get_state(file)?;
if state.installed && !state.needs_update {
    let info = steam::workshop::get_install_info(file)?;
    let _ = info.map(|info| info.folder);
}
```

Use:

- `steam::workshop::FileType`
- `steam::workshop::ItemState`
- `steam::workshop::InstallInfo`
- `steam::workshop::CreateItemResult`

## Auth

Auth uses Perro ticket + result enums.

```rust
let (ticket, bytes) = steam::auth::authentication_session_ticket()?;
let user = steam_account_self_id!()?;

match steam::auth::begin_authentication_session(user, &bytes)? {
    Ok(()) => {}
    Err(err) => {
        let _ = err;
    }
}

steam::auth::cancel_authentication_ticket(ticket)?;
steam::auth::end_authentication_session(user)?;
```

## Overlay

```rust
steam::friends::open_overlay(steam::OverlayDialog::Friends)?;
steam::friends::open_store(
    steam::AppID::from_id(480),
    steam::StoreOverlayAction::Open,
)?;
```

## Events

`steam_events!()` drains queued callback events.

```rust
for event in steam_events!()? {
    match event {
        steam::SteamEvent::LobbyList { lobbies } => {}
        steam::SteamEvent::LobbyListFailed => {}
        steam::SteamEvent::LobbyCreated { lobby } => {}
        steam::SteamEvent::LobbyCreateFailed => {}
        steam::SteamEvent::LobbyJoined { lobby } => {}
        steam::SteamEvent::LobbyJoinFailed { lobby } => {}
        steam::SteamEvent::LobbyDataUpdated { lobby, member } => {}
        steam::SteamEvent::LobbyChat { lobby, user, chat_id } => {}
        steam::SteamEvent::LobbyMemberChanged { lobby, user } => {}
        steam::SteamEvent::LobbyJoinRequested { lobby, friend } => {}
        steam::SteamEvent::RichPresenceJoinRequested { friend, connect } => {}
        steam::SteamEvent::PersonaChanged { user } => {}
        steam::SteamEvent::OverlayChanged { active } => {}
        steam::SteamEvent::Callback { name } => {}
    }
}
```
