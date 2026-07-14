use crate::multiplayer::heartbeat::HeartbeatConfig;
use crate::multiplayer::lobby::{FriendLobbyInfo, LobbyInfo, NetMode};
use crate::multiplayer::transport::ActiveTransport;
use std::net::{SocketAddr, UdpSocket};

/// Events surfaced to the game each poll. Payload bytes are the game's own
/// encoding, passed through untouched; `from_slot` is the transport-truth
/// sender slot (host side) or 0 when unknown (client side — the game codec
/// carries its own sender field if it needs one).
#[derive(Clone, Debug)]
pub enum NetEvent {
    /// Client: transport connected to the host.
    Connected,
    /// Client: host assigned this peer its slot.
    SlotAssigned {
        slot: i64,
    },
    /// Host: a peer connected and received a slot. `steam_id` is the peer's
    /// 64-bit SteamID on Steam transport, or 0 on local transport (no Steam
    /// identity — the game falls back to a manual face upload there).
    PeerJoined {
        slot: i64,
        steam_id: i64,
    },
    /// Host: a peer sent its join hello (repeats on retry; dedupe game-side).
    /// `steam_id` as in [`NetEvent::PeerJoined`].
    PeerReady {
        slot: i64,
        steam_id: i64,
    },
    /// Host: a peer disconnected.
    PeerLeft {
        slot: i64,
    },
    /// A game payload arrived.
    Payload {
        from_slot: i64,
        bytes: Vec<u8>,
    },
    /// Session ended (host gone / transport failed).
    Disconnected,
    LobbyRowsChanged,
}

pub enum Session {
    Host(crate::multiplayer::host_session::HostSession),
    Client(crate::multiplayer::client_session::ClientSession),
}

pub struct NetworkState {
    pub mode: NetMode,
    pub session: Option<Session>,
    pub transport: Option<ActiveTransport>,
    pub script_events: Vec<NetEvent>,
    pub lobbies: Vec<LobbyInfo>,
    pub friends: Vec<FriendLobbyInfo>,
    pub join_tokens: Vec<(i64, i64)>,
    pub lan_discovery: Option<LanDiscovery>,
    pub lan_host_addr: Option<SocketAddr>,
    pub heartbeat: HeartbeatConfig,
}

pub struct LanDiscovery {
    pub socket: UdpSocket,
    pub age: f32,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            mode: NetMode::Offline,
            session: None,
            transport: None,
            script_events: Vec::new(),
            lobbies: Vec::new(),
            friends: Vec::new(),
            join_tokens: Vec::new(),
            lan_discovery: None,
            lan_host_addr: None,
            heartbeat: HeartbeatConfig::default(),
        }
    }
}
