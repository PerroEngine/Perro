use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UdpPacket {
    pub peer: String,
    pub bytes: Vec<u8>,
}

pub struct UdpEndpoint {
    socket: UdpSocket,
    local: SocketAddr,
    // Keep Send + Sync and preserve concurrent shared-reference receives.
    // Idle polls reuse storage; successful packets take ownership of it.
    recv_buf: Mutex<Vec<u8>>,
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
        Ok(Self {
            socket,
            local,
            recv_buf: Mutex::new(Vec::new()),
        })
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
        let mut buf = self
            .recv_buf
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        buf.resize(max_bytes.max(1), 0);
        match self.socket.recv_from(&mut buf) {
            Ok((n, peer)) => {
                buf.truncate(n);
                Ok(Some(UdpPacket {
                    peer: peer.to_string(),
                    bytes: std::mem::take(&mut *buf),
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

pub const WEBSOCKET_VARIANT_PROTOCOL: &str = "perro.variant.v1";
pub const WEBSOCKET_HEARTBEAT_BYTES: &[u8] = b"PERRO_WS_HEARTBEAT";
pub const WEBSOCKET_ZLIB_TEXT_PREFIX: &[u8] = b"PERRO_WS_ZLIB_TEXT\0";
pub const WEBSOCKET_ZLIB_BINARY_PREFIX: &[u8] = b"PERRO_WS_ZLIB_BINARY\0";
