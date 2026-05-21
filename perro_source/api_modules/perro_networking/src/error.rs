use super::*;

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

    pub(crate) fn from_io(kind: NetErrorKind, err: io::Error) -> Self {
        Self::new(kind, err.to_string())
    }
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for NetError {}
