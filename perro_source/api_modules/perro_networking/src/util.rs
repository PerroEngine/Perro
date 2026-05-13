use super::*;

pub(crate) fn first_addr<A: ToSocketAddrs>(addr: A) -> NetResult<SocketAddr> {
    addr.to_socket_addrs()
        .map_err(|err| NetError::from_io(NetErrorKind::AddressResolve, err))?
        .next()
        .ok_or_else(|| NetError::new(NetErrorKind::AddressResolve, "no socket address resolved"))
}

pub(crate) fn utf8(bytes: Vec<u8>) -> NetResult<String> {
    String::from_utf8(bytes).map_err(|err: FromUtf8Error| {
        NetError::new(NetErrorKind::Handshake, format!("invalid utf8: {err}"))
    })
}
