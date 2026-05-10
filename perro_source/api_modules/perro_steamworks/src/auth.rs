use crate::{
    app,
    error::SteamError,
    types::{AppID, SteamID},
};

pub type AuthTicket = steamworks::AuthTicket;
pub type AuthSessionError = steamworks::AuthSessionError;
pub type UserHasLicense = steamworks::UserHasLicense;

pub fn authentication_session_ticket() -> Result<(AuthTicket, Vec<u8>), SteamError> {
    app::with_client(|client| {
        let user = client.user();
        Ok(user.authentication_session_ticket_with_steam_id(user.steam_id()))
    })
}

pub fn authentication_session_ticket_with_steam_id(
    remote: SteamID,
) -> Result<(AuthTicket, Vec<u8>), SteamError> {
    app::with_client(|client| {
        Ok(client
            .user()
            .authentication_session_ticket_with_steam_id(remote.into()))
    })
}

pub fn cancel_authentication_ticket(ticket: AuthTicket) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user().cancel_authentication_ticket(ticket);
        Ok(())
    })
}

pub fn begin_authentication_session(
    user: SteamID,
    ticket: &[u8],
) -> Result<Result<(), AuthSessionError>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .user()
            .begin_authentication_session(user.into(), ticket))
    })
}

pub fn end_authentication_session(user: SteamID) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user().end_authentication_session(user.into());
        Ok(())
    })
}

pub fn authentication_session_ticket_for_webapi(identity: &str) -> Result<AuthTicket, SteamError> {
    app::with_client(|client| {
        Ok(client
            .user()
            .authentication_session_ticket_for_webapi(identity))
    })
}

pub fn user_has_license_for_app(
    user: SteamID,
    app_id: AppID,
) -> Result<UserHasLicense, SteamError> {
    app::with_client(|client| {
        Ok(client
            .user()
            .user_has_license_for_app(user.into(), app_id.into()))
    })
}
