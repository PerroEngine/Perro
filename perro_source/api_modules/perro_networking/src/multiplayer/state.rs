use crate::multiplayer::heartbeat::HeartbeatConfig;
use crate::multiplayer::lobby::{FriendLobbyInfo, LobbyInfo, NetMode};
use crate::multiplayer::transport::ActiveTransport;
use std::collections::VecDeque;
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

/// Cap on queued script-facing events. Every [`crate::multiplayer::poll`]
/// pushes here from remote-driven traffic, and only `drain_events` empties it,
/// so a game that stops draining (load screen, paused scene, script error)
/// otherwise grows this without bound on attacker-influenced payload bytes.
/// Mirrors the steam event queue cap.
pub const NET_EVENT_QUEUE_CAPACITY: usize = 1024;

/// Bounded [`NetEvent`] queue. State events (`LobbyRowsChanged`) coalesce onto
/// the newest copy; on saturation a state event is dropped first, else the
/// oldest event (a payload) goes, always with a `dropped` count.
pub struct NetEventQueue {
    events: VecDeque<NetEvent>,
    dropped: u64,
    coalesced: u64,
    warned: bool,
}

/// Queue counters for tests + diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetEventQueueStats {
    pub capacity: usize,
    pub len: usize,
    pub dropped: u64,
    pub coalesced: u64,
}

impl NetEventQueue {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            dropped: 0,
            coalesced: 0,
            warned: false,
        }
    }

    pub fn push(&mut self, event: NetEvent) {
        // Scan only for state events: payloads are the hot path and never
        // coalesce, so they must not pay a queue walk per packet.
        if is_coalescible(&event)
            && let Some(index) = self
                .events
                .iter()
                .position(|queued| coalesces(queued, &event))
        {
            self.events.remove(index);
            self.events.push_back(event);
            self.coalesced = self.coalesced.saturating_add(1);
            return;
        }

        if self.events.len() >= NET_EVENT_QUEUE_CAPACITY {
            let drop_index = self.events.iter().position(is_coalescible).unwrap_or(0);
            self.events.remove(drop_index);
            self.dropped = self.dropped.saturating_add(1);
            if !self.warned {
                self.warned = true;
                perro_modules::log_warn!(
                    "[net] script event queue full (cap={}) -- dropping oldest; call drain_events() every frame",
                    NET_EVENT_QUEUE_CAPACITY
                );
            }
        }
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> Vec<NetEvent> {
        self.events.drain(..).collect()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn stats(&self) -> NetEventQueueStats {
        NetEventQueueStats {
            capacity: NET_EVENT_QUEUE_CAPACITY,
            len: self.events.len(),
            dropped: self.dropped,
            coalesced: self.coalesced,
        }
    }
}

impl Default for NetEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn is_coalescible(event: &NetEvent) -> bool {
    matches!(event, NetEvent::LobbyRowsChanged)
}

fn coalesces(queued: &NetEvent, incoming: &NetEvent) -> bool {
    matches!(
        (queued, incoming),
        (NetEvent::LobbyRowsChanged, NetEvent::LobbyRowsChanged)
    )
}

pub enum Session {
    Host(crate::multiplayer::host_session::HostSession),
    Client(crate::multiplayer::client_session::ClientSession),
}

pub struct NetworkState {
    pub mode: NetMode,
    pub session: Option<Session>,
    pub transport: Option<ActiveTransport>,
    pub script_events: NetEventQueue,
    pub lobbies: Vec<LobbyInfo>,
    pub friends: Vec<FriendLobbyInfo>,
    pub join_tokens: Vec<(i64, i64)>,
    pub hosted_lobby_code: String,
    pub pending_private_host_max_players: i64,
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
            script_events: NetEventQueue::new(),
            lobbies: Vec::new(),
            friends: Vec::new(),
            join_tokens: Vec::new(),
            hosted_lobby_code: String::new(),
            pending_private_host_max_players: 0,
            lan_discovery: None,
            lan_host_addr: None,
            heartbeat: HeartbeatConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lobby_row_events_coalesce_onto_the_newest() {
        let mut queue = NetEventQueue::new();
        queue.push(NetEvent::LobbyRowsChanged);
        queue.push(NetEvent::Connected);
        queue.push(NetEvent::LobbyRowsChanged);

        let stats = queue.stats();
        assert_eq!(stats.len, 2);
        assert_eq!(stats.coalesced, 1);
        assert_eq!(stats.dropped, 0);
        assert!(matches!(
            queue.drain().as_slice(),
            [NetEvent::Connected, NetEvent::LobbyRowsChanged]
        ));
    }

    #[test]
    fn payload_flood_caps_the_queue_and_counts_drops() {
        let mut queue = NetEventQueue::new();
        for index in 0..(NET_EVENT_QUEUE_CAPACITY + 32) {
            queue.push(NetEvent::Payload {
                from_slot: 1,
                bytes: vec![index as u8; 16],
            });
        }

        let stats = queue.stats();
        assert_eq!(stats.capacity, NET_EVENT_QUEUE_CAPACITY);
        assert_eq!(stats.len, NET_EVENT_QUEUE_CAPACITY);
        assert_eq!(stats.dropped, 32);

        // Oldest payloads went first, newest survived.
        let events = queue.drain();
        let last = events.last().expect("queue keeps the newest payload");
        assert!(matches!(
            last,
            NetEvent::Payload { bytes, .. } if bytes[0] == (NET_EVENT_QUEUE_CAPACITY + 31) as u8
        ));
    }

    #[test]
    fn saturation_drops_state_events_before_payloads() {
        let mut queue = NetEventQueue::new();
        queue.push(NetEvent::LobbyRowsChanged);
        for _ in 1..NET_EVENT_QUEUE_CAPACITY {
            queue.push(NetEvent::Payload {
                from_slot: 1,
                bytes: vec![7],
            });
        }
        queue.push(NetEvent::Disconnected);

        assert_eq!(queue.stats().len, NET_EVENT_QUEUE_CAPACITY);
        assert_eq!(queue.stats().dropped, 1);
        let events = queue.drain();
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, NetEvent::LobbyRowsChanged))
        );
        assert!(matches!(events.last(), Some(NetEvent::Disconnected)));
    }
}
