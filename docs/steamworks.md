# Steamworks

Use Steamworks from game scripts through `perro_api::prelude::*`.

## Setup

Add Steam config to `project.toml`:

```toml
[steam]
enabled = true
app_id = 480
```

Perro initializes Steam from project config.

Perro runs Steam callbacks each runtime update.

Game scripts do not need init, shutdown, or callback pump code.

Use real app id for shipped games.

Use `480` for local Steamworks tests.

When Steam is disabled, Steam calls return `Err(steam::SteamError::Disabled)`.

## API Shape

Main Rust path:

```rust
steam::apps::is_dlc_id_installed(steam::DLCID::from_id(12345))?;
steam::lobbies::create(steam::LobbyType::FriendsOnly, 4)?;
steam::cloud::write("save.bin", b"save data")?;
```

Friendly macro path:

```rust
steam_app_dlc_installed!(steam::DLCID::from_id(12345))?;
steam_lobby_create!(steam::LobbyType::FriendsOnly, 4)?;
steam_cloud_write!("save.bin", b"save data")?;
```

ID values:

```rust
let app = steam::AppID::from_id(480);
let dlc = steam::DLCID::from_id(12345);
let user = steam::SteamID::from_id(raw_user);
let lobby = steam::LobbyID::from_id(raw_lobby);
let workshop_file = steam::WorkshopFileID::from_id(raw_file);
```

## Macros

| Macro                                                       | Calls                                                       |
| ----------------------------------------------------------- | ----------------------------------------------------------- |
| `steam_unlock!(id)`                                         | `steam::achievements::unlock(id)`                           |
| `steam_clear!(id)`                                          | `steam::achievements::clear(id)`                            |
| `steam_ach_unlock!(id)`                                     | `steam::achievements::unlock_input(id)`                     |
| `steam_ach_unlock!(a, b, ...)`                              | `steam::achievements::unlock_many([...])`                   |
| `steam_ach_clear!(id)`                                      | `steam::achievements::clear(id)`                            |
| `steam_account_name!(id)`                                   | `steam::account::get_name(id)`                              |
| `steam_account_self_name!()`                                | `steam::account::get_self_name()`                           |
| `steam_account_self_id!()`                                  | `steam::account::get_self_id()`                             |
| `steam_friend_list!()`                                      | `steam::friends::get_list()`                                |
| `steam_rich_presence_set!(key, value)`                      | `steam::friends::set_rich_presence(key, value)`             |
| `steam_lobby_create!(kind, max)`                            | `steam::lobbies::create(kind, max)`                         |
| `steam_lobby_join!(id)`                                     | `steam::lobbies::join(id)`                                  |
| `steam_lobby_leave!(id)`                                    | `steam::lobbies::leave(id)`                                 |
| `steam_lobby_data_set!(id, key, value)`                     | `steam::lobbies::set_data(id, key, value)`                  |
| `steam_lobby_chat!(id, msg)`                                | `steam::lobbies::send_chat(id, msg)`                        |
| `steam_events!()`                                           | `steam::events::drain()`                                    |
| `steam_app_dlc_installed!(dlc_id)`                          | `steam::apps::is_dlc_id_installed(dlc_id)`                  |
| `steam_app_subscribed!()`                                   | `steam::apps::is_subscribed()`                              |
| `steam_app_subscribed!(app_id)`                             | `steam::apps::is_subscribed_app(app_id)`                    |
| `steam_stat_get_i32!(name)`                                 | `steam::stats::get_i32(name)`                               |
| `steam_stat_set_i32!(name, value)`                          | `steam::stats::set_i32(name, value)`                        |
| `steam_stat_store!()`                                       | `steam::stats::store()`                                     |
| `steam_leaderboard_upload!(lb, method, score, details, cb)` | `steam::leaderboards::upload(...)`                          |
| `steam_leaderboard_entries!(lb, req, start, end, len, cb)`  | `steam::leaderboards::entries(...)`                         |
| `steam_cloud_read!(name)`                                   | `steam::cloud::get_file_bytes(name)`                        |
| `steam_cloud_write!(name, bytes)`                           | `steam::cloud::write(name, bytes)`                          |
| `steam_workshop_subscribe!(file, cb)`                       | `steam::workshop::subscribe(file, cb)`                      |
| `steam_workshop_download!(file, high_priority)`             | `steam::workshop::is_download_started(file, high_priority)` |
| `steam_p2p_send!(user, send_type, data)`                    | `steam::networking::is_p2p_sent(user, send_type, data)`     |
| `steam_p2p_send!(user, send_type, data, channel)`           | `steam::networking::is_p2p_sent_on_channel(...)`            |
| `steam_p2p_read!(max_size)`                                 | `steam::networking::get_p2p_packet(max_size)`               |
| `steam_p2p_read!(max_size, channel)`                        | `steam::networking::get_p2p_packet_from_channel(...)`       |

