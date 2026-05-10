use crate::types::SteamID;
use crate::{app, error::SteamError};

pub fn get_self_id() -> Result<SteamID, SteamError> {
    app::with_client(|client| Ok(client.user().steam_id().into()))
}

pub fn get_name(id: SteamID) -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.friends().get_friend(id.into()).name()))
}

pub fn get_self_name() -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.friends().name()))
}

pub fn get_level() -> Result<u32, SteamError> {
    app::with_client(|client| Ok(client.user().level()))
}

pub fn is_logged_on() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.user().logged_on()))
}
