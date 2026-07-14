#![cfg_attr(not(feature = "steamworks"), allow(dead_code))]

use crate::multiplayer::lobby::{FriendLobbyInfo, LobbyDistanceMode, LobbyInfo, LobbyPrivacy};
use crate::multiplayer::transport::{NetTransport, PeerId, TransportEvent};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Display name stamped on hosted lobbies + rich presence. Games set it once at
// startup via [`crate::multiplayer::set_game_name`]; empty means "unset" and browse-side
// readers fall back to "Steam {lobby_id}".
static GAME_NAME: Mutex<String> = Mutex::new(String::new());
static GAME_TAG: Mutex<String> = Mutex::new(String::new());

pub(crate) fn set_game_name(name: &str) {
    *GAME_NAME.lock().unwrap_or_else(|err| err.into_inner()) = name.trim().to_string();
}

pub(crate) fn set_game_tag(tag: &str) {
    *GAME_TAG.lock().unwrap_or_else(|err| err.into_inner()) = tag.trim().to_string();
}

#[cfg_attr(not(feature = "steamworks"), allow(dead_code))]
pub(crate) fn game_name() -> String {
    GAME_NAME
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
}

pub(crate) fn game_tag() -> String {
    let tag = GAME_TAG
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone();
    if tag.is_empty() {
        DEFAULT_GAME_TAG_VALUE.to_string()
    } else {
        tag
    }
}

