use super::*;

pub struct WebSocketConnectOptions {
    pub headers: Vec<(String, String)>,
    pub subprotocols: Vec<String>,
    pub max_message_bytes: usize,
}

impl WebSocketConnectOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn subprotocol(mut self, protocol: impl Into<String>) -> Self {
        self.subprotocols.push(protocol.into());
        self
    }

    pub fn variant_protocol(self) -> Self {
        self.subprotocol(WEBSOCKET_VARIANT_PROTOCOL)
    }

    pub fn max_message_bytes(mut self, max_message_bytes: usize) -> Self {
        self.max_message_bytes = max_message_bytes.max(1);
        self
    }
}

impl Default for WebSocketConnectOptions {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            subprotocols: Vec::new(),
            max_message_bytes: 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSocketHostOptions {
    pub subprotocols: Vec<String>,
    pub require_subprotocol: bool,
    pub max_message_bytes: usize,
}

impl WebSocketHostOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subprotocol(mut self, protocol: impl Into<String>) -> Self {
        self.subprotocols.push(protocol.into());
        self
    }

    pub fn variant_protocol(self) -> Self {
        self.subprotocol(WEBSOCKET_VARIANT_PROTOCOL)
    }

    pub fn require_subprotocol(mut self, require_subprotocol: bool) -> Self {
        self.require_subprotocol = require_subprotocol;
        self
    }

    pub fn max_message_bytes(mut self, max_message_bytes: usize) -> Self {
        self.max_message_bytes = max_message_bytes.max(1);
        self
    }
}

