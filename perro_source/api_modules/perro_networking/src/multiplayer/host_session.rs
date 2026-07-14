use crate::multiplayer::heartbeat::HeartbeatConfig;
use crate::multiplayer::state::NetEvent;
use crate::multiplayer::transport::{NetTransport, PeerId, TransportEvent};
use crate::multiplayer::wire::{self, Frame};
use std::time::Instant;

struct Peer {
    id: PeerId,
    slot: i64,
    last_seen: Instant,
}

// A peer's SteamID for the game to fetch its avatar with. Steam transport
// carries the real id; local transport has no Steam identity, so 0.
fn peer_steam_id(peer: &PeerId) -> i64 {
    match peer {
        PeerId::Steam(id) => *id,
        PeerId::Lan(_) => 0,
    }
}

pub struct HostSession {
    peers: Vec<Peer>,
    next_slot: i64,
    free_slots: Vec<i64>,
    // Reused across sends so steady-state framing never allocates.
    scratch: Vec<u8>,
    // Last time we put anything on the wire; heartbeats only fill silence.
    last_sent: Option<Instant>,
}

impl HostSession {
    pub fn new() -> Self {
        Self {
            peers: Vec::new(),
            next_slot: 2,
            free_slots: Vec::new(),
            scratch: Vec::new(),
            last_sent: None,
        }
    }

    pub fn handle_transport_events(
        &mut self,
        transport: &mut impl NetTransport,
        events: Vec<TransportEvent>,
    ) -> Vec<NetEvent> {
        let mut out = Vec::new();
        self.handle_transport_events_into(transport, events, &mut out);
        out
    }

    pub fn handle_transport_events_into(
        &mut self,
        transport: &mut impl NetTransport,
        events: Vec<TransportEvent>,
        out: &mut Vec<NetEvent>,
    ) {
        for event in events {
            match event {
                TransportEvent::PeerConnected(peer) => {
                    let slot = self.slot_for_peer(&peer);
                    let steam_id = peer_steam_id(&peer);
                    wire::encode_slot_assigned_into(&mut self.scratch, slot);
                    transport.send(&peer, &self.scratch, true);
                    self.last_sent = Some(Instant::now());
                    perro_modules::log_info!(
                        "[net] host assigned client slot={} wait_client_ready",
                        slot
                    );
                    out.push(NetEvent::PeerJoined { slot, steam_id });
                }
                TransportEvent::PeerDisconnected(peer) => {
                    if let Some(slot) = self.remove_peer(&peer) {
                        perro_modules::log_info!("[net] host peer left slot={}", slot);
                        out.push(NetEvent::PeerLeft { slot });
                    }
                }
                TransportEvent::SessionFailed => {
                    out.push(NetEvent::Disconnected);
                }
                TransportEvent::PacketReceived(peer, mut bytes) => {
                    let slot = self.slot_for_peer(&peer);
                    if wire::strip_payload_in_place(&mut bytes) {
                        out.push(NetEvent::Payload {
                            from_slot: slot,
                            bytes,
                        });
                        continue;
                    }
                    match wire::parse(&bytes) {
                        Some(Frame::ClientReady) => {
                            // Re-send the slot assignment: the transport may be
                            // lossy UDP and the original SlotAssigned can drop.
                            wire::encode_slot_assigned_into(&mut self.scratch, slot);
                            transport.send(&peer, &self.scratch, true);
                            self.last_sent = Some(Instant::now());
                            out.push(NetEvent::PeerReady {
                                slot,
                                steam_id: peer_steam_id(&peer),
                            });
                        }
                        Some(Frame::ClientDisconnect) => {
                            if let Some(slot) = self.remove_peer(&peer) {
                                perro_modules::log_info!("[net] host peer quit slot={}", slot);
                                out.push(NetEvent::PeerLeft { slot });
                            }
                        }
                        // Heartbeats only bump last_seen (done above in
                        // slot_for_peer); clients never assign slots; drop
                        // stray/garbage frames.
                        Some(
                            Frame::Heartbeat
                            | Frame::SlotAssigned(_)
                            | Frame::HostDisconnect
                            | Frame::Payload(_),
                        )
                        | None => {}
                    }
                }
            }
        }
    }