const LOBBY_SEARCH_MAX: usize = 50;
const GAME_TAG_KEY: &str = "tag";
const DEFAULT_GAME_TAG_VALUE: &str = "KangarooCourtroom";
/// How often `drain_events` re-polls the lobby member list as a backstop.
/// Membership is primarily tracked from `LobbyMemberChanged` events; this
/// interval only bounds how long a dropped/misordered callback can leave the
/// peer set stale. Kept coarse so we don't query Steam every frame.
const PEER_SYNC_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub enum SteamLobbyEvent {
    Created(i64),
    CreateFailed,
    Joined(i64),
    JoinFailed(i64),
    LobbyList(Vec<LobbyInfo>),
    LobbyListFailed,
    FriendList(Vec<FriendLobbyInfo>),
    LobbyDataUpdated(i64),
    LobbyChat(i64),
    LobbyMemberChanged(i64),
    JoinRequested(i64),
    PersonaChanged(i64),
    OverlayChanged(bool),
    Callback(&'static str),
}

pub struct SteamTransport {
    is_host: bool,
    max_players: i64,
    privacy: LobbyPrivacy,
    lobby_id: i64,
    join_lobby_id: i64,
    peers: Vec<i64>,
    pending_events: Vec<TransportEvent>,
    // When the backstop last reconciled peers; `None` forces a sync next drain.
    last_peer_sync: Option<Instant>,
}

impl SteamTransport {
    pub fn new_host(max_players: i64, privacy: LobbyPrivacy) -> Self {
        Self {
            is_host: true,
            max_players,
            privacy,
            lobby_id: 0,
            join_lobby_id: 0,
            peers: Vec::new(),
            pending_events: Vec::new(),
            last_peer_sync: None,
        }
    }

    pub fn new_client(lobby_id: i64) -> Self {
        Self {
            is_host: false,
            max_players: 0,
            privacy: LobbyPrivacy::Public,
            lobby_id,
            join_lobby_id: lobby_id,
            peers: Vec::new(),
            pending_events: Vec::new(),
            last_peer_sync: None,
        }
    }

    pub fn refresh_lobbies(distance: LobbyDistanceMode) -> Result<(), String> {
        steam_browse_lobbies(distance)
    }

    pub fn drain_lobby_events() -> Vec<SteamLobbyEvent> {
        steam_poll_lobby_events()
    }

    pub fn friend_lobbies() -> Vec<FriendLobbyInfo> {
        steam_friend_lobbies()
    }

    /// Update the host lobby's started flag so browse rows stop accepting
    /// joins once a match begins.
    pub fn set_started(&self, started: bool) -> Result<(), String> {
        if !self.is_host || self.lobby_id <= 0 {
            return Ok(());
        }
        steam_set_lobby_started(self.lobby_id, started)
    }

    fn set_lobby(&mut self, lobby_id: i64) {
        self.lobby_id = lobby_id;
        self.peers = steam_lobby_peers(lobby_id);
        for peer in &self.peers {
            self.pending_events
                .push(TransportEvent::PeerConnected(PeerId::Steam(*peer)));
        }
    }

    /// Reconcile `self.peers` against the Steam lobby member list, emitting
    /// PeerConnected/PeerDisconnected for the delta. Driven by
    /// `LobbyMemberChanged` events (Steam's join/leave callback); `drain_events`
    /// also calls it on a `PEER_SYNC_INTERVAL` backstop. Every call stamps
    /// `last_peer_sync`, so an event-driven sync also resets the backstop and a
    /// member-change frame never syncs twice.
    fn sync_lobby_peers(&mut self, out: &mut Vec<TransportEvent>) {
        self.last_peer_sync = Some(Instant::now());
        if self.lobby_id <= 0 {
            return;
        }
        let current = steam_lobby_peers(self.lobby_id);
        for peer in self.peers.iter().copied() {
            if !current.contains(&peer) {
                out.push(TransportEvent::PeerDisconnected(PeerId::Steam(peer)));
            }
        }
        for peer in current.iter().copied() {
            if !self.peers.contains(&peer) {
                out.push(TransportEvent::PeerConnected(PeerId::Steam(peer)));
            }
        }
        self.peers = current;
    }

    fn handle_lobby_event(&mut self, event: SteamLobbyEvent, out: &mut Vec<TransportEvent>) {
        match event {
            SteamLobbyEvent::Created(lobby_id) => {
                let _ = steam_set_host_options(lobby_id, self.privacy);
                self.set_lobby(lobby_id);
            }
            SteamLobbyEvent::CreateFailed => {
                if self.is_host {
                    out.push(TransportEvent::SessionFailed);
                }
            }
            SteamLobbyEvent::Joined(lobby_id) => {
                self.set_lobby(lobby_id);
            }
            SteamLobbyEvent::JoinFailed(lobby_id) => {
                if !self.is_host && (self.join_lobby_id == lobby_id || lobby_id <= 0) {
                    out.push(TransportEvent::SessionFailed);
                }
            }
            SteamLobbyEvent::LobbyMemberChanged(lobby_id)
            | SteamLobbyEvent::LobbyDataUpdated(lobby_id)
            | SteamLobbyEvent::LobbyChat(lobby_id) => {
                if lobby_id == self.lobby_id {
                    self.sync_lobby_peers(out);
                }
            }
            SteamLobbyEvent::Callback(name) => {
                if matches!(
                    name,
                    "p2p_session_connect_fail" | "networking_messages_session_failed"
                ) {
                    out.push(TransportEvent::SessionFailed);
                }
            }
            SteamLobbyEvent::LobbyList(_)
            | SteamLobbyEvent::LobbyListFailed
            | SteamLobbyEvent::FriendList(_)
            | SteamLobbyEvent::JoinRequested(_)
            | SteamLobbyEvent::PersonaChanged(_)
            | SteamLobbyEvent::OverlayChanged(_) => {}
        }
    }
}

impl NetTransport for SteamTransport {
    fn host(&mut self) -> Result<(), String> {
        steam_create_host_lobby(self.max_players, self.privacy)
    }

    fn join(&mut self) -> Result<(), String> {
        steam_join_lobby(self.join_lobby_id)
    }

    fn send(&mut self, peer: &PeerId, bytes: &[u8], reliable: bool) {
        if let PeerId::Steam(id) = peer {
            steam_send_bytes(&[*id], bytes, reliable);
        }
    }

    fn broadcast(&mut self, bytes: &[u8], reliable: bool) {
        steam_send_bytes(&self.peers, bytes, reliable);
    }

    fn drain_events(&mut self) -> Vec<TransportEvent> {
        let mut out = Vec::new();
        for event in steam_poll_lobby_events() {
            match event {
                SteamLobbyEvent::JoinRequested(lobby_id) if !self.is_host => {
                    self.join_lobby_id = lobby_id;
                    let _ = self.join();
                }
                event => self.handle_lobby_event(event, &mut out),
            }
        }
        out.append(&mut self.pending_events);
        // Backstop only: event-driven syncs above handle the common case and
        // reset the timer. New peers are also caught immediately by the P2P
        // read path below, so this just bounds staleness for missed callbacks.
        let sync_due = self
            .last_peer_sync
            .is_none_or(|at| at.elapsed() >= PEER_SYNC_INTERVAL);
        if sync_due {
            self.sync_lobby_peers(&mut out);
        }
        for (from, bytes) in steam_read_bytes() {
            if !self.peers.contains(&from) {
                self.peers.push(from);
                out.push(TransportEvent::PeerConnected(PeerId::Steam(from)));
            }
            out.push(TransportEvent::PacketReceived(PeerId::Steam(from), bytes));
        }
        out
    }

    fn shutdown(&mut self) {
        for peer in self.peers.drain(..) {
            self.pending_events
                .push(TransportEvent::PeerDisconnected(PeerId::Steam(peer)));
        }
        if self.lobby_id > 0 {
            if self.is_host {
                let _ = steam_close_host_lobby(self.lobby_id);
            }
            let _ = steam_leave_lobby(self.lobby_id);
        }
        let _ = steam_clear_presence();
        self.lobby_id = 0;
    }
}

#[cfg(feature = "steamworks")]
fn steam_create_host_lobby(max_players: i64, privacy: LobbyPrivacy) -> Result<(), String> {
    use perro_steamworks as steam;

    let kind = match privacy {
        LobbyPrivacy::Public => steam::LobbyType::Public,
        LobbyPrivacy::Friends => steam::LobbyType::FriendsOnly,
        LobbyPrivacy::Private => steam::LobbyType::Private,
    };
    steam::lobbies::create(kind, max_players as u32).map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_create_host_lobby(_max_players: i64, _privacy: LobbyPrivacy) -> Result<(), String> {
    Err("steamworks feature off for scripts".to_string())
}

#[cfg(feature = "steamworks")]
fn steam_browse_lobbies(distance: LobbyDistanceMode) -> Result<(), String> {
    use perro_steamworks as steam;

    let tag = game_tag();
    let search = steam::LobbySearch {
        max_results: Some(LOBBY_SEARCH_MAX as u64),
        distance: Some(steam_distance(distance)),
        string_filters: vec![steam::LobbyStringFilter::new(
            GAME_TAG_KEY,
            &tag,
            steam::LobbyStringFilterKind::Equal,
        )],
        ..Default::default()
    };
    steam::lobbies::request_list(search).map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_browse_lobbies(_distance: LobbyDistanceMode) -> Result<(), String> {
    Err("steamworks feature off for scripts".to_string())
}

#[cfg(feature = "steamworks")]
fn steam_distance(distance: LobbyDistanceMode) -> perro_steamworks::LobbyDistance {
    use perro_steamworks as steam;

    match distance {
        LobbyDistanceMode::Local => steam::LobbyDistance::Close,
        LobbyDistanceMode::Regional => steam::LobbyDistance::Default,
        LobbyDistanceMode::Worldwide => steam::LobbyDistance::Worldwide,
    }
}

#[cfg(feature = "steamworks")]
fn steam_join_lobby(lobby_id: i64) -> Result<(), String> {
    use perro_steamworks as steam;

    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    steam::lobbies::join(lobby).map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_join_lobby(_lobby_id: i64) -> Result<(), String> {
    Err("steamworks feature off for scripts".to_string())
}

#[cfg(feature = "steamworks")]
fn steam_leave_lobby(lobby_id: i64) -> Result<(), String> {
    use perro_steamworks as steam;

    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    steam::lobbies::leave(lobby).map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_leave_lobby(_lobby_id: i64) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "steamworks")]
fn steam_set_host_options(lobby_id: i64, privacy: LobbyPrivacy) -> Result<(), String> {
    use perro_steamworks as steam;

    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    let tag = game_tag();
    steam::lobbies::set_data(lobby, steam::LobbyDataKey::Name, &game_name())
        .map_err(|err| format!("{err:?}"))?;
    steam::lobbies::set_data(lobby, steam::LobbyDataKey::Version, "v0")
        .map_err(|err| format!("{err:?}"))?;
    steam::lobbies::set_data(lobby, GAME_TAG_KEY, &tag).map_err(|err| format!("{err:?}"))?;
    steam::lobbies::set_data(lobby, "privacy", privacy.key()).map_err(|err| format!("{err:?}"))?;
    steam::lobbies::set_data(lobby, "started", "0").map_err(|err| format!("{err:?}"))?;
    steam_set_presence_lobby(lobby_id)
}

#[cfg(not(feature = "steamworks"))]
fn steam_set_host_options(_lobby_id: i64, _privacy: LobbyPrivacy) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "steamworks")]
fn steam_close_host_lobby(lobby_id: i64) -> Result<(), String> {
    use perro_steamworks as steam;

    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    steam::lobbies::set_data(lobby, "started", "1").map_err(|err| format!("{err:?}"))?;
    steam::lobbies::set_data(lobby, "privacy", "private").map_err(|err| format!("{err:?}"))
}

#[cfg(feature = "steamworks")]
fn steam_set_lobby_started(lobby_id: i64, started: bool) -> Result<(), String> {
    use perro_steamworks as steam;
    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    steam::lobbies::set_data(lobby, "started", if started { "1" } else { "0" })
        .map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_set_lobby_started(_lobby_id: i64, _started: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "steamworks"))]
