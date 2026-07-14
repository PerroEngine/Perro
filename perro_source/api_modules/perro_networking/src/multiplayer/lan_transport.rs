use crate::multiplayer::transport::{NetTransport, PeerId, TransportEvent};
use std::net::{SocketAddr, UdpSocket};

const LAN_HOST_ADDR: &str = "0.0.0.0:7777";
const LAN_CLIENT_ADDR: &str = "0.0.0.0:0";
const MAX_PACKET_BYTES: usize = 16384;
pub const LAN_JOIN_TOKEN: i64 = -1;
pub const LAN_DISCOVER: &[u8] = b"mm_discover";
pub const LAN_DISCOVER_REPLY: &[u8] = b"mm_here";

fn default_host_addr() -> SocketAddr {
    LAN_HOST_ADDR
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 7777)))
}

pub struct LanTransport {
    is_host: bool,
    socket: Option<UdpSocket>,
    host_addr: SocketAddr,
    peers: Vec<SocketAddr>,
    pending_events: Vec<TransportEvent>,
    recv_buf: Box<[u8; MAX_PACKET_BYTES]>,
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

    fn add_peer(&mut self, addr: SocketAddr) {
        if self.peers.contains(&addr) {
            return;
        }
        self.peers.push(addr);
        self.pending_events
            .push(TransportEvent::PeerConnected(PeerId::Lan(addr)));
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
        let _ = socket.send_to(b"connect", self.host_addr);
        self.socket = Some(socket);
        self.add_peer(self.host_addr);
        Ok(())
    }

    fn send(&mut self, peer: &PeerId, bytes: &[u8], _reliable: bool) {
        let Some(socket) = &self.socket else {
            return;
        };
        if let PeerId::Lan(addr) = peer {
            let _ = socket.send_to(bytes, addr);
        }
    }

    fn broadcast(&mut self, bytes: &[u8], reliable: bool) {
        let _ = reliable;
        let Some(socket) = &self.socket else {
            return;
        };
        for peer in &self.peers {
            let _ = socket.send_to(bytes, peer);
        }
    }

    fn drain_events(&mut self) -> Vec<TransportEvent> {
        let mut out = std::mem::take(&mut self.pending_events);

        // Take the socket out for the loop so `add_peer` can borrow self —
        // avoids the per-frame try_clone syscall this used to do.
        let Some(socket) = self.socket.take() else {
            return out;
        };
        loop {
            match socket.recv_from(&mut self.recv_buf[..]) {
                Ok((len, addr)) => {
                    let packet = &self.recv_buf[..len];
                    if self.is_host && packet == LAN_DISCOVER {
                        let _ = socket.send_to(LAN_DISCOVER_REPLY, addr);
                        continue;
                    }
                    let bytes = packet.to_vec();
                    if self.is_host {
                        self.add_peer(addr);
                    }
                    out.append(&mut self.pending_events);
                    out.push(TransportEvent::PacketReceived(PeerId::Lan(addr), bytes));
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        self.socket = Some(socket);
        out
    }

    fn shutdown(&mut self) {
        for peer in self.peers.drain(..) {
            self.pending_events
                .push(TransportEvent::PeerDisconnected(PeerId::Lan(peer)));
        }
        self.socket = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn add_peer_emits_connected_once() {
        let mut transport = LanTransport::new_host();
        let peer = addr(49152);

        transport.add_peer(peer);
        transport.add_peer(peer);

        let events = transport.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            TransportEvent::PeerConnected(PeerId::Lan(stored)) if stored == peer
        ));
    }

    #[test]
    fn shutdown_emits_disconnect_for_known_peer() {
        let mut transport = LanTransport::new_host();
        let peer = addr(49153);

        transport.add_peer(peer);
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
    fn host_drain_emits_connect_and_packet_for_new_sender() {
        let socket = bound_socket();
        let host_addr = socket.local_addr().unwrap();
        let sender = bound_socket();
        let sender_addr = sender.local_addr().unwrap();
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);

        sender.send_to(b"hello", host_addr).unwrap();

        let events = drain_until(&mut transport, |events| {
            events
                .iter()
                .any(|event| matches!(event, TransportEvent::PacketReceived(_, bytes) if bytes == b"hello"))
        });
        assert!(events.iter().any(|event| matches!(
            event,
            TransportEvent::PeerConnected(PeerId::Lan(stored)) if *stored == sender_addr
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            TransportEvent::PacketReceived(PeerId::Lan(stored), bytes)
                if *stored == sender_addr && bytes == b"hello"
        )));
    }

    #[test]
    fn discovery_gets_reply_without_packet_event() {
        let socket = bound_socket();
        let host_addr = socket.local_addr().unwrap();
        let sender = bound_socket();
        sender.set_nonblocking(false).unwrap();
        sender
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut transport = LanTransport::new_host();
        transport.socket = Some(socket);

        sender.send_to(LAN_DISCOVER, host_addr).unwrap();

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
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        socket
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
