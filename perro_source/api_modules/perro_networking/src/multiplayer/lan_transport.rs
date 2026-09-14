use crate::multiplayer::transport::{NetTransport, PeerId, TransportEvent};
use crate::multiplayer::wire;
use std::collections::VecDeque;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const LAN_HOST_ADDR: &str = "0.0.0.0:7777";
const LAN_CLIENT_ADDR: &str = "0.0.0.0:0";
const MAX_PACKET_BYTES: usize = 16384;
// Bound one game-thread poll, including discovery packets that emit no event.
const MAX_PACKETS_PER_POLL: usize = 256;
const MAX_BYTES_PER_POLL: usize = 1024 * 1024;
const MAX_LAN_PEERS: usize = 256;
const LAN_HELLO: &[u8] = b"perro_lan_hello_v1";
const LAN_CHALLENGE: &[u8] = b"perro_lan_challenge_v1";
const LAN_RESPONSE: &[u8] = b"perro_lan_response_v1";
const LAN_DATA: &[u8] = b"perro_lan_data_v1";
const LAN_HELLO_RETRY: Duration = Duration::from_millis(250);
const COOKIE_EPOCH_SECONDS: u64 = 5;
const MAX_ACCEPTED_HANDSHAKES: usize = MAX_LAN_PEERS * 4;
pub const LAN_JOIN_TOKEN: i64 = -1;
pub const LAN_DISCOVER: &[u8] = b"mm_discover";
pub const LAN_DISCOVER_REPLY: &[u8] = b"mm_here";

fn default_host_addr() -> SocketAddr {
    LAN_HOST_ADDR
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 7777)))
}

struct LanPeer {
    addr: SocketAddr,
    cookie: u64,
}

pub struct LanTransport {
    is_host: bool,
    socket: Option<UdpSocket>,
    host_addr: SocketAddr,
    peers: Vec<LanPeer>,
    pending_events: Vec<TransportEvent>,
    recv_buf: Box<[u8; MAX_PACKET_BYTES]>,
    cookie_key: RandomState,
    client_cookie: Option<u64>,
    client_nonce: Option<u64>,
    last_hello: Option<Instant>,
    accepted_handshakes: VecDeque<(SocketAddr, u64)>,
}

impl LanTransport {
    pub fn new_host() -> Self {
        Self::new(true, default_host_addr())
    }

    pub fn new_client() -> Self {
        Self::new(false, default_host_addr())
    }

    /// Host bound to an explicit address (tests / multiple hosts per machine).
    pub fn new_host_at(host_addr: SocketAddr) -> Self {
        Self::new(true, host_addr)
    }

    /// Client targeting an explicit host address.
    pub fn new_client_of(host_addr: SocketAddr) -> Self {
        Self::new(false, host_addr)
    }

    fn new(is_host: bool, host_addr: SocketAddr) -> Self {
        Self {
            is_host,
            socket: None,
            host_addr,
            peers: Vec::new(),
            pending_events: Vec::new(),
            recv_buf: Box::new([0; MAX_PACKET_BYTES]),
            cookie_key: RandomState::new(),
            client_cookie: None,
            client_nonce: None,
            last_hello: None,
            accepted_handshakes: VecDeque::new(),
        }
    }

