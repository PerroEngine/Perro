use super::*;
use std::collections::VecDeque;

const MIN_FRAME_QUEUE_BYTES: usize = 1024 * 1024;
const MAX_QUEUED_FRAMES: usize = 16;
/// Max bytes accepted by buffered TCP writes while the socket cannot make progress.
pub const MAX_TCP_PENDING_WRITE_BYTES: usize = 16 * 1024 * 1024;

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
    frame_buf: VecDeque<u8>,
    tx_queue: VecDeque<Vec<u8>>,
    tx_cursor: usize,
    read_eof: bool,
    disconnect_emitted: bool,
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
            frame_buf: VecDeque::new(),
            tx_queue: VecDeque::new(),
            tx_cursor: 0,
            read_eof: false,
            disconnect_emitted: false,
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
        self.flush_pending()?;
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
        if !self.tx_queue.is_empty() {
            self.enqueue_write(bytes.to_vec())?;
            self.flush_pending()?;
            return Ok(bytes.len());
        }
        self.stream
            .write(bytes)
            .map_err(|err| NetError::from_io(NetErrorKind::Send, err))
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> NetResult<()> {
        if !bytes.is_empty() {
            self.enqueue_write(bytes.to_vec())?;
        }
        self.flush_pending().map(|_| ())
    }

    pub fn write_frame(&mut self, bytes: &[u8]) -> NetResult<()> {
        let frame = encode_frame(bytes)?;
        self.enqueue_write(frame)?;
        self.flush_pending().map(|_| ())
    }

    pub fn write_handshake(&mut self, handshake: &NetHandshake) -> NetResult<()> {
        self.write_frame(&handshake.encode()?)
    }

    pub fn poll_frame(&mut self, max_frame_bytes: usize) -> NetResult<Option<Vec<u8>>> {
        self.flush_pending()?;
        if let Some(frame) = decode_next_queued_frame(&mut self.frame_buf, max_frame_bytes)? {
            return Ok(Some(frame));
        }
        self.read_into_frame_buf(max_frame_bytes)?;
        decode_next_queued_frame(&mut self.frame_buf, max_frame_bytes)
    }

    pub fn poll_frame_event(&mut self, max_frame_bytes: usize) -> NetResult<Option<NetEvent>> {
        let peer = self.peer_string();
        let Some(bytes) = self.poll_frame(max_frame_bytes)? else {
            if self.read_eof && !self.disconnect_emitted {
                self.frame_buf.clear();
                self.disconnect_emitted = true;
                return Ok(Some(NetEvent::TcpDisconnected { peer }));
            }
            return Ok(None);
        };
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
        if self.read_eof {
            return Ok(());
        }
        validate_queued_frame_len(&self.frame_buf, max_frame_bytes)?;
        let queue_limit = frame_queue_limit(max_frame_bytes);
        let mut tmp = [0_u8; 4096];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    self.read_eof = true;
                    return Ok(());
                }
                Ok(n) => {
                    self.frame_buf.extend(&tmp[..n]);
                    validate_queued_frame_len(&self.frame_buf, max_frame_bytes)?;
                    if self.frame_buf.len() > queue_limit {
                        return Err(NetError::new(
                            NetErrorKind::FrameTooLarge,
                            "tcp frame queue exceeds max",
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

    /// Try to write all queued bytes without blocking.
    ///
    /// Returns `true` when the queue is empty.
    pub fn flush_pending(&mut self) -> NetResult<bool> {
        while let Some(bytes) = self.tx_queue.front() {
            match self.stream.write(&bytes[self.tx_cursor..]) {
                Ok(0) => {
                    return Err(NetError::new(
                        NetErrorKind::Send,
                        "tcp stream wrote zero bytes",
                    ));
                }
                Ok(n) => {
                    self.tx_cursor += n;
                    if self.tx_cursor == bytes.len() {
                        self.tx_queue.pop_front();
                        self.tx_cursor = 0;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(err) => return Err(NetError::from_io(NetErrorKind::Send, err)),
            }
        }
        Ok(true)
    }

    /// Bytes accepted by buffered writes but not yet sent to the socket.
    pub fn pending_write_bytes(&self) -> usize {
        self.tx_queue
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_sub(self.tx_cursor)
    }

    fn enqueue_write(&mut self, bytes: Vec<u8>) -> NetResult<()> {
        if self.pending_write_bytes().saturating_add(bytes.len()) > MAX_TCP_PENDING_WRITE_BYTES {
            return Err(NetError::new(
                NetErrorKind::Send,
                "tcp pending write queue exceeds max",
            ));
        }
        self.tx_queue.push_back(bytes);
        Ok(())
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

fn decode_next_queued_frame(
    buffer: &mut VecDeque<u8>,
    max_frame_bytes: usize,
) -> NetResult<Option<Vec<u8>>> {
    validate_queued_frame_len(buffer, max_frame_bytes)?;
    if buffer.len() < 4 {
        return Ok(None);
    }
    let len = queued_frame_len(buffer);
    let frame_end = 4_usize.saturating_add(len);
    if buffer.len() < frame_end {
        return Ok(None);
    }

    buffer.drain(..4);
    Ok(Some(buffer.drain(..len).collect()))
}

fn validate_queued_frame_len(buffer: &VecDeque<u8>, max_frame_bytes: usize) -> NetResult<()> {
    if buffer.len() >= 4 && queued_frame_len(buffer) > max_frame_bytes {
        return Err(NetError::new(
            NetErrorKind::FrameTooLarge,
            "tcp frame exceeds max",
        ));
    }
    Ok(())
}

fn queued_frame_len(buffer: &VecDeque<u8>) -> usize {
    u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize
}

fn frame_queue_limit(max_frame_bytes: usize) -> usize {
    max_frame_bytes
        .saturating_add(4)
        .saturating_mul(MAX_QUEUED_FRAMES)
        .max(MIN_FRAME_QUEUE_BYTES)
}
