// Generic Perro networking layer: Steam/LAN transport split, session +
// slot lifecycle, lobby discovery. Game payloads are opaque bytes — games own
// their message types and codec, and drive this crate through send/broadcast +
// drained NetEvents.

pub mod client_session;
pub mod heartbeat;
pub mod host_session;
pub mod lan_transport;
pub mod lobby;
pub mod state;
pub mod steam_transport;
pub mod transport;
pub mod wire;

pub use heartbeat::HeartbeatConfig;
pub use lobby::*;
pub use state::NetEvent;

use crate::multiplayer::state::{NetworkState, Session};
use crate::multiplayer::steam_transport::{SteamLobbyEvent, SteamTransport};
use crate::multiplayer::transport::{ActiveTransport, NetTransport};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

const LAN_HOST_PORT: u16 = 7777;
const LAN_BROADCAST_ADDR: &str = "255.255.255.255:7777";
const LAN_LOOPBACK_ADDR: &str = "127.0.0.1:7777";
const LAN_CONSENT_PATH: &str = "user://networking/lan_consent";
pub const DEV_LAN_HOST_BUTTON: bool = true;

/// Built-in net backend. Steam code only enters the build through the
/// `steamworks` feature; LAN remains available without it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkBackend {
    #[default]
    Lan,
    Steam,
}

/// Per-game LAN choice stored under `user://`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LanConsent {
    #[default]
    Unknown,
    Allowed,
    Denied,
}

/// Read the saved per-game LAN choice.
pub fn lan_consent() -> LanConsent {
    let Ok(value) = perro_modules::file::load_string(LAN_CONSENT_PATH) else {
        return LanConsent::Unknown;
    };
    parse_lan_consent(&value)
}

/// Save the per-game LAN choice. Call from the game's one-time LAN prompt.
pub fn set_lan_consent(consent: LanConsent) -> Result<(), String> {
    let value = match consent {
        LanConsent::Unknown => "unknown",
        LanConsent::Allowed => "allowed",
        LanConsent::Denied => "denied",
    };
    perro_modules::file::save_string(LAN_CONSENT_PATH, value).map_err(|err| err.to_string())
}

static NETWORK: LazyLock<Mutex<NetworkState>> =
    LazyLock::new(|| Mutex::new(NetworkState::default()));

/// Set the display name used for hosted Steam lobby metadata and rich
/// presence ("Hosting {name}"). Call once at startup, before hosting.
pub fn set_game_name(name: &str) {
    crate::multiplayer::steam_transport::set_game_name(name);
}

/// Set the Steam lobby discovery tag. Host + browse must use the same tag.
/// Call once at startup, before hosting or refreshing lobbies.
pub fn set_game_tag(tag: &str) {
    crate::multiplayer::steam_transport::set_game_tag(tag);
}

/// Host through one backend while keeping game session code backend-free.
pub fn host(
    backend: NetworkBackend,
    max_players: i64,
    privacy: LobbyPrivacy,
) -> Result<(), String> {
    match backend {
        NetworkBackend::Lan => host_lan(),
        NetworkBackend::Steam => host_steam(max_players, privacy),
    }
}

/// Join through one backend. LAN ignores `lobby_id`.
pub fn join(backend: NetworkBackend, lobby_id: i64) -> Result<(), String> {
    match backend {
        NetworkBackend::Lan => join_lan(),
        NetworkBackend::Steam => join_steam(lobby_id),
    }
}

/// Refresh rows through one API. LAN discovery always runs; Steam also adds
/// public and friend rows when its feature is present.
pub fn refresh_lobbies(backend: NetworkBackend, distance: LobbyDistanceMode) -> Result<(), String> {
    match backend {
        NetworkBackend::Lan => {
            require_lan_consent()?;
            start_lan_discovery();
            Ok(())
        }
        NetworkBackend::Steam => refresh_steam_lobbies_only(distance),
    }
}

/// Mark the hosted Steam lobby as accepting or refusing late joins.
pub fn set_lobby_started(started: bool) -> Result<(), String> {
    let mut state = lock_state();
    match state.transport.as_mut() {
        Some(ActiveTransport::Steam(transport)) => transport.set_started(started),
        _ => Ok(()),
    }
}

