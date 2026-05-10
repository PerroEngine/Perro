use crate::{app, error::SteamError};

pub type RemotePlaySessionID = steamworks::RemotePlaySessionId;
pub type RemotePlaySession = steamworks::RemotePlaySession;
pub type SteamDeviceFormFactor = steamworks::SteamDeviceFormFactor;

pub fn get_sessions() -> Result<Vec<RemotePlaySession>, SteamError> {
    app::with_client(|client| Ok(client.remote_play().sessions()))
}

pub fn get_session(session: RemotePlaySessionID) -> Result<RemotePlaySession, SteamError> {
    app::with_client(|client| Ok(client.remote_play().session(session)))
}
