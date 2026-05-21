use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum NetEvent {
    TcpConnected {
        peer: String,
    },
    TcpClientConnected {
        peer: String,
    },
    TcpData {
        peer: String,
        bytes: Vec<u8>,
    },
    TcpDisconnected {
        peer: String,
    },
    UdpPacket {
        peer: String,
        bytes: Vec<u8>,
    },
    TcpFrame {
        peer: String,
        bytes: Vec<u8>,
    },
    HeartbeatPing {
        peer: String,
    },
    HeartbeatPong {
        peer: String,
    },
    WebSocketConnected {
        peer: String,
    },
    WebSocketClientConnected {
        peer: String,
    },
    WebSocketText {
        peer: String,
        text: String,
    },
    WebSocketBinary {
        peer: String,
        bytes: Vec<u8>,
    },
    WebSocketVariant {
        peer: String,
        value: Variant,
    },
    WebSocketInvalidJson {
        peer: String,
        text: String,
        message: String,
    },
    WebSocketPing {
        peer: String,
        bytes: Vec<u8>,
    },
    WebSocketPong {
        peer: String,
        bytes: Vec<u8>,
    },
    WebSocketClosed {
        peer: String,
        code: Option<u16>,
        reason: String,
    },
    NetError {
        op: String,
        message: String,
    },
}

impl NetEvent {
    pub fn signal_name(&self) -> &'static str {
        match self {
            NetEvent::TcpConnected { .. } => "TCP_Connected",
            NetEvent::TcpClientConnected { .. } => "TCP_ClientConnected",
            NetEvent::TcpData { .. } => "TCP_Data",
            NetEvent::TcpDisconnected { .. } => "TCP_Disconnected",
            NetEvent::UdpPacket { .. } => "UDP_Packet",
            NetEvent::TcpFrame { .. } => "TCP_Frame",
            NetEvent::HeartbeatPing { .. } => "Net_HeartbeatPing",
            NetEvent::HeartbeatPong { .. } => "Net_HeartbeatPong",
            NetEvent::WebSocketConnected { .. } => "WebSocket_Connected",
            NetEvent::WebSocketClientConnected { .. } => "WebSocket_ClientConnected",
            NetEvent::WebSocketText { .. } => "WebSocket_Text",
            NetEvent::WebSocketBinary { .. } => "WebSocket_Binary",
            NetEvent::WebSocketVariant { .. } => "WebSocket_Variant",
            NetEvent::WebSocketInvalidJson { .. } => "WebSocket_InvalidJson",
            NetEvent::WebSocketPing { .. } => "WebSocket_Ping",
            NetEvent::WebSocketPong { .. } => "WebSocket_Pong",
            NetEvent::WebSocketClosed { .. } => "WebSocket_Closed",
            NetEvent::NetError { .. } => "Net_Error",
        }
    }

    pub fn signal_id(&self) -> SignalID {
        SignalID::from_string(self.signal_name())
    }

    pub fn signal_params(&self) -> Vec<Variant> {
        match self {
            NetEvent::TcpConnected { peer }
            | NetEvent::TcpClientConnected { peer }
            | NetEvent::TcpDisconnected { peer }
            | NetEvent::HeartbeatPing { peer }
            | NetEvent::HeartbeatPong { peer }
            | NetEvent::WebSocketConnected { peer }
            | NetEvent::WebSocketClientConnected { peer } => vec![Variant::from(peer.clone())],
            NetEvent::TcpData { peer, bytes }
            | NetEvent::UdpPacket { peer, bytes }
            | NetEvent::TcpFrame { peer, bytes }
            | NetEvent::WebSocketBinary { peer, bytes }
            | NetEvent::WebSocketPing { peer, bytes }
            | NetEvent::WebSocketPong { peer, bytes } => {
                vec![Variant::from(peer.clone()), Variant::from(bytes.clone())]
            }
            NetEvent::WebSocketText { peer, text } => {
                vec![Variant::from(peer.clone()), Variant::from(text.clone())]
            }
            NetEvent::WebSocketVariant { peer, value } => {
                vec![Variant::from(peer.clone()), value.clone()]
            }
            NetEvent::WebSocketInvalidJson {
                peer,
                text,
                message,
            } => vec![
                Variant::from(peer.clone()),
                Variant::from(text.clone()),
                Variant::from(message.clone()),
            ],
            NetEvent::WebSocketClosed { peer, code, reason } => vec![
                Variant::from(peer.clone()),
                Variant::from(code.unwrap_or(0)),
                Variant::from(reason.clone()),
            ],
            NetEvent::NetError { op, message } => {
                vec![Variant::from(op.clone()), Variant::from(message.clone())]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NetSource {
    TcpHost(TcpHostId),
    TcpConnection(TcpConnectionId),
    UdpEndpoint(UdpEndpointId),
    WebSocketHost(WebSocketHostId),
    WebSocketConnection(WebSocketConnectionId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetworkEvent {
    pub source: NetSource,
    pub event: NetEvent,
}