impl Default for WebSocketHostOptions {
    fn default() -> Self {
        Self {
            subprotocols: Vec::new(),
            require_subprotocol: false,
            max_message_bytes: 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebSocketHeartbeat {
    pub ping_after_ms: u64,
    pub timeout_after_ms: u64,
}

impl WebSocketHeartbeat {
    pub fn new(ping_after_ms: u64, timeout_after_ms: u64) -> Self {
        Self {
            ping_after_ms,
            timeout_after_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebSocketReconnectBackoff {
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    pub current_delay_ms: u64,
}

impl WebSocketReconnectBackoff {
    pub fn new(min_delay_ms: u64, max_delay_ms: u64) -> Self {
        let min_delay_ms = min_delay_ms.max(1);
        let max_delay_ms = max_delay_ms.max(min_delay_ms);
        Self {
            min_delay_ms,
            max_delay_ms,
            current_delay_ms: min_delay_ms,
        }
    }

    pub fn next_delay_ms(&mut self) -> u64 {
        let delay = self.current_delay_ms;
        self.current_delay_ms = self
            .current_delay_ms
            .saturating_mul(2)
            .min(self.max_delay_ms);
        delay
    }

    pub fn reset(&mut self) {
        self.current_delay_ms = self.min_delay_ms;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close { code: Option<u16>, reason: String },
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

    fn close(&mut self, frame: Option<CloseFrame>) -> Result<(), tungstenite::Error> {
        match self {
            Self::Client(socket) => socket.close(frame),
            Self::Server(socket) => socket.close(frame),
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
    selected_subprotocol: Option<String>,
    max_message_bytes: usize,
}

impl WebSocketConnection {
    pub fn connect(url: impl AsRef<str>) -> NetResult<Self> {
        Self::connect_with_options(url, WebSocketConnectOptions::default())
    }

    pub fn connect_with_options(
        url: impl AsRef<str>,
        options: WebSocketConnectOptions,
    ) -> NetResult<Self> {
        let (socket, response) = tungstenite::client::connect_with_config(
            build_websocket_request(url.as_ref(), &options)?,
            Some(websocket_config(options.max_message_bytes)),
            3,
        )
        .map_err(|err| NetError::new(NetErrorKind::Connect, err.to_string()))?;
        let peer = maybe_tls_peer_string(socket.get_ref()).unwrap_or_else(|| url.as_ref().into());
        let selected_subprotocol = response
            .headers()
            .get("Sec-WebSocket-Protocol")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let mut out = Self {
            socket: WebSocketStream::Client(socket),
            peer,
            selected_subprotocol,
            max_message_bytes: options.max_message_bytes,
        };
        out.socket.set_nonblocking(true)?;
        Ok(out)
    }

    pub fn accept(stream: TcpStream) -> NetResult<Self> {
        Self::accept_with_options(stream, WebSocketHostOptions::default())
    }

    pub fn accept_with_options(
        stream: TcpStream,
        options: WebSocketHostOptions,
    ) -> NetResult<Self> {
        stream
            .set_nonblocking(false)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        let peer = stream
            .peer_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::PeerAddress, err))?
            .to_string();
        let mut selected_subprotocol = None;
        let socket = accept_websocket_with_options(stream, &options, &mut selected_subprotocol)?;
        let mut out = Self {
            socket: WebSocketStream::Server(socket),
            peer,
            selected_subprotocol,
            max_message_bytes: options.max_message_bytes,
        };
        out.socket.set_nonblocking(true)?;
        Ok(out)
    }

    pub fn peer_string(&self) -> String {
        self.peer.clone()
    }

    pub fn selected_subprotocol(&self) -> Option<&str> {
        self.selected_subprotocol.as_deref()
    }

    pub fn max_message_bytes(&self) -> usize {
        self.max_message_bytes
    }

    pub fn connected_event(&self) -> NetEvent {
        NetEvent::WebSocketConnected {
            peer: self.peer_string(),
        }
    }

    pub fn read_message(&mut self, max_bytes: usize) -> NetResult<Option<WebSocketMessage>> {
        let max_bytes = max_bytes.min(self.max_message_bytes);
        match self.socket.read() {
            Ok(message) => map_websocket_message(message, max_bytes).map(Some),
            Err(tungstenite::Error::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(tungstenite::Error::ConnectionClosed) | Err(tungstenite::Error::AlreadyClosed) => {
                Ok(Some(WebSocketMessage::Close {
                    code: None,
                    reason: String::new(),
                }))
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
            WebSocketMessage::Close { code, reason } => {
                NetEvent::WebSocketClosed { peer, code, reason }
            }
        }))
    }

    pub fn poll_event_default(&mut self) -> NetResult<Option<NetEvent>> {
        self.poll_event(self.max_message_bytes)
    }

    pub fn poll_variant_event(&mut self, max_bytes: usize) -> NetResult<Option<NetEvent>> {
        let Some(message) = self.read_message(max_bytes)? else {
            return Ok(None);
        };
        let peer = self.peer_string();
        Ok(Some(match message {
            WebSocketMessage::Text(text) => {
                match serde_json::from_str(&text).map(Variant::from_json_value) {
                    Ok(value) => NetEvent::WebSocketVariant { peer, value },
                    Err(err) => NetEvent::WebSocketInvalidJson {
                        peer,
                        text,
                        message: err.to_string(),
                    },
                }
            }
            WebSocketMessage::Binary(bytes) => NetEvent::WebSocketBinary { peer, bytes },
            WebSocketMessage::Ping(bytes) => NetEvent::WebSocketPing { peer, bytes },
            WebSocketMessage::Pong(bytes) => NetEvent::WebSocketPong { peer, bytes },
            WebSocketMessage::Close { code, reason } => {
                NetEvent::WebSocketClosed { peer, code, reason }
            }
        }))
    }

    pub fn poll_variant_event_default(&mut self) -> NetResult<Option<NetEvent>> {
        self.poll_variant_event(self.max_message_bytes)
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

    pub fn send_compressed_text(&mut self, text: impl Into<String>) -> NetResult<()> {
        let mut bytes = WEBSOCKET_ZLIB_TEXT_PREFIX.to_vec();
        bytes.extend(compress_zlib_best(text.into().as_bytes()).map_err(|err| {
            NetError::new(
                NetErrorKind::InvalidFrame,
                format!("compress websocket text: {err}"),
            )
        })?);
        self.send_binary(bytes)
    }

    pub fn send_compressed_binary(&mut self, payload: Vec<u8>) -> NetResult<()> {
        let mut bytes = WEBSOCKET_ZLIB_BINARY_PREFIX.to_vec();
        bytes.extend(compress_zlib_best(&payload).map_err(|err| {
            NetError::new(
                NetErrorKind::InvalidFrame,
                format!("compress websocket binary: {err}"),
            )
        })?);
        self.send_binary(bytes)
    }

    pub fn send_variant(&mut self, value: &Variant) -> NetResult<()> {
        let text = serde_json::to_string(&value.to_json_value())
            .map_err(|err| NetError::new(NetErrorKind::InvalidFrame, err.to_string()))?;
        self.send_text(text)
    }

    pub fn send_ping(&mut self, bytes: Vec<u8>) -> NetResult<()> {
        self.socket
            .send(Message::Ping(bytes.into()))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }

    pub fn send_heartbeat_ping(&mut self) -> NetResult<()> {
        self.send_ping(WEBSOCKET_HEARTBEAT_BYTES.to_vec())
    }

    pub fn send_pong(&mut self, bytes: Vec<u8>) -> NetResult<()> {
        self.socket
            .send(Message::Pong(bytes.into()))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }

    pub fn close(&mut self) -> NetResult<()> {
        self.close_with_reason(1000, "")
    }

    pub fn close_with_reason(&mut self, code: u16, reason: impl Into<String>) -> NetResult<()> {
        let frame = CloseFrame {
            code: CloseCode::from(code),
            reason: Utf8Bytes::from(reason.into()),
        };
        self.socket
            .close(Some(frame))
            .map_err(|err| NetError::new(NetErrorKind::Send, err.to_string()))
    }
}

#[derive(Clone)]
pub struct WebSocketAsyncConnection {
    inner: Arc<Mutex<WebSocketConnection>>,
}

impl WebSocketAsyncConnection {
    pub async fn connect(url: impl Into<String>) -> NetResult<Self> {
        Self::connect_with_options(url, WebSocketConnectOptions::default()).await
    }

    pub async fn connect_with_options(
        url: impl Into<String>,
        options: WebSocketConnectOptions,
    ) -> NetResult<Self> {
        let url = url.into();
        let connection =
            spawn_net_blocking(move || WebSocketConnection::connect_with_options(url, options))
                .await?;
        Ok(Self::from_connection(connection))
    }

    pub fn from_connection(connection: WebSocketConnection) -> Self {
        Self {
            inner: Arc::new(Mutex::new(connection)),
        }
    }

    pub async fn poll_event(&self, max_bytes: usize) -> NetResult<Option<NetEvent>> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || lock_websocket(&inner)?.poll_event(max_bytes)).await
    }

    pub async fn poll_event_default(&self) -> NetResult<Option<NetEvent>> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || {
            let mut connection = lock_websocket(&inner)?;
            connection.poll_event_default()
        })
        .await
    }

    pub async fn poll_variant_event(&self, max_bytes: usize) -> NetResult<Option<NetEvent>> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || lock_websocket(&inner)?.poll_variant_event(max_bytes)).await
    }

    pub async fn send_text(&self, text: impl Into<String>) -> NetResult<()> {
        let inner = self.inner.clone();
        let text = text.into();
        spawn_net_blocking(move || lock_websocket(&inner)?.send_text(text)).await
    }

    pub async fn send_binary(&self, bytes: Vec<u8>) -> NetResult<()> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || lock_websocket(&inner)?.send_binary(bytes)).await
    }