## `steam::app`

Runtime Steam state.

| Function                            | Use                                                   |
| ----------------------------------- | ----------------------------------------------------- |
| `init_from_config(enabled, app_id)` | Init Steam from project config. Runtime calls this.   |
| `run_callbacks()`                   | Pump Steam callbacks. Runtime calls this each update. |
| `enabled()`                         | Return project Steam enabled state.                   |
| `ready()`                           | Return true when Steam client exists.                 |
| `get_app_id()`                      | Return active app id if initialized.                  |

## `steam::apps`

App, DLC, entitlement, launch, language, and ownership info.

| Function                            | Use                                       |
| ----------------------------------- | ----------------------------------------- |
| `is_installed(app_id)`              | Check if target app installed.            |
| `is_dlc_installed(app_id)`          | Check DLC install/ownership by `AppID`.   |
| `is_dlc_id_installed(dlc_id)`       | Check DLC install/ownership by `DLCID`.   |
| `is_subscribed()`                   | Check current app ownership/subscription. |
| `is_subscribed_app(app_id)`         | Check related app ownership/subscription. |
| `is_subscribed_from_free_weekend()` | Check free weekend license.               |
| `is_vac_banned()`                   | Check account VAC status for app.         |
| `is_cybercafe()`                    | Check cybercafe license.                  |
| `is_low_violence()`                 | Check low-violence depot license.         |
| `get_build_id()`                    | Read app build id.                        |
| `get_install_dir(app_id)`           | Read install folder for app.              |
| `get_owner()`                       | Read original app owner Steam id.         |
| `get_available_languages()`         | Read available game languages.            |
| `get_current_language()`            | Read current game language.               |
| `get_current_beta_name()`           | Read beta branch name.                    |
| `get_launch_command_line()`         | Read Steam URL launch command line.       |
| `get_launch_query_param(key)`       | Read Steam URL query parameter.           |

Example:

```rust
let dlc = steam::DLCID::from_id(12345);
if steam_app_dlc_installed!(dlc)? {
    log_info!("DLC owned + installed");
}
```

## `steam::account`

Current user and Steam friend name helpers.

| Function          | Use                                   |
| ----------------- | ------------------------------------- |
| `get_self_id()`   | Return current user `SteamID`.        |
| `get_name(id)`    | Return display name for user id.      |
| `get_self_name()` | Return current user display name.     |
| `get_level()`     | Return current user Steam level.      |
| `is_logged_on()`  | Return Steam server connection state. |

## `steam::auth`

Auth tickets, session validation, and license check.

| Type               | Meaning                   |
| ------------------ | ------------------------- |
| `AuthTicket`       | Steam auth ticket handle. |
| `AuthSessionError` | Auth session start error. |
| `UserHasLicense`   | User license result enum. |

