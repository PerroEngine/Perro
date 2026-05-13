use std::{
    fmt,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket},
    string::FromUtf8Error,
};

use perro_ids::SignalID;
use perro_variant::Variant;
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

pub type NetResult<T> = Result<T, NetError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetErrorKind {
    AddressResolve,
    Bind,
    Connect,
    Accept,
    Send,
    Receive,
    SetNonBlocking,
    PeerAddress,
    LocalAddress,
    MissingHandle,
    FrameTooLarge,
    InvalidFrame,
    Handshake,
    WebSocket,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetError {
    pub kind: NetErrorKind,
    pub message: String,
}

impl NetError {
    pub fn new(kind: NetErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn from_io(kind: NetErrorKind, err: io::Error) -> Self {
        Self::new(kind, err.to_string())
    }
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for NetError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UdpPacket {
    pub peer: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetEvent {
    TcpConnected { peer: String },
    TcpClientConnected { peer: String },
    TcpData { peer: String, bytes: Vec<u8> },
    TcpDisconnected { peer: String },
    UdpPacket { peer: String, bytes: Vec<u8> },
    TcpFrame { peer: String, bytes: Vec<u8> },
    HeartbeatPing { peer: String },
    HeartbeatPong { peer: String },
    WebSocketConnected { peer: String },
    WebSocketClientConnected { peer: String },
    WebSocketText { peer: String, text: String },
    WebSocketBinary { peer: String, bytes: Vec<u8> },
    WebSocketPing { peer: String, bytes: Vec<u8> },
    WebSocketPong { peer: String, bytes: Vec<u8> },
    WebSocketClosed { peer: String },
    NetError { op: String, message: String },
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
            | NetEvent::WebSocketClientConnected { peer }
            | NetEvent::WebSocketClosed { peer } => vec![Variant::from(peer.clone())],
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
            NetEvent::NetError { op, message } => {
                vec![Variant::from(op.clone()), Variant::from(message.clone())]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TcpHostId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TcpConnectionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UdpEndpointId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WebSocketHostId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WebSocketConnectionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NetSource {
    TcpHost(TcpHostId),
    TcpConnection(TcpConnectionId),
    UdpEndpoint(UdpEndpointId),
    WebSocketHost(WebSocketHostId),
    WebSocketConnection(WebSocketConnectionId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkEvent {
    pub source: NetSource,
    pub event: NetEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetHandshake {
    pub app: String,
    pub protocol: String,
    pub version: u16,
}

impl NetHandshake {
    const MAGIC: &'static [u8] = b"PERRO_NET\0";

    pub fn new(app: impl Into<String>, protocol: impl Into<String>, version: u16) -> Self {
        Self {
            app: app.into(),
            protocol: protocol.into(),
            version,
        }
    }

    pub fn encode(&self) -> NetResult<Vec<u8>> {
        let app = self.app.as_bytes();
        let protocol = self.protocol.as_bytes();
        if app.len() > u16::MAX as usize || protocol.len() > u16::MAX as usize {
            return Err(NetError::new(
                NetErrorKind::Handshake,
                "handshake text too large",
            ));
        }

        let mut out = Vec::with_capacity(Self::MAGIC.len() + 6 + app.len() + protocol.len());
        out.extend_from_slice(Self::MAGIC);
        out.extend_from_slice(&self.version.to_be_bytes());
        out.extend_from_slice(&(app.len() as u16).to_be_bytes());
        out.extend_from_slice(&(protocol.len() as u16).to_be_bytes());
        out.extend_from_slice(app);
        out.extend_from_slice(protocol);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> NetResult<Self> {
        let min_len = Self::MAGIC.len() + 6;
        if bytes.len() < min_len || !bytes.starts_with(Self::MAGIC) {
            return Err(NetError::new(
                NetErrorKind::Handshake,
                "invalid handshake header",
            ));
        }

        let mut i = Self::MAGIC.len();
        let version = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
        i += 2;
        let app_len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2;
        let protocol_len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        i += 2;

        if bytes.len() != i + app_len + protocol_len {
            return Err(NetError::new(
                NetErrorKind::Handshake,
                "invalid handshake length",
            ));
        }

        let app = utf8(bytes[i..i + app_len].to_vec())?;
        i += app_len;
        let protocol = utf8(bytes[i..i + protocol_len].to_vec())?;

        Ok(Self {
            app,
            protocol,
            version,
        })
    }

    pub fn validate(&self, expected: &NetHandshake) -> NetResult<()> {
        if self == expected {
            return Ok(());
        }
        Err(NetError::new(
            NetErrorKind::Handshake,
            format!(
                "handshake mismatch: got {}/{}/{}, expected {}/{}/{}",
                self.app,
                self.protocol,
                self.version,
                expected.app,
                expected.protocol,
                expected.version
            ),
        ))
    }
}

pub struct TcpConnection {
    stream: TcpStream,
    peer: SocketAddr,
    frame_buf: Vec<u8>,
}

impl TcpConnection {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> NetResult<Self> {
        let stream = TcpStream::connect(addr)
            .map_err(|err| NetError::from_io(NetErrorKind::Connect, err))?;
        Self::from_stream(stream)
    }

    pub fn from_stream(stream: TcpStream) -> NetResult<Self> {
        let peer = stream
            .peer_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::PeerAddress, err))?;
        stream
            .set_nonblocking(true)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        Ok(Self {
            stream,
            peer,
            frame_buf: Vec::new(),
        })
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    pub fn peer_string(&self) -> String {
        self.peer.to_string()
    }

    pub fn connected_event(&self) -> NetEvent {
        NetEvent::TcpConnected {
            peer: self.peer_string(),
        }
    }

    pub fn read_available(&mut self, max_bytes: usize) -> NetResult<Option<Vec<u8>>> {
        let mut buf = vec![0_u8; max_bytes.max(1)];
        match self.stream.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                Ok(Some(buf))
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(NetError::from_io(NetErrorKind::Receive, err)),
        }
    }

    pub fn poll_event(&mut self, max_bytes: usize) -> NetResult<Option<NetEvent>> {
        let Some(bytes) = self.read_available(max_bytes)? else {
            return Ok(None);
        };
        let peer = self.peer_string();
        if bytes.is_empty() {
            return Ok(Some(NetEvent::TcpDisconnected { peer }));
        }
        Ok(Some(NetEvent::TcpData { peer, bytes }))
    }

    pub fn write(&mut self, bytes: &[u8]) -> NetResult<usize> {
        self.stream
            .write(bytes)
            .map_err(|err| NetError::from_io(NetErrorKind::Send, err))
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> NetResult<()> {
        self.stream
            .write_all(bytes)
            .map_err(|err| NetError::from_io(NetErrorKind::Send, err))
    }

    pub fn write_frame(&mut self, bytes: &[u8]) -> NetResult<()> {
        let frame = encode_frame(bytes)?;
        self.write_all(&frame)
    }

    pub fn write_handshake(&mut self, handshake: &NetHandshake) -> NetResult<()> {
        self.write_frame(&handshake.encode()?)
    }

    pub fn poll_frame(&mut self, max_frame_bytes: usize) -> NetResult<Option<Vec<u8>>> {
        self.read_into_frame_buf(max_frame_bytes)?;
        decode_next_frame(&mut self.frame_buf, max_frame_bytes)
    }

    pub fn poll_frame_event(&mut self, max_frame_bytes: usize) -> NetResult<Option<NetEvent>> {
        let Some(bytes) = self.poll_frame(max_frame_bytes)? else {
            return Ok(None);
        };
        let peer = self.peer_string();
        if is_heartbeat_ping(&bytes) {
            return Ok(Some(NetEvent::HeartbeatPing { peer }));
        }
        if is_heartbeat_pong(&bytes) {
            return Ok(Some(NetEvent::HeartbeatPong { peer }));
        }
        Ok(Some(NetEvent::TcpFrame { peer, bytes }))
    }

    pub fn poll_handshake(&mut self, max_frame_bytes: usize) -> NetResult<Option<NetHandshake>> {
        let Some(bytes) = self.poll_frame(max_frame_bytes)? else {
            return Ok(None);
        };
        NetHandshake::decode(&bytes).map(Some)
    }

    fn read_into_frame_buf(&mut self, max_frame_bytes: usize) -> NetResult<()> {
        let mut tmp = [0_u8; 4096];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => return Ok(()),
                Ok(n) => {
                    self.frame_buf.extend_from_slice(&tmp[..n]);
                    if self.frame_buf.len() > max_frame_bytes.saturating_add(4) {
                        return Err(NetError::new(
                            NetErrorKind::FrameTooLarge,
                            "tcp frame buffer exceeds max",
                        ));
                    }
                    if n < tmp.len() {
                        return Ok(());
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(err) => return Err(NetError::from_io(NetErrorKind::Receive, err)),
            }
        }
    }
}

pub struct TcpHost {
    listener: TcpListener,
    local: SocketAddr,
}

impl TcpHost {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> NetResult<Self> {
        let listener =
            TcpListener::bind(addr).map_err(|err| NetError::from_io(NetErrorKind::Bind, err))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        let local = listener
            .local_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::LocalAddress, err))?;
        Ok(Self { listener, local })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn accept(&self) -> NetResult<Option<TcpConnection>> {
        match self.listener.accept() {
            Ok((stream, _)) => TcpConnection::from_stream(stream).map(Some),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(NetError::from_io(NetErrorKind::Accept, err)),
        }
    }

    pub fn accept_event(&self) -> NetResult<Option<(TcpConnection, NetEvent)>> {
        let Some(connection) = self.accept()? else {
            return Ok(None);
        };
        let event = NetEvent::TcpClientConnected {
            peer: connection.peer_string(),
        };
        Ok(Some((connection, event)))
    }
}

pub struct UdpEndpoint {
    socket: UdpSocket,
    local: SocketAddr,
}

impl UdpEndpoint {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> NetResult<Self> {
        let socket =
            UdpSocket::bind(addr).map_err(|err| NetError::from_io(NetErrorKind::Bind, err))?;
        socket
            .set_nonblocking(true)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        let local = socket
            .local_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::LocalAddress, err))?;
        Ok(Self { socket, local })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn send_to<A: ToSocketAddrs>(&self, bytes: &[u8], addr: A) -> NetResult<usize> {
        let addr = first_addr(addr)?;
        self.socket
            .send_to(bytes, addr)
            .map_err(|err| NetError::from_io(NetErrorKind::Send, err))
    }

    pub fn recv_from(&self, max_bytes: usize) -> NetResult<Option<UdpPacket>> {
        let mut buf = vec![0_u8; max_bytes.max(1)];
        match self.socket.recv_from(&mut buf) {
            Ok((n, peer)) => {
                buf.truncate(n);
                Ok(Some(UdpPacket {
                    peer: peer.to_string(),
                    bytes: buf,
                }))
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(NetError::from_io(NetErrorKind::Receive, err)),
        }
    }

    pub fn poll_event(&self, max_bytes: usize) -> NetResult<Option<NetEvent>> {
        let Some(packet) = self.recv_from(max_bytes)? else {
            return Ok(None);
        };
        Ok(Some(NetEvent::UdpPacket {
            peer: packet.peer,
            bytes: packet.bytes,
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

enum WebSocketStream {
    Client(WebSocket<MaybeTlsStream<TcpStream>>),
    Server(WebSocket<TcpStream>),
}

impl WebSocketStream {
    fn read(&mut self) -> Result<Message, tungstenite::Error> {
        match self {
            Self::Client(socket) => socket.read(),
            Self::Server(socket) => socket.read(),
        }
    }

    fn send(&mut self, message: Message) -> Result<(), tungstenite::Error> {
        match self {
            Self::Client(socket) => socket.send(message),
            Self::Server(socket) => socket.send(message),
        }
    }

    fn close(&mut self) -> Result<(), tungstenite::Error> {
        match self {
            Self::Client(socket) => socket.close(None),
            Self::Server(socket) => socket.close(None),
        }
    }

    fn set_nonblocking(&mut self, nonblocking: bool) -> NetResult<()> {
        match self {
            Self::Client(socket) => set_maybe_tls_nonblocking(socket.get_mut(), nonblocking),
            Self::Server(socket) => socket
                .get_mut()
                .set_nonblocking(nonblocking)
                .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err)),
        }
    }
}

pub struct WebSocketConnection {
    socket: WebSocketStream,
    peer: String,
}

impl WebSocketConnection {
    pub fn connect(url: impl AsRef<str>) -> NetResult<Self> {
        let (socket, _) = tungstenite::connect(url.as_ref())
            .map_err(|err| NetError::new(NetErrorKind::Connect, err.to_string()))?;
        let peer = maybe_tls_peer_string(socket.get_ref()).unwrap_or_else(|| url.as_ref().into());
        let mut out = Self {
            socket: WebSocketStream::Client(socket),
            peer,
        };
        out.socket.set_nonblocking(true)?;
        Ok(out)
    }

    pub fn accept(stream: TcpStream) -> NetResult<Self> {
        stream
            .set_nonblocking(false)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        let peer = stream
            .peer_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::PeerAddress, err))?
            .to_string();
        let socket = tungstenite::accept(stream)
            .map_err(|err| NetError::new(NetErrorKind::Handshake, err.to_string()))?;
        let mut out = Self {
            socket: WebSocketStream::Server(socket),
            peer,
        };
        out.socket.set_nonblocking(true)?;
        Ok(out)
    }

    pub fn peer_string(&self) -> String {
        self.peer.clone()
    }

    pub fn connected_event(&self) -> NetEvent {
        NetEvent::WebSocketConnected {
            peer: self.peer_string(),
        }
    }

    pub fn read_message(&mut self, max_bytes: usize) -> NetResult<Option<WebSocketMessage>> {
        match self.socket.read() {
            Ok(message) => map_websocket_message(message, max_bytes).map(Some),
            Err(tungstenite::Error::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(tungstenite::Error::ConnectionClosed) | Err(tungstenite::Error::AlreadyClosed) => {
                Ok(Some(WebSocketMessage::Close))
            }
            Err(err) => Err(NetError::new(NetErrorKind::WebSocket, err.to_string())),
        }
    }

    pub fn poll_event(&mut self, max_bytes: usize) -> NetResult<Option<NetEvent>> {
        let Some(message) = self.read_message(max_bytes)? else {
            return Ok(None);
        };
        let peer = self.peer_string();
        Ok(Some(match message {
            WebSocketMessage::Text(text) => NetEvent::WebSocketText { peer, text },
            WebSocketMessage::Binary(bytes) => NetEvent::WebSocketBinary { peer, bytes },
            WebSocketMessage::Ping(bytes) => NetEvent::WebSocketPing { peer, bytes },
            WebSocketMessage::Pong(bytes) => NetEvent::WebSocketPong { peer, bytes },
            WebSocketMessage::Close => NetEvent::WebSocketClosed { peer },
        }))
    }

    pub fn send_text(&mut self, text: impl Into<String>) -> NetResult<()> {
        self.socket
            .send(Message::text(text.into()))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }

    pub fn send_binary(&mut self, bytes: Vec<u8>) -> NetResult<()> {
        self.socket
            .send(Message::binary(bytes))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }

    pub fn send_ping(&mut self, bytes: Vec<u8>) -> NetResult<()> {
        self.socket
            .send(Message::Ping(bytes.into()))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }

    pub fn send_pong(&mut self, bytes: Vec<u8>) -> NetResult<()> {
        self.socket
            .send(Message::Pong(bytes.into()))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }

    pub fn close(&mut self) -> NetResult<()> {
        self.socket
            .close()
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }
}

pub struct WebSocketHost {
    listener: TcpListener,
    local: SocketAddr,
}

impl WebSocketHost {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> NetResult<Self> {
        let listener =
            TcpListener::bind(addr).map_err(|err| NetError::from_io(NetErrorKind::Bind, err))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        let local = listener
            .local_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::LocalAddress, err))?;
        Ok(Self { listener, local })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn accept(&self) -> NetResult<Option<WebSocketConnection>> {
        match self.listener.accept() {
            Ok((stream, _)) => WebSocketConnection::accept(stream).map(Some),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(NetError::from_io(NetErrorKind::Accept, err)),
        }
    }

    pub fn accept_event(&self) -> NetResult<Option<(WebSocketConnection, NetEvent)>> {
        let Some(connection) = self.accept()? else {
            return Ok(None);
        };
        let event = NetEvent::WebSocketClientConnected {
            peer: connection.peer_string(),
        };
        Ok(Some((connection, event)))
    }
}

pub struct NetworkWorld {
    tcp_hosts: Vec<Option<TcpHost>>,
    tcp_connections: Vec<Option<TcpConnection>>,
    udp_endpoints: Vec<Option<UdpEndpoint>>,
    websocket_hosts: Vec<Option<WebSocketHost>>,
    websocket_connections: Vec<Option<WebSocketConnection>>,
}

impl NetworkWorld {
    pub fn new() -> Self {
        Self {
            tcp_hosts: Vec::new(),
            tcp_connections: Vec::new(),
            udp_endpoints: Vec::new(),
            websocket_hosts: Vec::new(),
            websocket_connections: Vec::new(),
        }
    }

    pub fn bind_tcp_host<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<TcpHostId> {
        let host = TcpHost::bind(addr)?;
        Ok(TcpHostId(insert_slot(&mut self.tcp_hosts, host)))
    }

    pub fn connect_tcp<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<TcpConnectionId> {
        let connection = TcpConnection::connect(addr)?;
        Ok(TcpConnectionId(insert_slot(
            &mut self.tcp_connections,
            connection,
        )))
    }

    pub fn bind_udp<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<UdpEndpointId> {
        let endpoint = UdpEndpoint::bind(addr)?;
        Ok(UdpEndpointId(insert_slot(
            &mut self.udp_endpoints,
            endpoint,
        )))
    }

    pub fn bind_websocket_host<A: ToSocketAddrs>(&mut self, addr: A) -> NetResult<WebSocketHostId> {
        let host = WebSocketHost::bind(addr)?;
        Ok(WebSocketHostId(insert_slot(
            &mut self.websocket_hosts,
            host,
        )))
    }

    pub fn connect_websocket(&mut self, url: impl AsRef<str>) -> NetResult<WebSocketConnectionId> {
        let connection = WebSocketConnection::connect(url)?;
        Ok(WebSocketConnectionId(insert_slot(
            &mut self.websocket_connections,
            connection,
        )))
    }

    pub fn tcp_host_addr(&self, id: TcpHostId) -> NetResult<SocketAddr> {
        Ok(self.tcp_host(id)?.local_addr())
    }

    pub fn tcp_peer_addr(&self, id: TcpConnectionId) -> NetResult<SocketAddr> {
        Ok(self.tcp_connection(id)?.peer_addr())
    }

    pub fn udp_addr(&self, id: UdpEndpointId) -> NetResult<SocketAddr> {
        Ok(self.udp_endpoint(id)?.local_addr())
    }

    pub fn websocket_host_addr(&self, id: WebSocketHostId) -> NetResult<SocketAddr> {
        Ok(self.websocket_host(id)?.local_addr())
    }

    pub fn tcp_send(&mut self, id: TcpConnectionId, bytes: &[u8]) -> NetResult<usize> {
        self.tcp_connection_mut(id)?.write(bytes)
    }

    pub fn tcp_send_frame(&mut self, id: TcpConnectionId, bytes: &[u8]) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_frame(bytes)
    }

    pub fn tcp_send_handshake(
        &mut self,
        id: TcpConnectionId,
        handshake: &NetHandshake,
    ) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_handshake(handshake)
    }

    pub fn tcp_send_heartbeat_ping(&mut self, id: TcpConnectionId) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_frame(heartbeat_ping())
    }

    pub fn tcp_send_heartbeat_pong(&mut self, id: TcpConnectionId) -> NetResult<()> {
        self.tcp_connection_mut(id)?.write_frame(heartbeat_pong())
    }

    pub fn udp_send_to<A: ToSocketAddrs>(
        &self,
        id: UdpEndpointId,
        bytes: &[u8],
        addr: A,
    ) -> NetResult<usize> {
        self.udp_endpoint(id)?.send_to(bytes, addr)
    }

    pub fn websocket_send_text(
        &mut self,
        id: WebSocketConnectionId,
        text: impl Into<String>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_text(text)
    }

    pub fn websocket_send_binary(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_binary(bytes)
    }

    pub fn websocket_send_ping(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_ping(bytes)
    }

    pub fn websocket_send_pong(
        &mut self,
        id: WebSocketConnectionId,
        bytes: Vec<u8>,
    ) -> NetResult<()> {
        self.websocket_connection_mut(id)?.send_pong(bytes)
    }

    pub fn websocket_close(&mut self, id: WebSocketConnectionId) -> NetResult<()> {
        self.websocket_connection_mut(id)?.close()
    }

    pub fn remove_tcp_host(&mut self, id: TcpHostId) -> bool {
        remove_slot(&mut self.tcp_hosts, id.0)
    }

    pub fn remove_tcp_connection(&mut self, id: TcpConnectionId) -> bool {
        remove_slot(&mut self.tcp_connections, id.0)
    }

    pub fn remove_udp(&mut self, id: UdpEndpointId) -> bool {
        remove_slot(&mut self.udp_endpoints, id.0)
    }

    pub fn remove_websocket_host(&mut self, id: WebSocketHostId) -> bool {
        remove_slot(&mut self.websocket_hosts, id.0)
    }

    pub fn remove_websocket_connection(&mut self, id: WebSocketConnectionId) -> bool {
        remove_slot(&mut self.websocket_connections, id.0)
    }

    pub fn poll_events(&mut self, max_per_socket: usize, max_bytes: usize) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        self.poll_accepts(max_per_socket, &mut events);
        self.poll_websocket_accepts(max_per_socket, &mut events);
        self.poll_tcp_data(max_per_socket, max_bytes, &mut events);
        self.poll_udp_packets(max_per_socket, max_bytes, &mut events);
        self.poll_websocket_messages(max_per_socket, max_bytes, &mut events);
        events
    }

    pub fn poll_frame_events(
        &mut self,
        max_per_socket: usize,
        max_frame_bytes: usize,
    ) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        self.poll_accepts(max_per_socket, &mut events);
        self.poll_websocket_accepts(max_per_socket, &mut events);
        self.poll_tcp_frames(max_per_socket, max_frame_bytes, &mut events);
        self.poll_udp_packets(max_per_socket, max_frame_bytes, &mut events);
        self.poll_websocket_messages(max_per_socket, max_frame_bytes, &mut events);
        events
    }

    fn poll_accepts(&mut self, max_per_socket: usize, events: &mut Vec<NetworkEvent>) {
        for host_index in 0..self.tcp_hosts.len() {
            let Some(host) = self.tcp_hosts[host_index].as_ref() else {
                continue;
            };
            for _ in 0..max_per_socket {
                match host.accept_event() {
                    Ok(Some((connection, event))) => {
                        let id =
                            TcpConnectionId(insert_slot(&mut self.tcp_connections, connection));
                        events.push(NetworkEvent {
                            source: NetSource::TcpConnection(id),
                            event,
                        });
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::TcpHost(TcpHostId(host_index as u32)),
                            "tcp_accept",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_tcp_data(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.tcp_connections.len() {
            let Some(connection) = self.tcp_connections[i].as_mut() else {
                continue;
            };
            let id = TcpConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_event(max_bytes) {
                    Ok(Some(event)) => {
                        let disconnected = matches!(event, NetEvent::TcpDisconnected { .. });
                        events.push(NetworkEvent {
                            source: NetSource::TcpConnection(id),
                            event,
                        });
                        if disconnected {
                            self.tcp_connections[i] = None;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::TcpConnection(id),
                            "tcp_recv",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_tcp_frames(
        &mut self,
        max_per_socket: usize,
        max_frame_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.tcp_connections.len() {
            let Some(connection) = self.tcp_connections[i].as_mut() else {
                continue;
            };
            let id = TcpConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_frame_event(max_frame_bytes) {
                    Ok(Some(event)) => events.push(NetworkEvent {
                        source: NetSource::TcpConnection(id),
                        event,
                    }),
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::TcpConnection(id),
                            "tcp_frame",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_udp_packets(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.udp_endpoints.len() {
            let Some(endpoint) = self.udp_endpoints[i].as_ref() else {
                continue;
            };
            let id = UdpEndpointId(i as u32);
            for _ in 0..max_per_socket {
                match endpoint.poll_event(max_bytes) {
                    Ok(Some(event)) => events.push(NetworkEvent {
                        source: NetSource::UdpEndpoint(id),
                        event,
                    }),
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(NetSource::UdpEndpoint(id), "udp_recv", err));
                        break;
                    }
                }
            }
        }
    }

    fn poll_websocket_accepts(&mut self, max_per_socket: usize, events: &mut Vec<NetworkEvent>) {
        for host_index in 0..self.websocket_hosts.len() {
            let Some(host) = self.websocket_hosts[host_index].as_ref() else {
                continue;
            };
            for _ in 0..max_per_socket {
                match host.accept_event() {
                    Ok(Some((connection, event))) => {
                        let id = WebSocketConnectionId(insert_slot(
                            &mut self.websocket_connections,
                            connection,
                        ));
                        events.push(NetworkEvent {
                            source: NetSource::WebSocketConnection(id),
                            event,
                        });
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::WebSocketHost(WebSocketHostId(host_index as u32)),
                            "websocket_accept",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn poll_websocket_messages(
        &mut self,
        max_per_socket: usize,
        max_bytes: usize,
        events: &mut Vec<NetworkEvent>,
    ) {
        for i in 0..self.websocket_connections.len() {
            let Some(connection) = self.websocket_connections[i].as_mut() else {
                continue;
            };
            let id = WebSocketConnectionId(i as u32);
            for _ in 0..max_per_socket {
                match connection.poll_event(max_bytes) {
                    Ok(Some(event)) => {
                        let disconnected = matches!(event, NetEvent::WebSocketClosed { .. });
                        events.push(NetworkEvent {
                            source: NetSource::WebSocketConnection(id),
                            event,
                        });
                        if disconnected {
                            self.websocket_connections[i] = None;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(err) => {
                        events.push(net_error_event(
                            NetSource::WebSocketConnection(id),
                            "websocket_recv",
                            err,
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn tcp_host(&self, id: TcpHostId) -> NetResult<&TcpHost> {
        get_slot(&self.tcp_hosts, id.0, "tcp host")
    }

    fn tcp_connection(&self, id: TcpConnectionId) -> NetResult<&TcpConnection> {
        get_slot(&self.tcp_connections, id.0, "tcp connection")
    }

    fn tcp_connection_mut(&mut self, id: TcpConnectionId) -> NetResult<&mut TcpConnection> {
        get_slot_mut(&mut self.tcp_connections, id.0, "tcp connection")
    }

    fn udp_endpoint(&self, id: UdpEndpointId) -> NetResult<&UdpEndpoint> {
        get_slot(&self.udp_endpoints, id.0, "udp endpoint")
    }

    fn websocket_host(&self, id: WebSocketHostId) -> NetResult<&WebSocketHost> {
        get_slot(&self.websocket_hosts, id.0, "websocket host")
    }

    fn websocket_connection_mut(
        &mut self,
        id: WebSocketConnectionId,
    ) -> NetResult<&mut WebSocketConnection> {
        get_slot_mut(
            &mut self.websocket_connections,
            id.0,
            "websocket connection",
        )
    }
}

impl Default for NetworkWorld {
    fn default() -> Self {
        Self::new()
    }
}

pub fn encode_frame(bytes: &[u8]) -> NetResult<Vec<u8>> {
    if bytes.len() > u32::MAX as usize {
        return Err(NetError::new(
            NetErrorKind::FrameTooLarge,
            "tcp frame exceeds u32 length",
        ));
    }
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(out)
}

pub fn decode_next_frame(
    buffer: &mut Vec<u8>,
    max_frame_bytes: usize,
) -> NetResult<Option<Vec<u8>>> {
    if buffer.len() < 4 {
        return Ok(None);
    }

    let len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    if len > max_frame_bytes {
        return Err(NetError::new(
            NetErrorKind::FrameTooLarge,
            "tcp frame exceeds max",
        ));
    }
    if buffer.len() < 4 + len {
        return Ok(None);
    }

    let frame = buffer[4..4 + len].to_vec();
    buffer.drain(0..4 + len);
    Ok(Some(frame))
}

pub fn heartbeat_ping() -> &'static [u8] {
    b"PERRO_HEARTBEAT_PING"
}

pub fn heartbeat_pong() -> &'static [u8] {
    b"PERRO_HEARTBEAT_PONG"
}

pub fn is_heartbeat_ping(bytes: &[u8]) -> bool {
    bytes == heartbeat_ping()
}

pub fn is_heartbeat_pong(bytes: &[u8]) -> bool {
    bytes == heartbeat_pong()
}

fn first_addr<A: ToSocketAddrs>(addr: A) -> NetResult<SocketAddr> {
    addr.to_socket_addrs()
        .map_err(|err| NetError::from_io(NetErrorKind::AddressResolve, err))?
        .next()
        .ok_or_else(|| NetError::new(NetErrorKind::AddressResolve, "no socket address resolved"))
}

fn utf8(bytes: Vec<u8>) -> NetResult<String> {
    String::from_utf8(bytes).map_err(|err: FromUtf8Error| {
        NetError::new(NetErrorKind::Handshake, format!("invalid utf8: {err}"))
    })
}

fn map_websocket_message(message: Message, max_bytes: usize) -> NetResult<WebSocketMessage> {
    if message.len() > max_bytes {
        return Err(NetError::new(
            NetErrorKind::FrameTooLarge,
            "websocket message exceeds max",
        ));
    }

    Ok(match message {
        Message::Text(text) => WebSocketMessage::Text(text.to_string()),
        Message::Binary(bytes) => WebSocketMessage::Binary(bytes.to_vec()),
        Message::Ping(bytes) => WebSocketMessage::Ping(bytes.to_vec()),
        Message::Pong(bytes) => WebSocketMessage::Pong(bytes.to_vec()),
        Message::Close(_) => WebSocketMessage::Close,
        Message::Frame(_) => {
            return Err(NetError::new(
                NetErrorKind::InvalidFrame,
                "raw websocket frame",
            ));
        }
    })
}

fn maybe_tls_peer_string(stream: &MaybeTlsStream<TcpStream>) -> Option<String> {
    match stream {
        MaybeTlsStream::Plain(stream) => stream.peer_addr().ok().map(|addr| addr.to_string()),
        _ => None,
    }
}

fn set_maybe_tls_nonblocking(
    stream: &mut MaybeTlsStream<TcpStream>,
    nonblocking: bool,
) -> NetResult<()> {
    match stream {
        MaybeTlsStream::Plain(stream) => stream
            .set_nonblocking(nonblocking)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err)),
        _ => Ok(()),
    }
}

fn insert_slot<T>(slots: &mut Vec<Option<T>>, value: T) -> u32 {
    if let Some(i) = slots.iter().position(Option::is_none) {
        slots[i] = Some(value);
        return i as u32;
    }
    slots.push(Some(value));
    (slots.len() - 1) as u32
}

fn remove_slot<T>(slots: &mut [Option<T>], id: u32) -> bool {
    let Some(slot) = slots.get_mut(id as usize) else {
        return false;
    };
    slot.take().is_some()
}

fn get_slot<'a, T>(slots: &'a [Option<T>], id: u32, label: &str) -> NetResult<&'a T> {
    slots
        .get(id as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| NetError::new(NetErrorKind::MissingHandle, format!("missing {label} {id}")))
}

fn get_slot_mut<'a, T>(slots: &'a mut [Option<T>], id: u32, label: &str) -> NetResult<&'a mut T> {
    slots
        .get_mut(id as usize)
        .and_then(Option::as_mut)
        .ok_or_else(|| NetError::new(NetErrorKind::MissingHandle, format!("missing {label} {id}")))
}

fn net_error_event(source: NetSource, op: &str, err: NetError) -> NetworkEvent {
    NetworkEvent {
        source,
        event: NetEvent::NetError {
            op: op.to_string(),
            message: err.to_string(),
        },
    }
}

#[macro_export]
macro_rules! emit_net_event {
    ($ctx:expr, $event:expr) => {{
        let event = $event;
        let params = event.signal_params();
        $ctx.Signals()
            .signal_emit(event.signal_id(), params.as_slice())
    }};
}

#[cfg(test)]
#[path = "../tests/unit/net_tests.rs"]
mod tests;