    pub async fn send_compressed_text(&self, text: impl Into<String>) -> NetResult<()> {
        let inner = self.inner.clone();
        let text = text.into();
        spawn_net_blocking(move || lock_websocket(&inner)?.send_compressed_text(text)).await
    }

    pub async fn send_compressed_binary(&self, bytes: Vec<u8>) -> NetResult<()> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || lock_websocket(&inner)?.send_compressed_binary(bytes)).await
    }

    pub async fn close(&self) -> NetResult<()> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || lock_websocket(&inner)?.close()).await
    }
}

#[derive(Clone)]
pub struct WebSocketAsyncHost {
    inner: Arc<WebSocketHost>,
}

pub struct WebSocketHost {
    listener: TcpListener,
    local: SocketAddr,
    options: WebSocketHostOptions,
}

impl WebSocketHost {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> NetResult<Self> {
        Self::bind_with_options(addr, WebSocketHostOptions::default())
    }

    pub fn bind_with_options<A: ToSocketAddrs>(
        addr: A,
        options: WebSocketHostOptions,
    ) -> NetResult<Self> {
        let listener =
            TcpListener::bind(addr).map_err(|err| NetError::from_io(NetErrorKind::Bind, err))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| NetError::from_io(NetErrorKind::SetNonBlocking, err))?;
        let local = listener
            .local_addr()
            .map_err(|err| NetError::from_io(NetErrorKind::LocalAddress, err))?;
        Ok(Self {
            listener,
            local,
            options,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn accept(&self) -> NetResult<Option<WebSocketConnection>> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                WebSocketConnection::accept_with_options(stream, self.options.clone()).map(Some)
            }
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

impl WebSocketAsyncHost {
    pub async fn bind(addr: impl ToSocketAddrs + Send + 'static) -> NetResult<Self> {
        Self::bind_with_options(addr, WebSocketHostOptions::default()).await
    }

    pub async fn bind_with_options(
        addr: impl ToSocketAddrs + Send + 'static,
        options: WebSocketHostOptions,
    ) -> NetResult<Self> {
        let host =
            spawn_net_blocking(move || WebSocketHost::bind_with_options(addr, options)).await?;
        Ok(Self {
            inner: Arc::new(host),
        })
    }

    pub fn from_host(host: WebSocketHost) -> Self {
        Self {
            inner: Arc::new(host),
        }
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr()
    }

    pub async fn accept(&self) -> NetResult<Option<WebSocketAsyncConnection>> {
        let inner = self.inner.clone();
        spawn_net_blocking(move || {
            inner
                .accept()
                .map(|connection| connection.map(WebSocketAsyncConnection::from_connection))
        })
        .await
    }
}

fn websocket_config(max_message_bytes: usize) -> WebSocketConfig {
    WebSocketConfig::default().max_message_size(Some(max_message_bytes.max(1)))
}

fn build_websocket_request(
    url: &str,
    options: &WebSocketConnectOptions,
) -> NetResult<ClientRequestBuilder> {
    let uri: Uri = url.parse().map_err(|err| {
        NetError::new(
            NetErrorKind::Connect,
            format!("invalid websocket url: {err}"),
        )
    })?;
    let mut request = ClientRequestBuilder::new(uri);
    for (name, value) in &options.headers {
        request = request.with_header(name.clone(), value.clone());
    }
    for protocol in &options.subprotocols {
        request = request.with_sub_protocol(protocol.clone());
    }
    Ok(request)
}

#[allow(clippy::result_large_err)]
fn accept_websocket_with_options(
    stream: TcpStream,
    options: &WebSocketHostOptions,
    selected_subprotocol: &mut Option<String>,
) -> NetResult<WebSocket<TcpStream>> {
    let protocols = options.subprotocols.clone();
    let require_subprotocol = options.require_subprotocol;
    let socket = tungstenite::accept_hdr_with_config(
        stream,
        |request: &tungstenite::handshake::server::Request,
         mut response: tungstenite::handshake::server::Response| {
            let selected = select_websocket_subprotocol(
                request
                    .headers()
                    .get("Sec-WebSocket-Protocol")
                    .and_then(|v| v.to_str().ok()),
                &protocols,
            );
            if let Some(protocol) = selected {
                response
                    .headers_mut()
                    .insert("Sec-WebSocket-Protocol", protocol.parse().unwrap());
                *selected_subprotocol = Some(protocol);
            } else if require_subprotocol {
                *selected_subprotocol = None;
            }
            Ok(response)
        },
        Some(websocket_config(options.max_message_bytes)),
    )
    .map_err(|err| NetError::new(NetErrorKind::Handshake, err.to_string()))?;
    if require_subprotocol && selected_subprotocol.is_none() {
        return Err(NetError::new(
            NetErrorKind::Handshake,
            "websocket subprotocol required",
        ));
    }
    Ok(socket)
}

fn select_websocket_subprotocol(header: Option<&str>, supported: &[String]) -> Option<String> {
    let header = header?;
    header
        .split(',')
        .map(str::trim)
        .find(|requested| supported.iter().any(|protocol| protocol == *requested))
        .map(str::to_string)
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
        Message::Binary(bytes) => map_websocket_binary(bytes.as_ref())?,
        Message::Ping(bytes) => WebSocketMessage::Ping(bytes.to_vec()),
        Message::Pong(bytes) => WebSocketMessage::Pong(bytes.to_vec()),
        Message::Close(frame) => WebSocketMessage::Close {
            code: frame.as_ref().map(|frame| u16::from(frame.code)),
            reason: frame
                .as_ref()
                .map(|frame| frame.reason.to_string())
                .unwrap_or_default(),
        },
        Message::Frame(_) => {
            return Err(NetError::new(
                NetErrorKind::InvalidFrame,
                "raw websocket frame",
            ));
        }
    })
}

