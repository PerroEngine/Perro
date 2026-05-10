use crate::{
    app,
    error::SteamError,
    types::{AppID, SteamID},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthTicket {
    raw: steamworks::AuthTicket,
}

impl AuthTicket {
    fn new(raw: steamworks::AuthTicket) -> Self {
        Self { raw }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthSessionError {
    InvalidTicket,
    DuplicateRequest,
    InvalidVersion,
    GameMismatch,
    ExpiredTicket,
}

impl From<steamworks::AuthSessionError> for AuthSessionError {
    fn from(err: steamworks::AuthSessionError) -> Self {
        match err {
            steamworks::AuthSessionError::InvalidTicket => Self::InvalidTicket,
            steamworks::AuthSessionError::DuplicateRequest => Self::DuplicateRequest,
            steamworks::AuthSessionError::InvalidVersion => Self::InvalidVersion,
            steamworks::AuthSessionError::GameMismatch => Self::GameMismatch,
            steamworks::AuthSessionError::ExpiredTicket => Self::ExpiredTicket,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserHasLicense {
    HasLicense,
    DoesNotHaveLicense,
    NoAuth,
}

impl From<steamworks::UserHasLicense> for UserHasLicense {
    fn from(val: steamworks::UserHasLicense) -> Self {
        match val {
            steamworks::UserHasLicense::HasLicense => Self::HasLicense,
            steamworks::UserHasLicense::DoesNotHaveLicense => Self::DoesNotHaveLicense,
            steamworks::UserHasLicense::NoAuth => Self::NoAuth,
        }
    }
}

pub fn authentication_session_ticket() -> Result<(AuthTicket, Vec<u8>), SteamError> {
    app::with_client(|client| {
        let user = client.user();
        let (ticket, bytes) = user.authentication_session_ticket_with_steam_id(user.steam_id());
        Ok((AuthTicket::new(ticket), bytes))
    })
}

pub fn authentication_session_ticket_with_steam_id(
    remote: SteamID,
) -> Result<(AuthTicket, Vec<u8>), SteamError> {
    app::with_client(|client| {
        let (ticket, bytes) = client
            .user()
            .authentication_session_ticket_with_steam_id(remote.into());
        Ok((AuthTicket::new(ticket), bytes))
    })
}

pub fn cancel_authentication_ticket(ticket: AuthTicket) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user().cancel_authentication_ticket(ticket.raw);
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
            .begin_authentication_session(user.into(), ticket)
            .map_err(Into::into))
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
        Ok(AuthTicket::new(
            client
                .user()
                .authentication_session_ticket_for_webapi(identity),
        ))
    })
}

pub fn user_has_license_for_app(
    user: SteamID,
    app_id: AppID,
) -> Result<UserHasLicense, SteamError> {
    app::with_client(|client| {
        Ok(client
            .user()
            .user_has_license_for_app(user.into(), app_id.into())
            .into())
    })
}
