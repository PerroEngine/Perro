use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerId {
    Lan(SocketAddr),
    Steam(i64),
}

#[derive(Clone, Debug)]
pub enum TransportEvent {
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    PacketReceived(PeerId, Vec<u8>),
    SessionFailed,
}

pub trait NetTransport {
    fn host(&mut self) -> Result<(), String>;
    fn join(&mut self) -> Result<(), String>;
    fn send(&mut self, peer: &PeerId, bytes: &[u8], reliable: bool);
    fn broadcast(&mut self, bytes: &[u8], reliable: bool);
    fn drain_events(&mut self) -> Vec<TransportEvent>;
    fn shutdown(&mut self);
}

pub enum ActiveTransport {
    Lan(crate::multiplayer::lan_transport::LanTransport),
    Steam(crate::multiplayer::steam_transport::SteamTransport),
}

impl NetTransport for ActiveTransport {
    fn host(&mut self) -> Result<(), String> {
        match self {
            ActiveTransport::Lan(transport) => transport.host(),
            ActiveTransport::Steam(transport) => transport.host(),
        }
    }

    fn join(&mut self) -> Result<(), String> {
        match self {
            ActiveTransport::Lan(transport) => transport.join(),
            ActiveTransport::Steam(transport) => transport.join(),
        }
    }

    fn send(&mut self, peer: &PeerId, bytes: &[u8], reliable: bool) {
        match self {
            ActiveTransport::Lan(transport) => transport.send(peer, bytes, reliable),
            ActiveTransport::Steam(transport) => transport.send(peer, bytes, reliable),
        }
    }

    fn broadcast(&mut self, bytes: &[u8], reliable: bool) {
        match self {
            ActiveTransport::Lan(transport) => transport.broadcast(bytes, reliable),
            ActiveTransport::Steam(transport) => transport.broadcast(bytes, reliable),
        }
    }

    fn drain_events(&mut self) -> Vec<TransportEvent> {
        match self {
            ActiveTransport::Lan(transport) => transport.drain_events(),
            ActiveTransport::Steam(transport) => transport.drain_events(),
        }
    }

    fn shutdown(&mut self) {
        match self {
            ActiveTransport::Lan(transport) => transport.shutdown(),
            ActiveTransport::Steam(transport) => transport.shutdown(),
        }
    }
}
