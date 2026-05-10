use crate::{app, error::SteamError, types::SteamID};
use std::net::Ipv4Addr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendType {
    Unreliable,
    UnreliableNoDelay,
    Reliable,
    ReliableWithBuffering,
}

impl From<SendType> for steamworks::SendType {
    fn from(send_type: SendType) -> Self {
        match send_type {
            SendType::Unreliable => Self::Unreliable,
            SendType::UnreliableNoDelay => Self::UnreliableNoDelay,
            SendType::Reliable => Self::Reliable,
            SendType::ReliableWithBuffering => Self::ReliableWithBuffering,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum P2PSessionError {
    None,
    NoRightsToApp,
    Timeout,
    Unknown(u8),
}

impl From<steamworks::P2PSessionError> for P2PSessionError {
    fn from(error: steamworks::P2PSessionError) -> Self {
        #[allow(deprecated)]
        match error {
            steamworks::P2PSessionError::None => Self::None,
            steamworks::P2PSessionError::NoRightsToApp => Self::NoRightsToApp,
            steamworks::P2PSessionError::Timeout => Self::Timeout,
            steamworks::P2PSessionError::Unknown(code) => Self::Unknown(code),
            steamworks::P2PSessionError::NotRunningApp
            | steamworks::P2PSessionError::NotLoggedIn => Self::Unknown(error.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2PSessionState {
    pub connection_active: bool,
    pub connecting: bool,
    pub error: P2PSessionError,
    pub using_relay: bool,
    pub bytes_queued_for_send: i32,
    pub packets_queued_for_send: i32,
    pub remote_ip: Option<Ipv4Addr>,
    pub remote_port: Option<u16>,
}

impl From<steamworks::P2PSessionState> for P2PSessionState {
    fn from(state: steamworks::P2PSessionState) -> Self {
        Self {
            connection_active: state.connection_active,
            connecting: state.connecting,
            error: state.error.into(),
            using_relay: state.using_relay,
            bytes_queued_for_send: state.bytes_queued_for_send,
            packets_queued_for_send: state.packets_queued_for_send,
            remote_ip: state.remote_ip,
            remote_port: state.remote_port,
        }
    }
}

pub fn is_p2p_session_accepted(user: SteamID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.networking().accept_p2p_session(user.into())))
}

pub fn is_p2p_session_closed(user: SteamID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.networking().close_p2p_session(user.into())))
}

pub fn get_session_state(user: SteamID) -> Result<Option<P2PSessionState>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .networking()
            .get_p2p_session_state(user.into())
            .map(Into::into))
    })
}

pub fn is_p2p_sent(user: SteamID, send_type: SendType, data: &[u8]) -> Result<bool, SteamError> {
    is_p2p_sent_on_channel(user, send_type, data, 0)
}

pub fn is_p2p_sent_on_channel(
    user: SteamID,
    send_type: SendType,
    data: &[u8],
    channel: i32,
) -> Result<bool, SteamError> {
    app::with_client(|client| {
        Ok(client.networking().send_p2p_packet_on_channel(
            user.into(),
            send_type.into(),
            data,
            channel,
        ))
    })
}

pub fn get_p2p_available() -> Result<Option<usize>, SteamError> {
    get_p2p_available_on_channel(0)
}

pub fn get_p2p_available_on_channel(channel: i32) -> Result<Option<usize>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .networking()
            .is_p2p_packet_available_on_channel(channel))
    })
}

pub fn get_p2p_packet(max_size: usize) -> Result<Option<(SteamID, Vec<u8>)>, SteamError> {
    get_p2p_packet_from_channel(max_size, 0)
}

pub fn get_p2p_packet_from_channel(
    max_size: usize,
    channel: i32,
) -> Result<Option<(SteamID, Vec<u8>)>, SteamError> {
    app::with_client(|client| {
        let mut buf = vec![0; max_size];
        Ok(client
            .networking()
            .read_p2p_packet_from_channel(&mut buf, channel)
            .map(|(id, size)| {
                buf.truncate(size);
                (id.into(), buf)
            }))
    })
}