| Function                                              | Use                                        |
| ----------------------------------------------------- | ------------------------------------------ |
| `authentication_session_ticket()`                     | Create ticket for current user identity.   |
| `authentication_session_ticket_with_steam_id(remote)` | Create ticket scoped to remote Steam user. |
| `cancel_authentication_ticket(ticket)`                | Cancel ticket handle.                      |
| `begin_authentication_session(user, ticket)`          | Validate remote ticket bytes.              |
| `end_authentication_session(user)`                    | End remote auth session.                   |
| `authentication_session_ticket_for_webapi(identity)`  | Create Web API auth ticket.                |
| `user_has_license_for_app(user, app_id)`              | Check user license for app.                |

## `steam::achievements`

Achievement write helpers.

| Function              | Use                                                   |
| --------------------- | ----------------------------------------------------- |
| `unlock(id)`          | Unlock one achievement.                               |
| `unlock_many(ids)`    | Unlock many achievements.                             |
| `clear(id)`           | Clear one achievement.                                |
| `unlock_input(input)` | Unlock using one id, array, slice, or vec-like input. |

Example:

```rust
steam_ach_unlock!("ACH_FIRST_WIN")?;
steam_ach_clear!("ACH_FIRST_WIN")?;
```

## `steam::stats`

Achievement read state and stat read/write.

| Function                      | Use                                      |
| ----------------------------- | ---------------------------------------- |
| `achievement_unlocked(id)`    | Return achievement unlocked bool.        |
| `achievement_unlock_time(id)` | Return unlocked bool + unlock unix time. |
| `achievement_percent(id)`     | Return global achieved percent.          |
| `achievement_names()`         | Return achievement ids from Steam.       |
| `get_i32(name)`               | Read `i32` stat.                         |
| `set_i32(name, value)`        | Write `i32` stat.                        |
| `get_f32(name)`               | Read `f32` stat.                         |
| `set_f32(name, value)`        | Write `f32` stat.                        |
| `global_i64(name)`            | Read global `i64` stat.                  |
| `global_f64(name)`            | Read global `f64` stat.                  |
| `store()`                     | Store stats/achievements to Steam.       |
| `reset_all(achievements_too)` | Reset stats and optionally achievements. |

Example:

```rust
steam_stat_set_i32!("wins", 10)?;
let wins = steam_stat_get_i32!("wins")?;
steam_stat_store!()?;
```

## `steam::leaderboards`

Leaderboard find, create, upload, and download.

| Type                       | Meaning                    |
| -------------------------- | -------------------------- |
| `LeaderboardID`            | Steam leaderboard handle.  |
| `LeaderboardEntry`         | Downloaded score entry.    |
| `LeaderboardDataRequest`   | Entry request scope.       |
| `LeaderboardDisplayType`   | Numeric/time display mode. |
| `LeaderboardSortMethod`    | Asc/desc score order.      |
| `LeaderboardScoreUploaded` | Upload result.             |
| `UploadScoreMethod`        | Keep best or force update. |

| Function                                                         | Use                                                      |
| ---------------------------------------------------------------- | -------------------------------------------------------- |
| `find(name, cb)`                                                 | Find leaderboard by name. Callback gets optional handle. |
| `find_or_create(name, sort, display, cb)`                        | Find or create leaderboard.                              |
| `upload(leaderboard, method, score, details, cb)`                | Upload score.                                            |
| `entries(leaderboard, request, start, end, max_details_len, cb)` | Download score entries.                                  |

Example:

```rust
steam::leaderboards::find("wins", |result| {
    if let Ok(Some(board)) = result {
        // keep board handle for upload/download
    }
})?;
```

## `steam::friends`

Friends, rich presence, overlay, and invites.

