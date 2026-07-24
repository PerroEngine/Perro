# Steamworks

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |
| Avatars | [Avatars](#avatars) |
| Steam Input | [Steam Input](#steam-input) |

## Purpose

Steamworks connects a Perro game to the Steam platform: achievements, stats,
leaderboards, cloud saves, friends and lobbies, rich presence, Workshop, and P2P
networking. Perro owns the plumbing (init, per-frame callback pump, and flushing
dirty stats/achievements), so scripts only call game actions through macros and
drain queued events. Most calls return `Result`, and when Steam is disabled they
return `SteamError::Disabled`, so the same code runs in non-Steam builds.

## Use Cases

- Milestone achievements and stats: unlock with `steam_ach_unlock!("ACH_WIN")`
  and track progress via `steam_stat_set_i32!` / `steam_stat_get_i32!`.
- Friends matchmaking: create a `LobbyType::FriendsOnly` lobby with
  `steam_lobby_create!`, then handle `LobbyCreated` / `LobbyJoined` from
  `steam_events!`.
- Cross-device saves: write and read progress with `steam_cloud_write!` /
  `steam_cloud_read!`.
- Competitive leaderboards: find or create a board and upload best scores with
  `steam_leaderboard_create!` / `steam_leaderboard_upload!`.
- Player-made content: subscribe to and download Workshop items with
  `steam_workshop_subscribe!` / `steam_workshop_download!`.
- Presence and invites: set status with `steam_rich_presence_set!` and react to
  `RichPresenceJoinRequested`.
- Friend avatars in UI: turn `steam_friend_avatar_large!` RGBA bytes into a
  runtime texture with `texture_create_from_rgba!`.

## Integration Boundary

Keep Steam calls in a platform/session owner. Convert callbacks into game-level
events before feature scripts react. This keeps offline fallback and non-Steam
targets from leaking platform checks into every gameplay script.

Use Steam lobbies/cloud/input when the shipped platform service owns that
capability. Use Perro LAN, `user://`, or normal input actions when the feature
must work without Steam. Steamworks remains native-only.

## Example

```rust
methods!({
    // Called from a game signal when the player wins their first match.
    fn on_first_win(&self, _ctx: &mut ScriptContext<'_, API>) {
        // Unlock an achievement and bump a stat; the engine flushes both.
        let _ = steam_ach_unlock!("ACH_FIRST_WIN");
        let wins = steam_stat_get_i32!("wins").unwrap_or(0);
        let _ = steam_stat_set_i32!("wins", wins + 1);
    }
});
```

## Reference

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
input = "off"
```

Use `480` for local Steamworks tests.

When Steam cfg disabled, Steam calls return `Err(steam::SteamError::Disabled)`.

`input` controls Steam Input access:

- `"off"`: no Steam Input init; keep native Perro input only.
- `"metadata"`: init Steam Input for controller type/glyph/origin/motion metadata, but action reads stay disabled.
- `"fallback"`: route Steam-only controllers into Perro `GamepadState`; keep natively supported controllers on Perro input.
- `"actions"`: init Steam Input and allow Steam Input action reads.

Use `"off"`, `"metadata"`, or `"fallback"` when Joy-Con / Joy-Con 2 stay on Perro custom input.
Use `"actions"` only when the game opts into Steam Input action maps.

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
| `steam_friend_avatar!(id, size)` | read friend avatar RGBA bytes |
| `steam_friend_avatar_small!(id)` | read 32x32 avatar RGBA bytes |
| `steam_friend_avatar_medium!(id)` | read 64x64 avatar RGBA bytes |
| `steam_friend_avatar_large!(id)` | read 184x184 avatar RGBA bytes |
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
- `input`
- `leaderboards`
- `lobbies`
- `networking`
- `stats`
- `utils`
- `workshop`

No script API for Steam init.
No script API for callback pump.
No script API for stat store.

## Avatars

Friend avatar calls return `Option<steam::SteamAvatar>`.

`None` means Steam has no avatar data ready.
Call `steam::friends::request_user_information(id, false)?` and try again later.

`SteamAvatar` fields:

- `width`: pixel width
- `height`: pixel height
- `rgba`: RGBA8 bytes

Use `texture_create_from_rgba!` to turn avatar bytes into a runtime texture.

```rust
let user = steam::SteamID::from_id(raw_user);

if let Some(avatar) = steam_friend_avatar_large!(user)? {
    let texture = texture_create_from_rgba!(
        ctx.res,
        avatar.width,
        avatar.height,
        avatar.rgba.as_slice(),
    );
    let _ = texture;
}
```

See [Runtime Bytes Resources](../resources/runtime_bytes.md).

## Steam Input

Steam Input is opt-in through `[steam].input`.

Perro supports four modes:

| Value | Init Steam Input | Action reads | Use |
| --- | --- | --- | --- |
| `"off"` | no | no | Default; Steam does not own controller input. |
| `"metadata"` | yes | no | Read connected Steam controller metadata, glyphs, origins, and motion. |
| `"fallback"` | yes | yes | Feed unsupported Steam controllers into normal Perro gamepad slots. |
| `"actions"` | yes | yes | Use Steam Input action sets and action data. |

Joy-Con and Joy-Con 2 custom input should use `"off"`, `"metadata"`, or `"fallback"`.
That keeps `ctx.ipt.JoyCons()` as the gameplay source.

Fallback policy:

- Keep Xbox, PlayStation, Switch Pro, Steam Deck, Apple MFi, Android, and all Joy-Con types native.
- Use Steam Controller, generic, and unknown types only when no native gamepad is present.
- Use Steam mobile touch even when a native gamepad is present.
- Merge fallback data into `ctx.ipt.Gamepads()` and normal gamepad actions.
- Move a fallback pad to a free gamepad slot if a native pad claims its old slot.

Fallback uses this Steam Input action contract:

- action set: `perro_gamepad`
- digital actions: `perro_bottom`, `perro_right`, `perro_left`, `perro_top`,
  `perro_dpad_up`, `perro_dpad_down`, `perro_dpad_left`, `perro_dpad_right`,
  `perro_start`, `perro_select`, `perro_home`, `perro_capture`, `perro_l1`,
  `perro_r1`, `perro_l2`, `perro_r2`, `perro_l3`, `perro_r3`
- analog actions: `perro_left_stick`, `perro_right_stick`,
  `perro_left_trigger`, `perro_right_trigger`

Add these names to the app Steam Input action manifest and default controller
layout. Missing `perro_gamepad` makes fallback return no pad data.

Steam Input metadata calls:

- `steam::input::mode()`
- `steam::input::get_connected_controllers()`
- `steam::input::get_controller_info()`
- `steam::input::input_type(handle)`
- `steam::input::input_type_is_joycon(kind)`
- `steam::input::digital_action_origins(handle, set, action)`
- `steam::input::analog_action_origins(handle, set, action)`
- `steam::input::glyph_for_action_origin(origin)`
- `steam::input::string_for_action_origin(origin)`
- `steam::input::motion_data(handle)`

Steam Input action calls require `input = "actions"`:

- `steam::input::is_action_manifest_set(path)`
- `steam::input::is_binding_panel_shown(handle)`
- `steam::input::action_set_handle(name)`
- `steam::input::activate_action_set(handle, set)`
- `steam::input::digital_action_handle(name)`
- `steam::input::analog_action_handle(name)`
- `steam::input::digital_action_data(handle, action)`
- `steam::input::analog_action_data(handle, action)`

Example metadata-only Joy-Con filter:

```rust
for controller in steam::input::get_controller_info()? {
    if controller.is_joycon {
        continue;
    }

    let _kind = controller.input_type;
}
```

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

let upload = steam::workshop::start_update(
    steam::AppID::from_id(480),
    file,
)?
.title("My mod")
.description("Adds a new map")
.content_path("mods/my_mod")
.preview_path("mods/my_mod.png")
.visibility(steam::workshop::Visibility::Public)
.tags(vec!["Map", "Co-op"], false)
.submit(Some("First release"), |result| {
    let _ = result;
});

let (status, sent, total) = upload.progress();
let _ = (status, sent, total);

steam::workshop::query_all(
    steam::workshop::QueryType::RankedByVote,
    steam::workshop::ItemType::Items,
    steam::workshop::QueryAppIDs::Consumer(steam::AppID::from_id(480)),
    1,
)?
.search_text("co-op")
.require_tag("Map")
.include_metadata(true)
.include_children(true)
.include_key_value_tags(true)
.fetch(|result| {
    let _ = result.map(|page| page.items);
});
```

Use:

- `steam::workshop::FileType`
- `steam::workshop::ItemState`
- `steam::workshop::InstallInfo`
- `steam::workshop::CreateItemResult`
- `steam::workshop::Update` and `UpdateWatch`
- `steam::workshop::Query`, `QueryPage`, and `QueryItem`
- `steam::workshop::QueryType`, `ItemType`, and `QueryAppIDs`
- `steam::workshop::Visibility`

Workshop also wraps delete, download, subscribe, playtime tracking, game-server init,
content descriptors, metadata, tags, key-value tags, and all Steamworks-rs UGC query filters.

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