fn steam_close_host_lobby(_lobby_id: i64) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "steamworks")]
fn steam_poll_lobby_events() -> Vec<SteamLobbyEvent> {
    use perro_steamworks as steam;

    let Ok(events) = steam::events::drain() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for event in events {
        match event {
            steam::SteamEvent::LobbyCreated { lobby } => {
                out.push(SteamLobbyEvent::Created(lobby.get_id() as i64));
            }
            steam::SteamEvent::LobbyCreateFailed => {
                out.push(SteamLobbyEvent::CreateFailed);
            }
            steam::SteamEvent::LobbyJoined { lobby } => {
                out.push(SteamLobbyEvent::Joined(lobby.get_id() as i64));
            }
            steam::SteamEvent::LobbyJoinFailed { lobby } => {
                out.push(SteamLobbyEvent::JoinFailed(lobby.get_id() as i64));
            }
            steam::SteamEvent::LobbyList { lobbies } => {
                out.push(SteamLobbyEvent::LobbyList(read_lobby_infos(lobbies)));
                out.push(SteamLobbyEvent::FriendList(steam_friend_lobbies()));
            }
            steam::SteamEvent::LobbyListFailed => {
                out.push(SteamLobbyEvent::LobbyListFailed);
                out.push(SteamLobbyEvent::FriendList(steam_friend_lobbies()));
            }
            steam::SteamEvent::LobbyJoinRequested { lobby, .. } => {
                out.push(SteamLobbyEvent::JoinRequested(lobby.get_id() as i64));
            }
            steam::SteamEvent::RichPresenceJoinRequested { connect, .. } => {
                if let Some(lobby_id) = parse_lobby_connect(&connect) {
                    out.push(SteamLobbyEvent::JoinRequested(lobby_id));
                }
            }
            steam::SteamEvent::LobbyDataUpdated { lobby, .. } => {
                out.push(SteamLobbyEvent::LobbyDataUpdated(lobby.get_id() as i64));
            }
            steam::SteamEvent::LobbyChat { lobby, .. } => {
                out.push(SteamLobbyEvent::LobbyChat(lobby.get_id() as i64));
            }
            steam::SteamEvent::LobbyMemberChanged { lobby, .. } => {
                out.push(SteamLobbyEvent::LobbyMemberChanged(lobby.get_id() as i64));
            }
            steam::SteamEvent::PersonaChanged { user } => {
                out.push(SteamLobbyEvent::PersonaChanged(user.get_id() as i64));
            }
            steam::SteamEvent::OverlayChanged { active } => {
                out.push(SteamLobbyEvent::OverlayChanged(active));
            }
            steam::SteamEvent::ServerAuthValidated { .. } => {
                out.push(SteamLobbyEvent::Callback("server_auth_validated"));
            }
            steam::SteamEvent::Callback { name } => {
                out.push(SteamLobbyEvent::Callback(name));
            }
        }
    }
    out
}