| Function                             | Use                               |
| ------------------------------------ | --------------------------------- |
| `list()`                             | List normal friends.              |
| `list_by(kind)`                      | List friends by `FriendListKind`. |
| `get(id)`                            | Return `FriendInfo`.              |
| `rich_presence(id, key)`             | Read friend rich presence value.  |
| `set_rich_presence(key, value)`      | Set current user rich presence.   |
| `clear_rich_presence()`              | Clear current user rich presence. |
| `open_overlay(dialog)`               | Open Steam overlay dialog.        |
| `open_user_overlay(dialog, user)`    | Open overlay focused on user.     |
| `open_store(app_id, action)`         | Open Steam store overlay for app. |
| `open_web_page(url)`                 | Open overlay browser to URL.      |
| `open_invite_dialog(lobby)`          | Open Steam lobby invite dialog.   |
| `invite_user_to_game(user, connect)` | Invite user with connect string.  |

Example:

```rust
let friends = steam_friend_list!()?;
steam_rich_presence_set!(steam::RichPresenceKey::Status, "In menu")?;
steam::friends::open_overlay(steam::OverlayDialog::Friends)?;
steam::friends::open_store(
    steam::AppID::from_id(480),
    steam::StoreOverlayAction::Open,
)?;
```

## `steam::lobbies`

Lobby create/search/join/data/chat.

| Function                              | Use                                               |
| ------------------------------------- | ------------------------------------------------- |
| `create(kind, max_members)`           | Request async lobby create. Event reports result. |
| `request_list(search)`                | Request async lobby list. Event reports result.   |
| `join(lobby)`                         | Request async lobby join. Event reports result.   |
| `leave(lobby)`                        | Leave lobby.                                      |
| `set_data(lobby, key, value)`         | Set lobby key/value data.                         |
| `get_data(lobby, key)`                | Read lobby key value.                             |
| `all_data(lobby)`                     | Read all lobby key/value pairs.                   |
| `members(lobby)`                      | List lobby members.                               |
| `get_owner(lobby)`                    | Read lobby owner.                                 |
| `info(lobby)`                         | Read owner, members, limit, data, game server.    |
| `set_joinable(lobby, joinable)`       | Set lobby open/closed by bool.                    |
| `set_joinability(lobby, joinability)` | Set lobby open/closed by enum.                    |
| `send_chat(lobby, message)`           | Send lobby chat bytes/text.                       |
| `read_chat(lobby, chat_id)`           | Read lobby chat bytes for event chat id.          |

Example:

```rust
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

## `steam::networking`

Steam legacy P2P packet API.

| Type              | Meaning                          |
| ----------------- | -------------------------------- |
| `SendType`        | Reliable/unreliable packet mode. |
| `P2PSessionState` | P2P connection state.            |

| Function                                                 | Use                                   |
| -------------------------------------------------------- | ------------------------------------- |
| `is_p2p_session_accepted(user)`                          | Accept incoming P2P session.          |
| `is_p2p_session_closed(user)`                            | Close P2P session.                    |
| `get_session_state(user)`                                | Read session state for user.          |
| `is_p2p_sent(user, send_type, data)`                     | Send packet on channel 0.             |
| `is_p2p_sent_on_channel(user, send_type, data, channel)` | Send packet on channel.               |
| `get_p2p_available()`                                    | Return next packet size on channel 0. |
| `get_p2p_available_on_channel(channel)`                  | Return next packet size on channel.   |
| `get_p2p_packet(max_size)`                               | Read packet on channel 0.             |
| `get_p2p_packet_from_channel(max_size, channel)          | Read packet on channel.               |

Example:

```rust
steam_p2p_send!(peer, steam::networking::SendType::Reliable, b"hello")?;

if let Some((sender, bytes)) = steam_p2p_read!(4096)? {
    log_info!("P2P packet: {} bytes", bytes.len());
}
```

## `steam::networking_messages`

Steam identity message API.

