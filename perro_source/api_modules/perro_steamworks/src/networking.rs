use crate::{app, error::SteamError, types::SteamID};

pub type SendType = steamworks::SendType;
pub type P2PSessionState = steamworks::P2PSessionState;

pub fn is_p2p_session_accepted(user: SteamID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.networking().accept_p2p_session(user.into())))
}

pub fn is_p2p_session_closed(user: SteamID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.networking().close_p2p_session(user.into())))
}

pub fn get_session_state(user: SteamID) -> Result<Option<P2PSessionState>, SteamError> {
    app::with_client(|client| Ok(client.networking().get_p2p_session_state(user.into())))
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
        Ok(client
            .networking()
            .send_p2p_packet_on_channel(user.into(), send_type, data, channel))
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
