use crate::{app, error::SteamError, types::SteamID};

pub type SendFlags = steamworks::networking_types::SendFlags;
pub type NetworkingMessage = steamworks::networking_types::NetworkingMessage;
pub type NetworkingIdentity = steamworks::networking_types::NetworkingIdentity;
pub type NetworkingConnectionState = steamworks::networking_types::NetworkingConnectionState;
pub type NetConnectionInfo = steamworks::networking_types::NetConnectionInfo;
pub type NetConnectionRealTimeInfo = steamworks::networking_types::NetConnectionRealTimeInfo;

pub fn identity_steam_id(id: SteamID) -> NetworkingIdentity {
    NetworkingIdentity::new_steam_id(id.into())
}

pub fn send_to_user(
    user: SteamID,
    send_flags: SendFlags,
    data: &[u8],
    channel: u32,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .networking_messages()
            .send_message_to_user(identity_steam_id(user), send_flags, data, channel)
            .map_err(|_| SteamError::CallFailed("networking_messages.send_to_user"))
    })
}

pub fn get_received(channel: u32, batch_size: usize) -> Result<Vec<NetworkingMessage>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .networking_messages()
            .receive_messages_on_channel(channel, batch_size))
    })
}

pub fn get_session_info(
    user: SteamID,
) -> Result<
    (
        NetworkingConnectionState,
        Option<NetConnectionInfo>,
        Option<NetConnectionRealTimeInfo>,
    ),
    SteamError,
> {
    app::with_client(|client| {
        Ok(client
            .networking_messages()
            .get_session_connection_info(&identity_steam_id(user)))
    })
}