| Type                        | Meaning                    |
| --------------------------- | -------------------------- |
| `SendFlags`                 | Message delivery flags.    |
| `NetworkingMessage`         | Received message.          |
| `NetworkingIdentity`        | Steam networking identity. |
| `NetworkingConnectionState` | Session state enum.        |
| `NetConnectionInfo`         | Session connection info.   |
| `NetConnectionRealTimeInfo` | Realtime connection stats. |

| Function                                        | Use                              |
| ----------------------------------------------- | -------------------------------- |
| `identity_steam_id(id)`                         | Build identity from `SteamID`.   |
| `send_to_user(user, send_flags, data, channel)` | Send message to Steam user.      |
| `receive(channel, batch_size)`                  | Receive messages on channel.     |
| `get_session_info(user)`                        | Read state, info, realtime info. |

## `steam::networking_sockets`

Steam Networking Sockets API.

| Type                          | Meaning               |
| ----------------------------- | --------------------- |
| `ListenSocket`                | Listen socket handle. |
| `NetConnection`               | Connection handle.    |
| `NetPollGroup`                | Poll group handle.    |
| `NetworkingConfigEntry`       | Socket config entry.  |
| `NetworkingIdentity`          | Remote identity.      |
| `NetworkingAvailability`      | Availability state.   |
| `NetworkingAvailabilityError` | Availability error.   |

| Function                                              | Use                             |
| ----------------------------------------------------- | ------------------------------- |
| `listen_ip(addr, options)`                            | Create IP listen socket.        |
| `connect_ip(addr, options)`                           | Connect to IP socket.           |
| `listen_p2p(local_virtual_port, options)`             | Create P2P listen socket.       |
| `connect_p2p(identity, remote_virtual_port, options)` | Connect to P2P identity.        |
| `init_authentication()`                               | Init networking auth resources. |
| `auth_status()`                                       | Read networking auth status.    |

## `steam::networking_utils`

Steam relay/network utility API.

| Type                          | Meaning                   |
| ----------------------------- | ------------------------- |
| `NetworkingAvailability`      | Relay availability.       |
| `NetworkingAvailabilityError` | Relay availability error. |
| `RelayNetworkStatus`          | Detailed relay status.    |

| Function                              | Use                         |
| ------------------------------------- | --------------------------- |
| `init_relay_network_access()`         | Start relay network init.   |
| `get_relay_network_status()`          | Read summary relay status.  |
| `get_detailed_relay_network_status()` | Read detailed relay status. |

## `steam::servers`

Server browser helper API.

| Type                   | Meaning                                       |
| ---------------------- | --------------------------------------------- |
| `MatchmakingServers`   | Raw Steam matchmaking servers interface type. |
| `GameServerItem`       | Server list entry.                            |
| `ServerListRequest`    | Active server query handle.                   |
| `PingCallbacks`        | Ping callbacks.                               |
| `ServerRulesCallbacks` | Server rules callbacks.                       |

| Function                            | Use                   |
| ----------------------------------- | --------------------- |
| `ping_server(ip, port, callbacks)`  | Ping one game server. |
| `server_rules(ip, port, callbacks)` | Query server rules.   |

## `steam::cloud`

Steam Cloud file API.

| Type        | Meaning                    |
| ----------- | -------------------------- |
| `FileInfo`  | Cloud file name + size.    |
| `Platforms` | Cloud sync platform flags. |

| Function                       | Use                           |
| ------------------------------ | ----------------------------- |
| `set_enabled_for_app(enabled)` | Enable/disable cloud for app. |
| `is_enabled_for_app()`         | Read app cloud setting.       |
| `is_enabled_for_account()`     | Read account cloud setting.   |
| `get_files()`                  | List cloud files.             |
| `is_file_present(name)`        | Check file exists.            |
| `delete(name)`                 | Delete file.                  |
| `read(name)`                   | Read file bytes.              |
| `write(name, bytes)`           | Write file bytes.             |

Example:

```rust
steam_cloud_write!("save.bin", b"save data")?;
let bytes = steam_cloud_read!("save.bin")?;
```