/// Host UDP on all IPv4 interfaces, port 7777.
pub fn host_lan() -> Result<(), String> {
    require_lan_consent()?;
    perro_modules::log_info!("[net] host LAN start");
    let mut transport =
        ActiveTransport::Lan(crate::multiplayer::lan_transport::LanTransport::new_host());
    transport.host()?;
    let mut state = lock_state();
    shutdown_state(&mut state);
    state.mode = NetMode::Host;
    state.session = Some(Session::Host(
        crate::multiplayer::host_session::HostSession::new(),
    ));
    state.transport = Some(transport);
    Ok(())
}

/// Start hosting a Steam lobby. `max_players` is clamped to [MIN_PLAYERS, MAX_PLAYERS].
pub fn host_steam(max_players: i64, privacy: LobbyPrivacy) -> Result<(), String> {
    perro_modules::log_info!(
        "[net] host steam start max={} privacy={:?}",
        max_players,
        privacy
    );
    let mut transport = ActiveTransport::Steam(SteamTransport::new_host(
        clamp_max_players(max_players),
        privacy,
    ));
    transport.host()?;
    let mut state = lock_state();
    shutdown_state(&mut state);
    state.mode = NetMode::Host;
    state.session = Some(Session::Host(
        crate::multiplayer::host_session::HostSession::new(),
    ));
    state.transport = Some(transport);
    Ok(())
}

/// Join last discovered LAN host; fall back to localhost.
pub fn join_lan() -> Result<(), String> {
    let host = lock_state()
        .lan_host_addr
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], LAN_HOST_PORT)));
    join_lan_addr(host)
}

/// Join LAN or routed VPN host by socket address.
/// Missing port uses 7777: `join_lan_at("25.1.2.3")`.
pub fn join_lan_at(host: &str) -> Result<(), String> {
    let addr = parse_lan_addr(host)?;
    join_lan_addr(addr)
}

fn join_lan_addr(host: SocketAddr) -> Result<(), String> {
    require_lan_consent()?;
    perro_modules::log_info!("[net] join LAN host={}", host);
    let mut transport =
        ActiveTransport::Lan(crate::multiplayer::lan_transport::LanTransport::new_client_of(host));
    transport.join()?;
    let mut state = lock_state();
    shutdown_state(&mut state);
    state.mode = NetMode::Client;
    state.session = Some(Session::Client(
        crate::multiplayer::client_session::ClientSession::new(),
    ));
    state.transport = Some(transport);
    Ok(())
}

/// Join a Steam lobby. Accepts either a join token from lobby/friend rows or
/// a raw lobby id.
pub fn join_steam(lobby_id: i64) -> Result<(), String> {
    let lobby_id = resolve_join_token(lobby_id);
    perro_modules::log_info!("[net] join steam start lobby={}", lobby_id);
    let mut transport = ActiveTransport::Steam(SteamTransport::new_client(lobby_id));
    transport.join()?;
    let mut state = lock_state();
    shutdown_state(&mut state);
    state.mode = NetMode::Client;
    state.session = Some(Session::Client(
        crate::multiplayer::client_session::ClientSession::new(),
    ));
    state.transport = Some(transport);
    Ok(())
}

/// Kick off Steam rows plus the legacy LAN probe.
/// Prefer [`refresh_lobbies`] when selecting one backend explicitly.
/// Results arrive via [`NetEvent::LobbyRowsChanged`]; read them with
/// [`lobbies`] / [`friends`].
pub fn refresh_steam_lobbies(distance: LobbyDistanceMode) -> Result<(), String> {
    start_lan_discovery();
    refresh_steam_lobbies_only(distance)
}

fn refresh_steam_lobbies_only(distance: LobbyDistanceMode) -> Result<(), String> {
    let friends = SteamTransport::friend_lobbies();
    {
        let mut state = lock_state();
        state.join_tokens.clear();
        set_friend_rows(&mut state, friends);
        state.script_events.push(NetEvent::LobbyRowsChanged);
    }
    SteamTransport::refresh_lobbies(distance)
}

/// Pump transports + sessions. Call once per frame; results queue for
/// [`drain_events`].
pub fn poll() {
    if mode() == NetMode::Offline {
        poll_lan_discovery();
        poll_lobby_events();
    }

    let mut guard = lock_state();
    let state = &mut *guard;
    let Some(mut transport) = state.transport.take() else {
        return;
    };
    let events = transport.drain_events();
    let now = Instant::now();
    match state.session.as_mut() {
        Some(Session::Host(session)) => {
            session.handle_transport_events_into(&mut transport, events, &mut state.script_events);
            session.tick(
                &mut transport,
                &state.heartbeat,
                now,
                &mut state.script_events,
            );
        }
        Some(Session::Client(session)) => {
            session.handle_transport_events_into(&mut transport, events, &mut state.script_events);
            session.tick(
                &mut transport,
                &state.heartbeat,
                now,
                &mut state.script_events,
            );
        }
        None => {}
    }
    state.transport = Some(transport);
}