#[cfg(not(feature = "steamworks"))]
fn steam_poll_lobby_events() -> Vec<SteamLobbyEvent> {
    Vec::new()
}

#[cfg(feature = "steamworks")]
fn steam_lobby_peers(lobby_id: i64) -> Vec<i64> {
    use perro_steamworks as steam;

    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    let self_id = steam::account::get_self_id()
        .ok()
        .map(|id| id.get_id() as i64);
    steam::lobbies::get_members(lobby)
        .unwrap_or_default()
        .into_iter()
        .map(|id| id.get_id() as i64)
        .filter(|id| Some(*id) != self_id)
        .collect()
}

#[cfg(not(feature = "steamworks"))]
fn steam_lobby_peers(_lobby_id: i64) -> Vec<i64> {
    Vec::new()
}

#[cfg(feature = "steamworks")]
fn steam_send_bytes(targets: &[i64], bytes: &[u8], reliable: bool) {
    use perro_steamworks as steam;

    let send_type = if reliable {
        steam::networking::SendType::Reliable
    } else {
        steam::networking::SendType::UnreliableNoDelay
    };
    for target in targets {
        let _ = steam::networking::is_p2p_sent(
            steam::SteamID::from_id((*target).max(0) as u64),
            send_type,
            bytes,
        );
    }
}