## `steam::workshop`

Steam Workshop / UGC API.

| Type           | Meaning                                    |
| -------------- | ------------------------------------------ |
| `FileType`     | Workshop item file type.                   |
| `ItemState`    | Subscribed/installed/download state flags. |
| `InstallInfo`  | Install folder, size, timestamp.           |
| `QueryHandle`  | Active UGC query builder.                  |
| `QueryResult`  | UGC query result entry.                    |
| `UGCQueryType` | Query sort/filter kind.                    |
| `UGCType`      | UGC item category.                         |
| `AppIDs`       | Creator/consumer app id query filter.      |

| Function                                   | Use                                |
| ------------------------------------------ | ---------------------------------- |
| `suspend_downloads(suspend)`               | Pause/resume Workshop downloads.   |
| `subscribe(file, cb)`                      | Subscribe to Workshop item.        |
| `unsubscribe(file, cb)`                    | Unsubscribe from Workshop item.    |
| `get_subscribed(include_locally_disabled)` | List subscribed item ids.          |
| `get_state(file)`                          | Read item state flags.             |
| `get_download_info(file)`                  | Read current/total download bytes. |
| `get_install_info(file)`                   | Read install info.                 |
| `is_download_started(file, high_priority)` | Start item download.               |
| `create(app_id, file_type, cb)`            | Create Workshop item.              |
| `get_query_item(file)`                     | Create query for one item.         |

Example:

```rust
let item = steam::WorkshopFileID::from_id(123);

steam_workshop_subscribe!(item, |result| {
    // handle result
})?;

steam_workshop_download!(item, true)?;
```

## `steam::input`

Steam Input helper API.

| Type        | Meaning          |
| ----------- | ---------------- |
| `InputType` | Controller type. |

| Function                               | Use                                |
| -------------------------------------- | ---------------------------------- |
| `is_init(explicitly_call_run_frame)`   | Init Steam Input.                  |
| `run_frame()`                          | Pump Steam Input frame.            |
| `get_connected_controllers()`          | Return controller handles.         |
| `is_action_manifest_set(path)`         | Set action manifest path.          |
| `is_binding_panel_shown(input_handle)` | Open binding panel for controller. |
| `shutdown()`                           | Shutdown Steam Input.              |

## `steam::remote_play`

Steam Remote Play helper API.

| Type                    | Meaning                        |
| ----------------------- | ------------------------------ |
| `RemotePlaySessionID`   | Remote Play session id.        |
| `RemotePlaySession`     | Remote Play session handle.    |
| `SteamDeviceFormFactor` | Phone/tablet/computer/TV enum. |

| Function                  | Use                               |
| ------------------------- | --------------------------------- |
| `get_sessions()`          | List active Remote Play sessions. |
| `get_session(session_id)` | Get one session handle.           |

## `steam::screenshots`

Steam screenshots API.

| Type               | Meaning                  |
| ------------------ | ------------------------ |
| `ScreenshotHandle` | Steam screenshot handle. |

| Function                                         | Use                                         |
| ------------------------------------------------ | ------------------------------------------- |
| `trigger()`                                      | Trigger screenshot flow.                    |
| `hook_screenshots(hook)`                         | Let game handle screenshot requests.        |
| `add_to_library(path, thumbnail, width, height)` | Add image file to Steam screenshot library. |

## `steam::timeline`

Steam Timeline API.

| Type                        | Meaning                            |
| --------------------------- | ---------------------------------- |
| `TimelineGameMode`          | Playing/staging/menu/loading mode. |
| `TimelineEventClipPriority` | Clip priority for event.           |

| Function                                                                               | Use                        |
| -------------------------------------------------------------------------------------- | -------------------------- |
| `set_game_mode(mode)`                                                                  | Set current timeline mode. |
| `set_state_description(description, duration)`                                         | Set timeline state text.   |
| `clear_state_description(duration)`                                                    | Clear state text.          |
| `add_event(icon, title, description, priority, start_offset, duration, clip_priority)` | Add timeline event.        |

