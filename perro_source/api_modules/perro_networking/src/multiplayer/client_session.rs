use crate::multiplayer::heartbeat::HeartbeatConfig;
use crate::multiplayer::state::NetEvent;
use crate::multiplayer::transport::{NetTransport, PeerId, TransportEvent};
use crate::multiplayer::wire::{self, Frame};
use std::time::Instant;

pub struct ClientSession {
    host_peer: Option<PeerId>,
    // Reused across sends so steady-state framing never allocates.
    scratch: Vec<u8>,
    // Last time we heard from / sent to the host; drives heartbeat + timeout.
    last_seen: Option<Instant>,
    last_sent: Option<Instant>,
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            host_peer: None,
            scratch: Vec::new(),
            last_seen: None,
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
                    transport.send(&peer, &wire::encode_client_ready(), true);
                    self.host_peer = Some(peer);
                    let now = Instant::now();
                    self.last_seen = Some(now);
                    self.last_sent = Some(now);
                    out.push(NetEvent::Connected);
                }
                TransportEvent::PeerDisconnected(peer) => {
                    if self.host_peer.as_ref() == Some(&peer) {
                        self.host_peer = None;
                        out.push(NetEvent::Disconnected);
                    }
                }
                TransportEvent::SessionFailed => {
                    self.host_peer = None;
                    out.push(NetEvent::Disconnected);
                }
                TransportEvent::PacketReceived(peer, mut bytes) => {
                    if self.host_peer.is_none() {
                        self.host_peer = Some(peer);
                    }
                    // Any frame from the host proves it's alive.
                    self.last_seen = Some(Instant::now());
                    if wire::strip_payload_in_place(&mut bytes) {
                        out.push(NetEvent::Payload {
                            from_slot: 0,
                            bytes,
                        });
                        continue;
                    }
                    match wire::parse(&bytes) {
                        Some(Frame::SlotAssigned(slot)) => {
                            perro_modules::log_info!("[net] client assigned slot={}", slot);
                            out.push(NetEvent::SlotAssigned { slot });
                        }
                        // Heartbeats only bump last_seen (done above).
                        Some(Frame::HostDisconnect) => {
                            self.host_peer = None;
                            out.push(NetEvent::Disconnected);
                        }
                        Some(Frame::Heartbeat) => {}
                        Some(Frame::ClientReady | Frame::ClientDisconnect | Frame::Payload(_))
                        | None => {
                            perro_modules::log_info!(
                                "[net] client packet parse fail len={}",
                                bytes.len()
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn send_to_host(
        &mut self,
        transport: &mut impl NetTransport,
        bytes: &[u8],
        reliable: bool,
    ) {
        let Some(peer) = &self.host_peer else {
            return;
        };
        wire::wrap_payload_into(&mut self.scratch, bytes);
        transport.send(peer, &self.scratch, reliable);
        self.last_sent = Some(Instant::now());
    }

    /// Re-send the join hello. The initial hello (or the SlotAssigned reply)
    /// can drop on lossy transports; games retry this until assigned a slot.
    pub fn send_ready(&mut self, transport: &mut impl NetTransport) {
        let Some(peer) = &self.host_peer else {
            return;
        };
        transport.send(peer, &wire::encode_client_ready(), true);
        self.last_sent = Some(Instant::now());
    }

    pub fn send_disconnect(&mut self, transport: &mut impl NetTransport) {
        let Some(peer) = &self.host_peer else {
            return;
        };
        transport.send(peer, &wire::encode_client_disconnect(), true);
        self.last_sent = Some(Instant::now());
    }

    /// Per-frame liveness. If the host has gone silent past the timeout, tear
    /// down and emit `Disconnected`; otherwise send a heartbeat when we've been
    /// quiet so the host can time *us* out too. No-op unless enabled. `now` is
    /// injected for deterministic tests.
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
        let Some(peer) = self.host_peer.clone() else {
            return;
        };
        if let Some(seen) = self.last_seen
            && now.saturating_duration_since(seen) > config.timeout
        {
            perro_modules::log_info!("[net] client host timed out");
            self.host_peer = None;
            self.last_seen = None;
            out.push(NetEvent::Disconnected);
            return;
        }
        let silent = self
            .last_sent
            .is_none_or(|sent| now.saturating_duration_since(sent) >= config.interval);
        if silent {
            wire::encode_heartbeat_into(&mut self.scratch);
            transport.send(&peer, &self.scratch, false);
            self.last_sent = Some(now);
        }
    }
}

impl Default for ClientSession {
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
            self.send(&steam_peer(), bytes, reliable);
        }

        fn drain_events(&mut self) -> Vec<TransportEvent> {
            Vec::new()
        }

        fn shutdown(&mut self) {}
    }

    #[test]
    fn peer_connected_sends_ready_hello_for_steam() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();

        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(steam_peer())],
        );

        assert!(matches!(events[0], NetEvent::Connected));
        assert_eq!(transport.sent.len(), 1);
        assert_eq!(transport.sent[0].0, steam_peer());
        assert!(transport.sent[0].2);
        assert_eq!(wire::parse(&transport.sent[0].1), Some(Frame::ClientReady));
    }

    #[test]
    fn peer_connected_sends_ready_hello_for_local() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();
        let peer = local_peer();

        let _ = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(peer.clone())],
        );

        assert_eq!(transport.sent.len(), 1);
        assert_eq!(transport.sent[0].0, peer);
        assert_eq!(wire::parse(&transport.sent[0].1), Some(Frame::ClientReady));
    }

    #[test]
    fn slot_assignment_becomes_event() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();

        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PacketReceived(
                steam_peer(),
                wire::encode_slot_assigned(3),
            )],
        );

        assert!(matches!(events[0], NetEvent::SlotAssigned { slot: 3 }));
    }

    #[test]
    fn host_payload_passes_through_untouched() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();
        let game_bytes = vec![7, 7, 7];

        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PacketReceived(
                local_peer(),
                wire::wrap_payload(&game_bytes),
            )],
        );

        match &events[0] {
            NetEvent::Payload { bytes, .. } => assert_eq!(*bytes, game_bytes),
            event => panic!("unexpected event: {event:?}"),
        }
    }

    #[test]
    fn send_to_host_wraps_payload_and_targets_host() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();
        let _ = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(steam_peer())],
        );
        transport.sent.clear();

        session.send_to_host(&mut transport, &[5, 5], true);

        assert_eq!(transport.sent.len(), 1);
        assert_eq!(transport.sent[0].0, steam_peer());
        assert!(transport.sent[0].2);
        assert_eq!(
            wire::parse(&transport.sent[0].1),
            Some(Frame::Payload(&[5, 5][..]))
        );
    }

    #[test]
    fn send_before_connect_is_dropped() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();

        session.send_to_host(&mut transport, &[1], true);
        session.send_ready(&mut transport);

        assert!(transport.sent.is_empty());
    }

    #[test]
    fn silent_host_times_out_into_disconnect() {
        use crate::multiplayer::heartbeat::HeartbeatConfig;
        use std::time::{Duration, Instant};

        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();
        let config = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(5));
        session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(steam_peer())],
        );

        let mut out = Vec::new();
        session.tick(
            &mut transport,
            &config,
            Instant::now() + Duration::from_secs(6),
            &mut out,
        );

        assert!(matches!(out[0], NetEvent::Disconnected));
    }

    #[test]
    fn recent_host_traffic_keeps_client_alive() {
        use crate::multiplayer::heartbeat::HeartbeatConfig;
        use std::time::{Duration, Instant};

        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();
        let config = HeartbeatConfig::new(Duration::from_secs(1), Duration::from_secs(5));
        session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PeerConnected(steam_peer())],
        );

        // Just under the timeout: no disconnect, but a heartbeat goes out since
        // we've been quiet past the interval.
        transport.sent.clear();
        let mut out = Vec::new();
        session.tick(
            &mut transport,
            &config,
            Instant::now() + Duration::from_secs(3),
            &mut out,
        );

        assert!(out.is_empty());
        assert_eq!(transport.sent.len(), 1);
        assert_eq!(wire::parse(&transport.sent[0].1), Some(Frame::Heartbeat));
    }

    #[test]
    fn session_failed_disconnects_client() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();

        let events =
            session.handle_transport_events(&mut transport, vec![TransportEvent::SessionFailed]);

        assert!(matches!(events[0], NetEvent::Disconnected));
    }

    #[test]
    fn malformed_packet_is_ignored() {
        let mut session = ClientSession::new();
        let mut transport = MockTransport::default();

        let events = session.handle_transport_events(
            &mut transport,
            vec![TransportEvent::PacketReceived(
                steam_peer(),
                b"bad".to_vec(),
            )],
        );

        assert!(events.is_empty());
    }

    fn steam_peer() -> PeerId {
        PeerId::Steam(42)
    }

    fn local_peer() -> PeerId {
        PeerId::Lan(
            "127.0.0.1:7777"
                .parse::<SocketAddr>()
                .expect("test setup must succeed"),
        )
    }
}