#[cfg(not(feature = "steamworks"))]
fn steam_send_bytes(_targets: &[i64], _bytes: &[u8], _reliable: bool) {}

#[cfg(feature = "steamworks")]
fn steam_read_bytes() -> Vec<(i64, Vec<u8>)> {
    use perro_steamworks as steam;

    let mut out = Vec::new();
    while let Ok(Some((from, bytes))) = steam::networking::get_p2p_packet(16384) {
        out.push((from.get_id() as i64, bytes));
    }
    out
}

#[cfg(not(feature = "steamworks"))]
fn steam_read_bytes() -> Vec<(i64, Vec<u8>)> {
    Vec::new()
}

fn parse_lobby_connect(connect: &str) -> Option<i64> {
    connect.strip_prefix("lobby:")?.parse::<i64>().ok()
}

#[cfg(feature = "steamworks")]
fn steam_friend_lobbies() -> Vec<FriendLobbyInfo> {
    use perro_steamworks as steam;

    let Ok(friends) = steam::friends::get_list() else {
        return Vec::new();
    };
    friends
        .into_iter()
        .filter_map(|friend| {
            let lobby_id = friend
                .game
                .as_ref()
                .map(|game| game.lobby.get_id() as i64)
                .or_else(|| {
                    steam::friends::get_rich_presence(friend.id, steam::RichPresenceKey::Connect)
                        .ok()
                        .flatten()
                        .and_then(|connect| parse_lobby_connect(&connect))
                })?;
            if lobby_id <= 0 {
                return None;
            }
            if !steam_lobby_has_game_tag(lobby_id) {
                return None;
            }
            Some(FriendLobbyInfo {
                steam_id: friend.id.get_id() as i64,
                lobby_id,
                name: friend.name,
                state: format!("{:?}", friend.state),
            })
        })
        .collect()
}