    pub fn broadcast(&mut self, transport: &mut impl NetTransport, bytes: &[u8], reliable: bool) {
        wire::wrap_payload_into(&mut self.scratch, bytes);
        transport.broadcast(&self.scratch, reliable);
        self.last_sent = Some(Instant::now());
    }

    /// Send a game payload to one assigned slot. Returns false when the slot
    /// is not connected; clients cannot call this path through the public API.
    pub fn send_to_slot(
        &mut self,
        transport: &mut impl NetTransport,
        slot: i64,
        bytes: &[u8],
        reliable: bool,
    ) -> bool {
        let Some(peer) = self.peers.iter().find(|peer| peer.slot == slot) else {
            return false;
        };
        wire::wrap_payload_into(&mut self.scratch, bytes);
        transport.send(&peer.id, &self.scratch, reliable);
        self.last_sent = Some(Instant::now());
        true
    }

    pub fn send_host_disconnect(&mut self, transport: &mut impl NetTransport) {
        if self.peers.is_empty() {
            return;
        }
        wire::encode_host_disconnect_into(&mut self.scratch);
        transport.broadcast(&self.scratch, true);
        self.last_sent = Some(Instant::now());
    }

    /// Per-frame liveness. Drops peers we haven't heard from within the timeout
    /// (freeing their slot and emitting `PeerLeft`) and sends a heartbeat to all
    /// peers if we've gone quiet, so they can time *us* out too. No-op unless
    /// heartbeats are enabled. `now` is injected for deterministic tests.
    pub fn tick(
        &mut self,
        transport: &mut impl NetTransport,
        config: &HeartbeatConfig,
        now: Instant,
        out: &mut Vec<NetEvent>,
    ) {
        if !config.enabled {
            return;
        }
        let mut index = 0;
        while index < self.peers.len() {
            if now.saturating_duration_since(self.peers[index].last_seen) > config.timeout {
                let slot = self.peers.remove(index).slot;
                self.free_slots.push(slot);
                perro_modules::log_info!("[net] host peer timed out slot={}", slot);
                out.push(NetEvent::PeerLeft { slot });
            } else {
                index += 1;
            }
        }
        if !self.peers.is_empty() && self.silent_for(config, now) {
            wire::encode_heartbeat_into(&mut self.scratch);
            transport.broadcast(&self.scratch, false);
            self.last_sent = Some(now);
        }
    }

    fn silent_for(&self, config: &HeartbeatConfig, now: Instant) -> bool {
        match self.last_sent {
            Some(sent) => now.saturating_duration_since(sent) >= config.interval,
            None => true,
        }
    }

    fn slot_for_peer(&mut self, peer: &PeerId) -> i64 {
        let now = Instant::now();
        if let Some(existing) = self.peers.iter_mut().find(|entry| entry.id == *peer) {
            existing.last_seen = now;
            return existing.slot;
        }
        // Slots are seats, not counters: reuse the lowest freed slot so ids stay
        // bounded no matter how often clients rejoin.
        let slot = match self
            .free_slots
            .iter()
            .enumerate()
            .min_by_key(|(_, slot)| **slot)
        {
            Some((index, _)) => self.free_slots.swap_remove(index),
            None => {
                let slot = self.next_slot;
                self.next_slot += 1;
                slot
            }
        };
        self.peers.push(Peer {
            id: peer.clone(),
            slot,
            last_seen: now,
        });
        slot
    }

    fn remove_peer(&mut self, peer: &PeerId) -> Option<i64> {
        let index = self.peers.iter().position(|entry| entry.id == *peer)?;
        let slot = self.peers.remove(index).slot;
        self.free_slots.push(slot);
        Some(slot)
    }
}