/// Set the heartbeat policy (interval / timeout / on-off). Applies immediately
/// to the live session and to any future one. Games should use this instead of
/// implementing their own keepalive; see [`HeartbeatConfig`].
pub fn configure_heartbeat(config: HeartbeatConfig) {
    lock_state().heartbeat = config;
}

/// Current heartbeat policy.
pub fn heartbeat_config() -> HeartbeatConfig {
    lock_state().heartbeat
}

/// Send a game payload. Routes by role: host broadcasts to every peer, client
/// sends to the host. Bytes are the game's own encoding, passed through as-is.
pub fn send(bytes: &[u8], reliable: bool) {
    let mut state = lock_state();
    let Some(mut transport) = state.transport.take() else {
        return;
    };
    match state.session.as_mut() {
        Some(Session::Host(session)) => session.broadcast(&mut transport, bytes, reliable),
        Some(Session::Client(session)) => session.send_to_host(&mut transport, bytes, reliable),
        None => {}
    }
    state.transport = Some(transport);
}

/// Host only: send a game payload to one connected player slot.
pub fn send_to_slot(slot: i64, bytes: &[u8], reliable: bool) -> bool {
    let mut state = lock_state();
    let Some(mut transport) = state.transport.take() else {
        return false;
    };
    let sent = match state.session.as_mut() {
        Some(Session::Host(session)) => session.send_to_slot(&mut transport, slot, bytes, reliable),
        _ => false,
    };
    state.transport = Some(transport);
    sent
}

/// Client only: re-send the join hello. Retried by games until the host's
/// SlotAssigned lands (either can drop on lossy transports).
pub fn send_ready() {
    let mut state = lock_state();
    let Some(mut transport) = state.transport.take() else {
        return;
    };
    if let Some(Session::Client(session)) = state.session.as_mut() {
        session.send_ready(&mut transport);
    }
    state.transport = Some(transport);
}

/// Take all queued [`NetEvent`]s. Call after [`poll`] each frame.
pub fn drain_events() -> Vec<NetEvent> {
    std::mem::take(&mut lock_state().script_events)
}

/// Current public lobby rows. `lobby_id` is a join token for [`join_steam`].
pub fn lobbies() -> Vec<LobbyInfo> {
    lock_state().lobbies.clone()
}

/// Current friend lobby rows. `lobby_id` is a join token for [`join_steam`].
pub fn friends() -> Vec<FriendLobbyInfo> {
    lock_state().friends.clone()
}

/// Tear down the active session + transport and return to [`NetMode::Offline`].
pub fn disconnect() {
    let mut state = lock_state();
    if let Some(mut transport) = state.transport.take() {
        match state.session.as_mut() {
            Some(Session::Client(session)) => {
                session.send_disconnect(&mut transport);
            }
            Some(Session::Host(session)) => {
                session.send_host_disconnect(&mut transport);
            }
            None => {}
        }
        state.transport = Some(transport);
    }
    shutdown_state(&mut state);
}

/// Current role: offline, host, or client.
pub fn mode() -> NetMode {
    lock_state().mode
}

/// Role helpers so game code can branch cheaply at use sites: hosts see
/// `Payload.from_slot` = sender slot (client input); clients see host
/// broadcasts (snapshots/events) w/ `from_slot` 0.
pub fn is_host() -> bool {
    mode() == NetMode::Host
}

pub fn is_client() -> bool {
    mode() == NetMode::Client
}

pub fn is_online() -> bool {
    mode() != NetMode::Offline
}