#[cfg(not(feature = "steamworks"))]
fn steam_friend_lobbies() -> Vec<FriendLobbyInfo> {
    Vec::new()
}

#[cfg(feature = "steamworks")]
fn steam_lobby_has_game_tag(lobby_id: i64) -> bool {
    use perro_steamworks as steam;

    let lobby = steam::LobbyID::from_id(lobby_id.max(0) as u64);
    let tag = game_tag();
    steam::lobbies::get_data(lobby, GAME_TAG_KEY)
        .ok()
        .flatten()
        .as_deref()
        == Some(tag.as_str())
}

#[cfg(feature = "steamworks")]
fn steam_set_presence_lobby(lobby_id: i64) -> Result<(), String> {
    use perro_steamworks as steam;

    let name = game_name();
    let status = if name.is_empty() {
        "Hosting a lobby".to_string()
    } else {
        format!("Hosting {name}")
    };
    steam::friends::set_rich_presence(steam::RichPresenceKey::Status, &status)
        .map_err(|err| format!("{err:?}"))?;
    steam::friends::set_rich_presence(
        steam::RichPresenceKey::Connect,
        &format!("lobby:{}", lobby_id),
    )
    .map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_set_presence_lobby(_lobby_id: i64) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "steamworks")]
fn steam_clear_presence() -> Result<(), String> {
    use perro_steamworks as steam;

    steam::friends::clear_rich_presence().map_err(|err| format!("{err:?}"))
}

#[cfg(not(feature = "steamworks"))]
fn steam_clear_presence() -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "steamworks")]
fn read_lobby_infos(lobbies: Vec<perro_steamworks::LobbyID>) -> Vec<LobbyInfo> {
    use perro_steamworks as steam;

    let mut infos = Vec::new();
    for lobby in lobbies {
        let Ok(info) = steam::lobbies::get_info(lobby) else {
            continue;
        };
        if !lobby_has_game_tag(&info.data) {
            continue;
        }
        let started = info
            .data
            .iter()
            .find(|(key, _)| key == "started")
            .map(|(_, value)| value == "1")
            .unwrap_or(false);
        let private = info
            .data
            .iter()
            .find(|(key, _)| key == "privacy")
            .map(|(_, value)| value == "private")
            .unwrap_or(false);
        if private {
            continue;
        }
        infos.push(LobbyInfo {
            lobby_id: info.id.get_id() as i64,
            owner_id: info.owner.get_id() as i64,
            name: info
                .data
                .iter()
                .find(|(key, _)| key == "name")
                .map(|(_, value)| value.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("Steam {}", info.id.get_id())),
            members: info.members.len() as i64,
            max_players: info.member_limit.unwrap_or(0) as i64,
            started,
        });
    }
    infos
}