    fn bind_addr(&self) -> SocketAddr {
        if self.is_host {
            self.host_addr
        } else {
            LAN_CLIENT_ADDR
                .parse()
                .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)))
        }
    }

    fn add_peer(&mut self, addr: SocketAddr, cookie: u64) -> bool {
        if let Some(peer) = self.peers.iter_mut().find(|peer| peer.addr == addr) {
            peer.cookie = cookie;
            return true;
        }
        if self.peers.len() >= MAX_LAN_PEERS {
            return false;
        }
        self.peers.push(LanPeer { addr, cookie });
        self.pending_events
            .push(TransportEvent::PeerConnected(PeerId::Lan(addr)));
        true
    }

    fn cookie(&self, addr: SocketAddr, nonce: u64, epoch: u64) -> u64 {
        let mut hasher = self.cookie_key.build_hasher();
        b"perro-lan-cookie-v1".hash(&mut hasher);
        addr.hash(&mut hasher);
        nonce.hash(&mut hasher);
        epoch.hash(&mut hasher);
        hasher.finish()
    }

    fn cookie_epoch() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / COOKIE_EPOCH_SECONDS
    }

    fn valid_response_cookie(&self, addr: SocketAddr, nonce: u64, cookie: u64, epoch: u64) -> bool {
        cookie == self.cookie(addr, nonce, epoch)
            || epoch
                .checked_sub(1)
                .is_some_and(|prior| cookie == self.cookie(addr, nonce, prior))
    }

    fn tagged_cookie(tag: &[u8], cookie: u64) -> Vec<u8> {
        let mut packet = Vec::with_capacity(tag.len() + 8);
        packet.extend_from_slice(tag);
        packet.extend_from_slice(&cookie.to_le_bytes());
        packet
    }

    fn parse_tagged_cookie(packet: &[u8], tag: &[u8]) -> Option<u64> {
        let bytes = packet.strip_prefix(tag)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    }

    fn tagged_cookie_pair(tag: &[u8], nonce: u64, cookie: u64) -> Vec<u8> {
        let mut packet = Self::tagged_cookie(tag, nonce);
        packet.extend_from_slice(&cookie.to_le_bytes());
        packet
    }

    fn parse_tagged_cookie_pair(packet: &[u8], tag: &[u8]) -> Option<(u64, u64)> {
        let bytes = packet.strip_prefix(tag)?;
        let (nonce, cookie) = bytes.split_at_checked(8)?;
        Some((
            u64::from_le_bytes(nonce.try_into().ok()?),
            u64::from_le_bytes(cookie.try_into().ok()?),
        ))
    }

    fn hello_packet(&self) -> Option<Vec<u8>> {
        Some(Self::tagged_cookie(LAN_HELLO, self.client_nonce?))
    }

    fn wrap_data(cookie: u64, bytes: &[u8]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(LAN_DATA.len() + 8 + bytes.len());
        packet.extend_from_slice(LAN_DATA);
        packet.extend_from_slice(&cookie.to_le_bytes());
        packet.extend_from_slice(bytes);
        packet
    }

    fn unwrap_data(packet: &[u8]) -> Option<(u64, &[u8])> {
        let bytes = packet.strip_prefix(LAN_DATA)?;
        let (cookie, payload) = bytes.split_at_checked(8)?;
        Some((u64::from_le_bytes(cookie.try_into().ok()?), payload))
    }

    fn drain_with_budget(
        &mut self,
        packet_budget: usize,
        byte_budget: usize,
    ) -> Vec<TransportEvent> {
        let mut out = std::mem::take(&mut self.pending_events);
        // Take ownership instead of cloning the socket on every poll.
        let Some(socket) = self.socket.take() else {
            return out;
        };
        if !self.is_host
            && self.peers.is_empty()
            && self
                .last_hello
                .is_none_or(|sent| sent.elapsed() >= LAN_HELLO_RETRY)
        {
            if let Some(hello) = self.hello_packet() {
                let _ = socket.send_to(&hello, self.host_addr);
            }
            self.last_hello = Some(Instant::now());
        }
        let mut received_bytes = 0usize;
        for _ in 0..packet_budget {
            if received_bytes >= byte_budget {
                break;
            }
            match socket.recv_from(&mut self.recv_buf[..]) {
                Ok((len, addr)) => {
                    // Whole datagrams: the last packet may cross the byte budget
                    // by at most MAX_PACKET_BYTES - 1; never truncate to fit it.
                    received_bytes += len;
                    let packet = self.recv_buf[..len].to_vec();
                    if self.is_host && packet == LAN_DISCOVER {
                        let _ = socket.send_to(LAN_DISCOVER_REPLY, addr);
                        continue;
                    }
                    if self.is_host {
                        if let Some(nonce) = Self::parse_tagged_cookie(&packet, LAN_HELLO) {
                            let challenge = Self::tagged_cookie_pair(
                                LAN_CHALLENGE,
                                nonce,
                                self.cookie(addr, nonce, Self::cookie_epoch()),
                            );
                            let _ = socket.send_to(&challenge, addr);
                            continue;
                        }
                        let epoch = Self::cookie_epoch();
                        let response = Self::parse_tagged_cookie_pair(&packet, LAN_RESPONSE)
                            .filter(|(nonce, cookie)| {
                                self.valid_response_cookie(addr, *nonce, *cookie, epoch)
                            });
                        if let Some((nonce, cookie)) = response {
                            let existed = self.peers.iter().any(|peer| peer.addr == addr);
                            if !existed && self.accepted_handshakes.contains(&(addr, nonce)) {
                                continue;
                            }
                            if self.add_peer(addr, cookie) {
                                if existed {
                                    self.pending_events
                                        .push(TransportEvent::PeerConnected(PeerId::Lan(addr)));
                                }
                                if !existed {
                                    if self.accepted_handshakes.len() >= MAX_ACCEPTED_HANDSHAKES {
                                        self.accepted_handshakes.pop_front();
                                    }
                                    self.accepted_handshakes.push_back((addr, nonce));
                                }
                                self.pending_events.push(TransportEvent::PacketReceived(
                                    PeerId::Lan(addr),
                                    wire::encode_client_ready(),
                                ));
                                out.append(&mut self.pending_events);
                            }
                            continue;
                        }
                        if !self.peers.iter().any(|peer| peer.addr == addr) {
                            continue;
                        }
                    } else {
                        if addr != self.host_addr {
                            continue;
                        }
                        if self.peers.is_empty()
                            && let Some((nonce, cookie)) =
                                Self::parse_tagged_cookie_pair(&packet, LAN_CHALLENGE)
                            && self.client_nonce == Some(nonce)
                        {
                            self.client_cookie = Some(cookie);
                            let response = Self::tagged_cookie_pair(LAN_RESPONSE, nonce, cookie);
                            let _ = socket.send_to(&response, self.host_addr);
                            continue;
                        }
                    }
                    let Some((cookie, bytes)) = Self::unwrap_data(&packet) else {
                        continue;
                    };
                    let expected_cookie = if self.is_host {
                        self.peers
                            .iter()
                            .find(|peer| peer.addr == addr)
                            .map(|peer| peer.cookie)
                    } else {
                        self.client_cookie.filter(|_| addr == self.host_addr)
                    };
                    if expected_cookie != Some(cookie) || wire::parse(bytes).is_none() {
                        continue;
                    }
                    if !self.is_host
                        && !self.peers.iter().any(|peer| peer.addr == addr)
                        && !self.add_peer(addr, cookie)
                    {
                        continue;
                    }
                    out.append(&mut self.pending_events);
                    out.push(TransportEvent::PacketReceived(
                        PeerId::Lan(addr),
                        bytes.to_vec(),
                    ));
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        self.socket = Some(socket);
        out
    }
}

impl NetTransport for LanTransport {
    fn host(&mut self) -> Result<(), String> {
        let socket = UdpSocket::bind(self.bind_addr()).map_err(|err| err.to_string())?;
        socket
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        self.socket = Some(socket);
        Ok(())
    }

    fn join(&mut self) -> Result<(), String> {
        let socket = UdpSocket::bind(self.bind_addr()).map_err(|err| err.to_string())?;
        socket
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        let local_addr = socket.local_addr().map_err(|err| err.to_string())?;
        self.client_nonce = Some(self.cookie(local_addr, 0, Self::cookie_epoch()));
        if let Some(hello) = self.hello_packet() {
            let _ = socket.send_to(&hello, self.host_addr);
        }
        self.last_hello = Some(Instant::now());
        self.socket = Some(socket);
        Ok(())
    }

    fn send(&mut self, peer: &PeerId, bytes: &[u8], _reliable: bool) {
        let Some(socket) = &self.socket else {
            return;
        };
        if let PeerId::Lan(addr) = peer {
            let cookie = self
                .peers
                .iter()
                .find(|peer| peer.addr == *addr)
                .map(|peer| peer.cookie)
                .or(self.client_cookie);
            if let Some(cookie) = cookie {
                let _ = socket.send_to(&Self::wrap_data(cookie, bytes), addr);
            }
        }
    }

    fn broadcast(&mut self, bytes: &[u8], reliable: bool) {
        let _ = reliable;
        let Some(socket) = &self.socket else {
            return;
        };
        for peer in &self.peers {
            let _ = socket.send_to(&Self::wrap_data(peer.cookie, bytes), peer.addr);
        }
    }

    fn drain_events(&mut self) -> Vec<TransportEvent> {
        self.drain_with_budget(MAX_PACKETS_PER_POLL, MAX_BYTES_PER_POLL)
    }

    fn shutdown(&mut self) {
        for peer in self.peers.drain(..) {
            self.pending_events
                .push(TransportEvent::PeerDisconnected(PeerId::Lan(peer.addr)));
        }
        self.client_cookie = None;
        self.client_nonce = None;
        self.last_hello = None;
        self.socket = None;
    }

    fn forget_peer(&mut self, peer: &PeerId) {
        let PeerId::Lan(addr) = peer else {
            return;
        };
        self.peers.retain(|peer| peer.addr != *addr);
        if !self.is_host && *addr == self.host_addr {
            self.client_cookie = None;
            self.client_nonce = None;
            self.last_hello = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn receive_budgets_preserve_backlog_and_whole_packets() {
        let socket = bound_socket();
        let destination = socket.local_addr().expect("test setup/result must succeed");
        let sender = bound_socket();
        let sender_addr = sender.local_addr().expect("sender addr");
        let mut transport = LanTransport::new_client_of(sender_addr);
        transport.socket = Some(socket);
        transport.client_cookie = Some(7);
        assert!(transport.add_peer(sender_addr, 7));
        transport.pending_events.clear();
        for bytes in [b"one", b"two", b"end"] {
            sender
                .send_to(
                    &LanTransport::wrap_data(7, &wire::wrap_payload(bytes)),
                    destination,
                )
                .expect("test setup/result must succeed");
        }
        let mut events = Vec::new();
        while events.len() < 3 {
            wait_readable(
                transport
                    .socket
                    .as_ref()
                    .expect("test setup/result must succeed"),
            );
            let batch = transport.drain_with_budget(2, usize::MAX);
            assert!(batch.len() <= 2);
            events.extend(batch);
        }
        assert!(
            matches!(&events[2], TransportEvent::PacketReceived(_, bytes) if wire::parse(bytes) == Some(wire::Frame::Payload(b"end")))
        );
        sender
            .send_to(
                &LanTransport::wrap_data(7, &wire::wrap_payload(b"whole")),
                destination,
            )
            .expect("test setup/result must succeed");
        sender
            .send_to(
                &LanTransport::wrap_data(7, &wire::wrap_payload(b"later")),
                destination,
            )
            .expect("test setup/result must succeed");
        wait_readable(
            transport
                .socket
                .as_ref()
                .expect("test setup/result must succeed"),
        );
        let first = transport.drain_with_budget(8, 1);
        assert_eq!(first.len(), 1);
        assert!(
            matches!(&first[0], TransportEvent::PacketReceived(_, bytes) if wire::parse(bytes) == Some(wire::Frame::Payload(b"whole")))
        );
        wait_readable(
            transport
                .socket
                .as_ref()
                .expect("test setup/result must succeed"),
        );
        assert!(
            matches!(&transport.drain_events()[0], TransportEvent::PacketReceived(_, bytes) if wire::parse(bytes) == Some(wire::Frame::Payload(b"later")))
        );
    }

    #[test]
    fn discovery_packets_consume_poll_budget() {
        let socket = bound_socket();
        let destination = socket.local_addr().expect("test setup/result must succeed");
        let sender = bound_socket();
        let sender_addr = sender.local_addr().expect("sender addr");
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);
        assert!(transport.add_peer(sender_addr, 7));
        transport.pending_events.clear();
        sender
            .send_to(LAN_DISCOVER, destination)
            .expect("test setup/result must succeed");
        sender
            .send_to(
                &LanTransport::wrap_data(7, &wire::wrap_payload(b"payload")),
                destination,
            )
            .expect("test setup/result must succeed");
        wait_readable(
            transport
                .socket
                .as_ref()
                .expect("test setup/result must succeed"),
        );
        assert!(transport.drain_with_budget(1, usize::MAX).is_empty());
        wait_readable(
            transport
                .socket
                .as_ref()
                .expect("test setup/result must succeed"),
        );
        assert!(transport.drain_events().iter().any(
            |event| matches!(event, TransportEvent::PacketReceived(_, bytes) if wire::parse(bytes) == Some(wire::Frame::Payload(b"payload")))
        ));
    }

    #[test]
    fn add_peer_emits_connected_once() {
        let mut transport = LanTransport::new_host();
        let peer = addr(49152);

        assert!(transport.add_peer(peer, 1));
        assert!(transport.add_peer(peer, 2));

        let events = transport.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            TransportEvent::PeerConnected(PeerId::Lan(stored)) if stored == peer
        ));
        assert_eq!(transport.peers[0].cookie, 2);
    }

    #[test]
    fn shutdown_emits_disconnect_for_known_peer() {
        let mut transport = LanTransport::new_host();
        let peer = addr(49153);

        assert!(transport.add_peer(peer, 1));
        let _ = transport.drain_events();
        transport.shutdown();

        let events = transport.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            TransportEvent::PeerDisconnected(PeerId::Lan(stored)) if stored == peer
        ));
    }

    #[test]
    fn host_requires_cookie_response_before_connect() {
        let socket = bound_socket();
        let host_addr = socket.local_addr().expect("test setup must succeed");
        let sender = bound_socket();
        let sender_addr = sender.local_addr().expect("test setup must succeed");
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);

        sender
            .send_to(b"junk", host_addr)
            .expect("test setup must succeed");
        wait_readable(transport.socket.as_ref().expect("host socket"));
        assert!(transport.drain_events().is_empty());

        let nonce = 11;
        sender
            .send_to(&LanTransport::tagged_cookie(LAN_HELLO, nonce), host_addr)
            .expect("test setup must succeed");
        wait_readable(transport.socket.as_ref().expect("host socket"));
        assert!(transport.drain_events().is_empty());
        wait_readable(&sender);
        let mut challenge = [0_u8; 64];
        let (len, _) = sender.recv_from(&mut challenge).expect("challenge");
        let (challenge_nonce, cookie) =
            LanTransport::parse_tagged_cookie_pair(&challenge[..len], LAN_CHALLENGE)
                .expect("valid challenge");
        assert_eq!(challenge_nonce, nonce);
        let response = LanTransport::tagged_cookie_pair(LAN_RESPONSE, nonce, cookie);
        sender.send_to(&response, host_addr).expect("response");

        let events = drain_until(&mut transport, |events| {
            events.iter().any(|event| {
                matches!(
                    event,
                    TransportEvent::PacketReceived(_, bytes)
                        if wire::parse(bytes) == Some(wire::Frame::ClientReady)
                )
            })
        });
        assert!(events.iter().any(|event| matches!(
            event,
            TransportEvent::PeerConnected(PeerId::Lan(stored)) if *stored == sender_addr
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            TransportEvent::PacketReceived(PeerId::Lan(stored), bytes)
                if *stored == sender_addr
                    && wire::parse(bytes) == Some(wire::Frame::ClientReady)
        )));

        sender
            .send_to(&response, host_addr)
            .expect("retry response");
        let retry = drain_until(&mut transport, |events| !events.is_empty());
        assert!(retry
            .iter()
            .any(|event| matches!(event, TransportEvent::PeerConnected(PeerId::Lan(addr)) if *addr == sender_addr)));
        assert!(retry.iter().any(|event| matches!(
            event,
            TransportEvent::PacketReceived(_, bytes)
                if wire::parse(bytes) == Some(wire::Frame::ClientReady)
        )));

        transport.forget_peer(&PeerId::Lan(sender_addr));
        sender
            .send_to(&response, host_addr)
            .expect("replay response");
        wait_readable(transport.socket.as_ref().expect("host socket"));
        assert!(transport.drain_events().is_empty());
    }

    #[test]
    fn client_rejects_challenge_for_wrong_nonce() {
        let socket = bound_socket();
        let destination = socket.local_addr().expect("client addr");
        let sender = bound_socket();
        let sender_addr = sender.local_addr().expect("host addr");
        let mut transport = LanTransport::new_client_of(sender_addr);
        transport.socket = Some(socket);
        transport.client_nonce = Some(11);

        sender
            .send_to(
                &LanTransport::tagged_cookie_pair(LAN_CHALLENGE, 12, 99),
                destination,
            )
            .expect("spoof challenge");
        wait_readable(transport.socket.as_ref().expect("client socket"));
        assert!(transport.drain_events().is_empty());
        assert_eq!(transport.client_cookie, None);
    }

    #[test]
    fn peer_state_has_hard_cap() {
        let mut transport = LanTransport::new_host();
        for port in 1..=(MAX_LAN_PEERS as u16 + 1) {
            let accepted = transport.add_peer(addr(port), port as u64);
            assert_eq!(accepted, port as usize <= MAX_LAN_PEERS);
        }
        assert_eq!(transport.peers.len(), MAX_LAN_PEERS);
    }

    #[test]
    fn response_cookie_expires_after_prior_epoch() {
        let transport = LanTransport::new_host();
        let peer = addr(49155);
        let nonce = 77;
        let cookie = transport.cookie(peer, nonce, 10);
        assert!(transport.valid_response_cookie(peer, nonce, cookie, 10));
        assert!(transport.valid_response_cookie(peer, nonce, cookie, 11));
        assert!(!transport.valid_response_cookie(peer, nonce, cookie, 12));
    }

    #[test]
    fn reconnect_replaces_cookie_used_for_outbound_data() {
        let socket = bound_socket();
        let sender = bound_socket();
        let sender_addr = sender.local_addr().expect("peer addr");
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);
        assert!(transport.add_peer(sender_addr, 1));
        assert!(transport.add_peer(sender_addr, 2));
        transport.broadcast(&wire::encode_heartbeat(), false);
        wait_readable(&sender);
        let mut packet = [0_u8; 128];
        let (len, _) = sender.recv_from(&mut packet).expect("data");
        let (cookie, payload) = LanTransport::unwrap_data(&packet[..len]).expect("envelope");
        assert_eq!(cookie, 2);
        assert_eq!(wire::parse(payload), Some(wire::Frame::Heartbeat));
    }

    #[test]
    fn established_peer_packets_require_cookie_and_frame() {
        let socket = bound_socket();
        let host_addr = socket.local_addr().expect("host addr");
        let sender = bound_socket();
        let sender_addr = sender.local_addr().expect("sender addr");
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);
        assert!(transport.add_peer(sender_addr, 7));
        transport.pending_events.clear();

        for packet in [
            wire::wrap_payload(b"raw"),
            LanTransport::wrap_data(8, &wire::wrap_payload(b"wrong cookie")),
            LanTransport::wrap_data(7, b"bad frame"),
        ] {
            sender.send_to(&packet, host_addr).expect("send invalid");
            wait_readable(transport.socket.as_ref().expect("host socket"));
            assert!(transport.drain_events().is_empty());
        }

        sender
            .send_to(
                &LanTransport::wrap_data(7, &wire::wrap_payload(b"ok")),
                host_addr,
            )
            .expect("send valid");
        wait_readable(transport.socket.as_ref().expect("host socket"));
        assert!(matches!(
            &transport.drain_events()[0],
            TransportEvent::PacketReceived(_, bytes)
                if wire::parse(bytes) == Some(wire::Frame::Payload(b"ok"))
        ));
    }

    #[test]
    fn discovery_gets_reply_without_packet_event() {
        let socket = bound_socket();
        let host_addr = socket.local_addr().expect("test setup must succeed");
        let sender = bound_socket();
        sender
            .set_nonblocking(false)
            .expect("test setup must succeed");
        sender
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("test setup must succeed");
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);

        sender
            .send_to(LAN_DISCOVER, host_addr)
            .expect("test setup must succeed");

        let mut events = Vec::new();
        let mut got_reply = false;
        let mut buf = [0_u8; 64];
        for _ in 0..10 {
            events.extend(transport.drain_events());
            match sender.recv_from(&mut buf) {
                Ok((len, _)) => {
                    assert_eq!(&buf[..len], LAN_DISCOVER_REPLY);
                    got_reply = true;
                    break;
                }
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("discovery recv failed: {err}"),
            }
        }
        assert!(got_reply);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, TransportEvent::PacketReceived(_, _)))
        );
    }

    fn bound_socket() -> UdpSocket {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("test setup must succeed");
        socket
            .set_nonblocking(true)
            .expect("test setup must succeed");
        socket
    }

    fn wait_readable(socket: &UdpSocket) {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        let mut byte = [0; MAX_PACKET_BYTES];
        loop {
            match socket.peek_from(&mut byte) {
                Ok(_) => return,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "loopback packet deadline"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(err) => panic!("loopback peek: {err}"),
            }
        }
    }

    fn drain_until(
        transport: &mut LanTransport,
        done: impl Fn(&[TransportEvent]) -> bool,
    ) -> Vec<TransportEvent> {
        for _ in 0..10 {
            let events = transport.drain_events();
            if done(&events) {
                return events;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        transport.drain_events()
    }

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }
}