fn poll_lobby_events() {
    let events = SteamTransport::drain_lobby_events();
    if events.is_empty() {
        return;
    }
    let mut state = lock_state();
    for event in events {
        match event {
            SteamLobbyEvent::LobbyList(lobbies) => {
                set_lobby_rows(&mut state, lobbies);
                state.script_events.push(NetEvent::LobbyRowsChanged);
            }
            SteamLobbyEvent::LobbyListFailed => {
                state.lobbies.clear();
                state.join_tokens.clear();
                state.script_events.push(NetEvent::LobbyRowsChanged);
            }
            SteamLobbyEvent::FriendList(friends) => {
                set_friend_rows(&mut state, friends);
                state.script_events.push(NetEvent::LobbyRowsChanged);
            }
            SteamLobbyEvent::LobbyDataUpdated(_)
            | SteamLobbyEvent::LobbyMemberChanged(_)
            | SteamLobbyEvent::PersonaChanged(_) => {
                let friends = SteamTransport::friend_lobbies();
                set_friend_rows(&mut state, friends);
                state.script_events.push(NetEvent::LobbyRowsChanged);
            }
            SteamLobbyEvent::CreateFailed
            | SteamLobbyEvent::JoinFailed(_)
            | SteamLobbyEvent::LobbyChat(_)
            | SteamLobbyEvent::OverlayChanged(_)
            | SteamLobbyEvent::Callback(_)
            | SteamLobbyEvent::Created(_)
            | SteamLobbyEvent::Joined(_) => {}
            SteamLobbyEvent::JoinRequested(lobby_id) => {
                drop(state);
                let _ = join_steam(lobby_id);
                return;
            }
        }
    }
}

fn shutdown_state(state: &mut NetworkState) {
    if let Some(transport) = state.transport.as_mut() {
        transport.shutdown();
    }
    state.mode = NetMode::Offline;
    state.session = None;
    state.transport = None;
    state.script_events.clear();
    state.lan_discovery = None;
}

fn parse_lan_addr(host: &str) -> Result<SocketAddr, String> {
    let host = host.trim();
    if let Ok(addr) = host.parse() {
        return Ok(addr);
    }
    format!("{host}:{LAN_HOST_PORT}")
        .parse()
        .map_err(|err| format!("invalid LAN host `{host}`: {err}"))
}

fn parse_lan_consent(value: &str) -> LanConsent {
    match value.trim() {
        "allowed" => LanConsent::Allowed,
        "denied" => LanConsent::Denied,
        _ => LanConsent::Unknown,
    }
}

fn require_lan_consent() -> Result<(), String> {
    match lan_consent() {
        LanConsent::Unknown | LanConsent::Allowed => Ok(()),
        LanConsent::Denied => Err("LAN access denied by saved game setting".into()),
    }
}

fn lock_state() -> std::sync::MutexGuard<'static, NetworkState> {
    NETWORK.lock().unwrap_or_else(|err| err.into_inner())
}

fn set_lobby_rows(state: &mut NetworkState, lobbies: Vec<LobbyInfo>) {
    state.lobbies.clear();
    state
        .join_tokens
        .retain(|(token, _)| *token == crate::multiplayer::lan_transport::LAN_JOIN_TOKEN);
    for (index, mut lobby) in lobbies.into_iter().enumerate() {
        let real_id = lobby.lobby_id;
        let token = index as i64 + 1;
        lobby.lobby_id = token;
        state.join_tokens.push((token, real_id));
        state.lobbies.push(lobby);
    }
}

fn set_friend_rows(state: &mut NetworkState, friends: Vec<FriendLobbyInfo>) {
    let local = state
        .friends
        .iter()
        .find(|friend| friend.lobby_id == crate::multiplayer::lan_transport::LAN_JOIN_TOKEN)
        .cloned();
    state.friends.clear();
    state.join_tokens.retain(|(token, _)| *token < 1000);
    if let Some(local) = local {
        state.friends.push(local);
    }
    for (index, mut friend) in friends.into_iter().enumerate() {
        let real_id = friend.lobby_id;
        let token = 1000 + index as i64 + 1;
        friend.lobby_id = token;
        state.join_tokens.push((token, real_id));
        state.friends.push(friend);
    }
}

fn resolve_join_token(token: i64) -> i64 {
    if token == crate::multiplayer::lan_transport::LAN_JOIN_TOKEN {
        return token;
    }
    let state = lock_state();
    state
        .join_tokens
        .iter()
        .find(|(stored, _)| *stored == token)
        .map(|(_, lobby_id)| *lobby_id)
        .unwrap_or(token)
}