fn lobby_has_game_tag(data: &[(String, String)]) -> bool {
    let tag = game_tag();
    data.iter()
        .any(|(key, value)| key == GAME_TAG_KEY && value == &tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_create_failed_emits_session_failed() {
        let mut transport = SteamTransport::new_host(4, LobbyPrivacy::Friends);
        let mut out = Vec::new();

        transport.handle_lobby_event(SteamLobbyEvent::CreateFailed, &mut out);

        assert!(
            out.iter()
                .any(|event| matches!(event, TransportEvent::SessionFailed))
        );
    }

    #[test]
    fn client_join_failed_for_target_emits_session_failed() {
        let mut transport = SteamTransport::new_client(55);
        let mut out = Vec::new();

        transport.handle_lobby_event(SteamLobbyEvent::JoinFailed(55), &mut out);

        assert!(
            out.iter()
                .any(|event| matches!(event, TransportEvent::SessionFailed))
        );
    }

    #[test]
    fn client_join_failed_for_other_lobby_is_ignored() {
        let mut transport = SteamTransport::new_client(55);
        let mut out = Vec::new();

        transport.handle_lobby_event(SteamLobbyEvent::JoinFailed(99), &mut out);

        assert!(out.is_empty());
    }

    #[test]
    fn p2p_connect_fail_callback_emits_session_failed() {
        let mut transport = SteamTransport::new_client(55);
        let mut out = Vec::new();

        transport.handle_lobby_event(
            SteamLobbyEvent::Callback("p2p_session_connect_fail"),
            &mut out,
        );

        assert!(
            out.iter()
                .any(|event| matches!(event, TransportEvent::SessionFailed))
        );
    }

    #[test]
    fn lobby_member_change_disconnects_missing_peer() {
        let mut transport = SteamTransport::new_host(4, LobbyPrivacy::Public);
        let mut out = Vec::new();
        transport.lobby_id = 77;
        transport.peers.push(42);

        transport.handle_lobby_event(SteamLobbyEvent::LobbyMemberChanged(77), &mut out);

        assert!(
            out.iter()
                .any(|event| matches!(event, TransportEvent::PeerDisconnected(PeerId::Steam(42))))
        );
        assert!(transport.peers.is_empty());
    }

    #[test]
    fn backstop_peer_sync_is_throttled_within_interval() {
        let mut transport = SteamTransport::new_host(4, LobbyPrivacy::Public);
        transport.lobby_id = 77;
        transport.peers.push(42);

        // First drain runs the backstop sync. The stubbed member list is empty,
        // so the known peer is reported gone and the timer is stamped.
        let first = transport.drain_events();
        assert!(
            first
                .iter()
                .any(|event| matches!(event, TransportEvent::PeerDisconnected(PeerId::Steam(42))))
        );
        assert!(transport.last_peer_sync.is_some());

        // A second drain inside the same interval must NOT re-run the sync: a
        // peer re-added behind its back is left untouched until the window passes.
        transport.peers.push(43);
        let second = transport.drain_events();
        assert!(
            !second
                .iter()
                .any(|event| matches!(event, TransportEvent::PeerDisconnected(PeerId::Steam(43))))
        );
        assert_eq!(transport.peers, vec![43]);
    }

    #[test]
    fn event_driven_sync_still_runs_when_backstop_is_throttled() {
        let mut transport = SteamTransport::new_host(4, LobbyPrivacy::Public);
        transport.lobby_id = 77;
        // Pretend the backstop just ran, so the tail poll would skip this frame.
        transport.last_peer_sync = Some(Instant::now());
        transport.peers.push(42);
        let mut out = Vec::new();

        // A membership callback must sync regardless of the backstop throttle.
        transport.handle_lobby_event(SteamLobbyEvent::LobbyMemberChanged(77), &mut out);

        assert!(
            out.iter()
                .any(|event| matches!(event, TransportEvent::PeerDisconnected(PeerId::Steam(42))))
        );
        assert!(transport.peers.is_empty());
    }

    #[test]
    fn non_session_steam_events_do_not_fail_session() {
        let mut transport = SteamTransport::new_host(4, LobbyPrivacy::Public);
        let mut out = Vec::new();

        for event in [
            SteamLobbyEvent::LobbyList(Vec::new()),
            SteamLobbyEvent::LobbyListFailed,
            SteamLobbyEvent::FriendList(Vec::new()),
            SteamLobbyEvent::LobbyDataUpdated(7),
            SteamLobbyEvent::LobbyChat(7),
            SteamLobbyEvent::PersonaChanged(5),
            SteamLobbyEvent::OverlayChanged(true),
            SteamLobbyEvent::Callback("overlay"),
        ] {
            transport.handle_lobby_event(event, &mut out);
        }

        assert!(
            !out.iter()
                .any(|event| matches!(event, TransportEvent::SessionFailed))
        );
    }

    #[test]
    fn lobby_tag_match_is_exact() {
        set_game_tag("FroggyForklift");
        assert!(lobby_has_game_tag(&[(
            GAME_TAG_KEY.to_string(),
            "FroggyForklift".to_string(),
        )]));
        assert!(!lobby_has_game_tag(&[(
            GAME_TAG_KEY.to_string(),
            "OtherGame".to_string(),
        )]));
    }
}