## `steam::utils`

Steam utility API.

| Type                       | Meaning                       |
| -------------------------- | ----------------------------- |
| `NotificationPosition`     | Overlay notification corner.  |
| `GamepadTextInputMode`     | Gamepad text input mode.      |
| `GamepadTextInputLineMode` | Gamepad text input line mode. |

| Function                                      | Use                               |
| --------------------------------------------- | --------------------------------- |
| `get_app_id()`                                | Read current app id.              |
| `get_ip_country()`                            | Read current country code.        |
| `is_overlay_enabled()`                        | Check Steam overlay availability. |
| `get_ui_language()`                           | Read Steam UI language.           |
| `get_server_real_time()`                      | Read Steam server time.           |
| `set_overlay_notification_position(position)` | Set overlay toast position.       |
| `is_steam_deck()`                             | Check Steam Deck.                 |
| `is_big_picture()`                            | Check Big Picture mode.           |

## `steam::events`

Queued Steam callback events.

| Function     | Use             |
| ------------ | --------------- |
| `poll_one()` | Pop one event.  |
| `drain()`    | Pop all events. |
| `clear()`    | Clear queue.    |

Full match example:

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
        steam::SteamEvent::LobbyChat {
            lobby,
            user,
            chat_id,
        } => {}
        steam::SteamEvent::LobbyMemberChanged { lobby, user } => {}
        steam::SteamEvent::LobbyJoinRequested { lobby, friend } => {}
        steam::SteamEvent::RichPresenceJoinRequested { friend, connect } => {}
        steam::SteamEvent::PersonaChanged { user } => {}
        steam::SteamEvent::OverlayChanged { active } => {}
        steam::SteamEvent::Callback { name } => {}
    }
}
```

Callback names used by `SteamEvent::Callback`:

| Name                                    | Source                                   |
| --------------------------------------- | ---------------------------------------- |
| `auth_session_ticket_response`          | Auth ticket callback.                    |
| `ticket_for_webapi_response`            | Web API ticket callback.                 |
| `validate_auth_ticket_response`         | Auth validation callback.                |
| `steam_servers_connected`               | Steam server connected.                  |
| `steam_servers_disconnected`            | Steam server disconnected.               |
| `steam_server_connect_failure`          | Steam server connect failure.            |
| `microtxn_authorization_response`       | Microtransaction authorization response. |
| `p2p_session_request`                   | P2P session request.                     |
| `p2p_session_connect_fail`              | P2P connect failure.                     |
| `networking_messages_session_request`   | Networking messages session request.     |
| `networking_messages_session_failed`    | Networking messages session failed.      |
| `net_connection_status_changed`         | Networking sockets status changed.       |
| `relay_network_status`                  | Relay status update.                     |
| `download_item_result`                  | Workshop download result.                |
| `screenshot_requested`                  | Screenshot request.                      |
| `screenshot_ready`                      | Screenshot ready.                        |
| `remote_play_connected`                 | Remote Play connect.                     |
| `remote_play_disconnected`              | Remote Play disconnect.                  |
| `user_stats_received`                   | User stats received.                     |
| `user_stats_stored`                     | User stats stored.                       |
| `user_achievement_stored`               | Achievement stored.                      |
| `user_achievement_icon_fetched`         | Achievement icon fetched.                |
| `gamepad_text_input_dismissed`          | Gamepad text input done.                 |
| `floating_gamepad_text_input_dismissed` | Floating gamepad text input done.        |
| `gs_client_approve`                     | Game server client approved.             |
| `gs_client_deny`                        | Game server client denied.               |
| `gs_client_kick`                        | Game server client kicked.               |
| `gs_client_group_status`                | Game server group status.                |
| `new_url_launch_parameters`             | New Steam URL launch params.             |