fn start_lan_discovery() {
    if require_lan_consent().is_err() {
        return;
    }
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        return;
    };
    if socket.set_broadcast(true).is_err() {
        return;
    }
    if socket.set_nonblocking(true).is_err() {
        return;
    }
    let packet = crate::multiplayer::lan_transport::LAN_DISCOVER;
    let _ = socket.send_to(packet, LAN_BROADCAST_ADDR);
    let _ = socket.send_to(packet, LAN_LOOPBACK_ADDR);
    let mut state = lock_state();
    state.lan_discovery = Some(crate::multiplayer::state::LanDiscovery { socket, age: 0.0 });
}

fn poll_lan_discovery() {
    let mut state = lock_state();
    let Some(discovery) = state.lan_discovery.as_mut() else {
        return;
    };
    let mut buf = [0_u8; 64];
    match discovery.socket.recv_from(&mut buf) {
        Ok((len, addr)) if &buf[..len] == crate::multiplayer::lan_transport::LAN_DISCOVER_REPLY => {
            state.lan_host_addr = Some(addr);
            add_lan_lobby_row(&mut state);
            state.lan_discovery = None;
            state.script_events.push(NetEvent::LobbyRowsChanged);
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
            discovery.age += 1.0 / 60.0;
            if discovery.age > 1.0 {
                state.lan_discovery = None;
            }
        }
        Err(_) => {
            state.lan_discovery = None;
        }
    }
}

fn add_lan_lobby_row(state: &mut NetworkState) {
    let token = crate::multiplayer::lan_transport::LAN_JOIN_TOKEN;
    if state.friends.iter().any(|friend| friend.lobby_id == token) {
        return;
    }
    state.friends.insert(
        0,
        FriendLobbyInfo {
            steam_id: 0,
            lobby_id: token,
            name: "LAN Host".to_string(),
            state: "LAN".to_string(),
        },
    );
    if !state.join_tokens.iter().any(|(stored, _)| *stored == token) {
        state.join_tokens.push((token, token));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_consent_values_parse_strictly() {
        assert_eq!(parse_lan_consent("allowed\n"), LanConsent::Allowed);
        assert_eq!(parse_lan_consent("denied"), LanConsent::Denied);
        assert_eq!(parse_lan_consent("yes"), LanConsent::Unknown);
    }

    #[test]
    fn lobby_rows_use_join_tokens_and_keep_real_ids() {
        let mut state = NetworkState::default();

        set_lobby_rows(&mut state, vec![lobby(9001, "A"), lobby(9002, "B")]);

        assert_eq!(state.lobbies[0].lobby_id, 1);
        assert_eq!(state.lobbies[1].lobby_id, 2);
        assert_eq!(state.join_tokens, vec![(1, 9001), (2, 9002)]);
    }

    #[test]
    fn friend_rows_use_high_tokens_and_keep_local_row() {
        let mut state = NetworkState::default();
        add_lan_lobby_row(&mut state);

        set_friend_rows(
            &mut state,
            vec![friend(42, 7001, "One"), friend(43, 7002, "Two")],
        );

        assert_eq!(
            state.friends[0].lobby_id,
            crate::multiplayer::lan_transport::LAN_JOIN_TOKEN
        );
        assert_eq!(state.friends[1].lobby_id, 1001);
        assert_eq!(state.friends[2].lobby_id, 1002);
        assert!(
            state
                .join_tokens
                .contains(&(crate::multiplayer::lan_transport::LAN_JOIN_TOKEN, -1))
        );
        assert!(state.join_tokens.contains(&(1001, 7001)));
        assert!(state.join_tokens.contains(&(1002, 7002)));
    }

    #[test]
    fn add_lan_lobby_row_is_idempotent() {
        let mut state = NetworkState::default();

        add_lan_lobby_row(&mut state);
        add_lan_lobby_row(&mut state);

        assert_eq!(state.friends.len(), 1);
        assert_eq!(
            state.join_tokens,
            vec![(
                crate::multiplayer::lan_transport::LAN_JOIN_TOKEN,
                crate::multiplayer::lan_transport::LAN_JOIN_TOKEN,
            )]
        );
    }

    fn lobby(lobby_id: i64, name: &str) -> LobbyInfo {
        LobbyInfo {
            lobby_id,
            name: name.to_string(),
            ..Default::default()
        }
    }

    fn friend(steam_id: i64, lobby_id: i64, name: &str) -> FriendLobbyInfo {
        FriendLobbyInfo {
            steam_id,
            lobby_id,
            name: name.to_string(),
            ..Default::default()
        }
    }
}