impl Default for HostSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplayer::transport::{NetTransport, PeerId, TransportEvent};
    use crate::multiplayer::wire::{self, Frame};
    use std::net::SocketAddr;

    #[derive(Default)]
    struct MockTransport {
        sent: Vec<(PeerId, Vec<u8>, bool)>,
        broadcasts: Vec<(Vec<u8>, bool)>,
    }

    impl NetTransport for MockTransport {
        fn host(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn join(&mut self) -> Result<(), String> {
            Ok(())
        }

        fn send(&mut self, peer: &PeerId, bytes: &[u8], reliable: bool) {
            self.sent.push((peer.clone(), bytes.to_vec(), reliable));
        }

        fn broadcast(&mut self, bytes: &[u8], reliable: bool) {
            self.broadcasts.push((bytes.to_vec(), reliable));
        }

        fn drain_events(&mut self) -> Vec<TransportEvent> {
            Vec::new()
        }

        fn shutdown(&mut self) {}
    }

    #[test]
    fn steam_peer_gets_slot_and_ready_resends_assignment() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let peer = PeerId::Steam(99);

        let events = session.handle_transport_events(
            &mut transport,
            vec![
                TransportEvent::PeerConnected(peer.clone()),
                TransportEvent::PacketReceived(peer, wire::encode_client_ready()),
            ],
        );

        // Connect sends SlotAssigned, and ClientReady triggers a resend.
        assert_eq!(transport.sent.len(), 2);
        for (_, bytes, reliable) in &transport.sent {
            assert!(*reliable);
            assert_eq!(wire::parse(bytes), Some(Frame::SlotAssigned(2)));
        }
        assert!(matches!(events[0], NetEvent::PeerJoined { slot: 2, .. }));
        assert!(matches!(events[1], NetEvent::PeerReady { slot: 2, .. }));
    }

    #[test]
    fn local_payload_uses_same_host_path_as_steam() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let peer = PeerId::Lan("127.0.0.1:49152".parse::<SocketAddr>().unwrap());
        let game_bytes = vec![1, 2, 3];

        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PacketReceived(
                peer,
                wire::wrap_payload(&game_bytes),
            )],
        );

        match &events[0] {
            NetEvent::Payload { from_slot, bytes } => {
                assert_eq!(*from_slot, 2);
                assert_eq!(*bytes, game_bytes);
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }

    #[test]
    fn peer_disconnect_emits_peer_left() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let peer = PeerId::Steam(99);

        let events = session.handle_transport_events(
            &mut transport,
            vec![
                TransportEvent::PeerConnected(peer.clone()),
                TransportEvent::PeerDisconnected(peer),
            ],
        );

        assert!(matches!(events[1], NetEvent::PeerLeft { slot: 2 }));
    }

    #[test]
    fn client_disconnect_control_frees_slot() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let peer = PeerId::Steam(99);

        let events = session.handle_transport_events(
            &mut transport,
            vec![
                TransportEvent::PeerConnected(peer.clone()),
                TransportEvent::PacketReceived(peer.clone(), wire::encode_client_disconnect()),
                TransportEvent::PeerConnected(PeerId::Steam(100)),
            ],
        );

        assert!(matches!(events[1], NetEvent::PeerLeft { slot: 2 }));
        assert!(matches!(events[2], NetEvent::PeerJoined { slot: 2, .. }));
    }

    #[test]
    fn freed_slots_are_reused() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let first = PeerId::Steam(1);
        let second = PeerId::Steam(2);

        let _ = session.handle_transport_events(
            &mut transport,
            vec![
                TransportEvent::PeerConnected(first.clone()),
                TransportEvent::PeerDisconnected(first),
            ],
        );
        let events = session
            .handle_transport_events(&mut transport, vec![TransportEvent::PeerConnected(second)]);

        assert!(matches!(events[0], NetEvent::PeerJoined { slot: 2, .. }));
    }

    #[test]
    fn session_failed_disconnects_host() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();

        let events =
            session.handle_transport_events(&mut transport, vec![TransportEvent::SessionFailed]);

        assert!(matches!(events[0], NetEvent::Disconnected));
    }

    #[test]
    fn malformed_packet_is_ignored() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();

        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PacketReceived(
                PeerId::Steam(99),
                b"bad".to_vec(),
            )],
        );

        assert!(events.is_empty());
    }

    #[test]
    fn silent_peer_times_out_and_frees_its_slot() {
        use crate::multiplayer::heartbeat::HeartbeatConfig;
        use std::time::{Duration, Instant};

        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let config = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(5));
        session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(PeerId::Steam(99))],
        );

        // Far enough in the future that the peer's last_seen is stale.
        let mut out = Vec::new();
        session.tick(
            &mut transport,
            &config,
            Instant::now() + Duration::from_secs(6),
            &mut out,
        );

        assert!(matches!(out[0], NetEvent::PeerLeft { slot: 2 }));
        // Slot 2 is freed, so the next joiner reuses it rather than getting 3.
        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(PeerId::Steam(100))],
        );
        assert!(matches!(events[0], NetEvent::PeerJoined { slot: 2, .. }));
    }

    #[test]
    fn tick_sends_heartbeat_only_after_going_silent() {
        use crate::multiplayer::heartbeat::HeartbeatConfig;
        use std::time::{Duration, Instant};

        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let config = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(5));
        let base = Instant::now();
        session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(PeerId::Steam(99))],
        );
        transport.broadcasts.clear();

        // Within the interval: no heartbeat.
        let mut out = Vec::new();
        session.tick(&mut transport, &config, base, &mut out);
        assert!(transport.broadcasts.is_empty());

        // Past the interval with no other traffic: one heartbeat broadcast.
        session.tick(
            &mut transport,
            &config,
            base + Duration::from_secs(2),
            &mut out,
        );
        assert_eq!(transport.broadcasts.len(), 1);
        assert_eq!(
            wire::parse(&transport.broadcasts[0].0),
            Some(Frame::Heartbeat)
        );
    }

    #[test]
    fn tick_is_noop_when_heartbeat_disabled() {
        use crate::multiplayer::heartbeat::HeartbeatConfig;
        use std::time::{Duration, Instant};

        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(PeerId::Steam(99))],
        );

        let mut out = Vec::new();
        session.tick(
            &mut transport,
            &HeartbeatConfig::disabled(),
            Instant::now() + Duration::from_secs(60),
            &mut out,
        );

        assert!(out.is_empty());
    }

    #[test]
    fn broadcast_wraps_game_bytes() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let game_bytes = vec![9, 9, 9];

        session.broadcast(&mut transport, &game_bytes, false);

        assert_eq!(transport.broadcasts.len(), 1);
        assert!(!transport.broadcasts[0].1);
        assert_eq!(
            wire::parse(&transport.broadcasts[0].0),
            Some(Frame::Payload(&game_bytes[..]))
        );
    }

    #[test]
    fn targeted_payload_only_goes_to_requested_slot() {
        let mut session = HostSession::new();
        let mut transport = MockTransport::default();
        let first = PeerId::Steam(101);
        let second = PeerId::Steam(202);
        let _ = session.handle_transport_events(
            &mut transport,
            vec![
                TransportEvent::PeerConnected(first.clone()),
                TransportEvent::PeerConnected(second),
            ],
        );
        transport.sent.clear();

        assert!(session.send_to_slot(&mut transport, 2, &[7, 8], true));
        assert!(!session.send_to_slot(&mut transport, 99, &[7, 8], true));
        assert_eq!(transport.sent.len(), 1);
        assert_eq!(transport.sent[0].0, first);
        assert_eq!(
            wire::parse(&transport.sent[0].1),
            Some(Frame::Payload(&[7, 8]))
        );
    }
}