fn map_websocket_binary(bytes: &[u8]) -> NetResult<WebSocketMessage> {
    if let Some(payload) = bytes.strip_prefix(WEBSOCKET_ZLIB_TEXT_PREFIX) {
        return Ok(WebSocketMessage::Text(utf8(
            decompress_zlib(payload).map_err(|err| {
                NetError::new(
                    NetErrorKind::InvalidFrame,
                    format!("decompress websocket text: {err}"),
                )
            })?,
        )?));
    }
    if let Some(payload) = bytes.strip_prefix(WEBSOCKET_ZLIB_BINARY_PREFIX) {
        return Ok(WebSocketMessage::Binary(decompress_zlib(payload).map_err(
            |err| {
                NetError::new(
                    NetErrorKind::InvalidFrame,
                    format!("decompress websocket binary: {err}"),
                )
            },
        )?));
    }
    Ok(WebSocketMessage::Binary(bytes.to_vec()))
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

async fn spawn_net_blocking<T>(f: impl FnOnce() -> NetResult<T> + Send + 'static) -> NetResult<T>
where
    T: Send + 'static,
{
    task::spawn_blocking(f)
        .await
        .map_err(|err| NetError::new(NetErrorKind::WebSocket, err.to_string()))?
}

fn lock_websocket(
    inner: &Arc<Mutex<WebSocketConnection>>,
) -> NetResult<std::sync::MutexGuard<'_, WebSocketConnection>> {
    inner
        .lock()
        .map_err(|_| NetError::new(NetErrorKind::WebSocket, "websocket lock poisoned"))
}
