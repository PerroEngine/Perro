use crate::{
    app,
    error::SteamError,
    types::{AppID, DLCID, SteamID},
};

pub fn is_installed(app_id: AppID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_app_installed(app_id.into())))
}

pub fn is_dlc_installed(app_id: AppID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_dlc_installed(app_id.into())))
}

pub fn is_dlc_id_installed(dlc_id: DLCID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_dlc_installed(dlc_id.into())))
}

pub fn is_subscribed() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_subscribed()))
}

pub fn is_subscribed_app(app_id: AppID) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_subscribed_app(app_id.into())))
}

pub fn is_subscribed_from_free_weekend() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_subscribed_from_free_weekend()))
}

pub fn is_vac_banned() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_vac_banned()))
}

pub fn is_cybercafe() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_cybercafe()))
}

pub fn is_low_violence() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.apps().is_low_violence()))
}

pub fn get_build_id() -> Result<i32, SteamError> {
    app::with_client(|client| Ok(client.apps().app_build_id()))
}

pub fn get_install_dir(app_id: AppID) -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.apps().app_install_dir(app_id.into())))
}

pub fn get_owner() -> Result<SteamID, SteamError> {
    app::with_client(|client| Ok(client.apps().app_owner().into()))
}

pub fn get_available_languages() -> Result<Vec<String>, SteamError> {
    app::with_client(|client| Ok(client.apps().available_game_languages()))
}

pub fn get_current_language() -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.apps().current_game_language()))
}

pub fn get_current_beta_name() -> Result<Option<String>, SteamError> {
    app::with_client(|client| Ok(client.apps().current_beta_name()))
}

pub fn get_launch_command_line() -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.apps().launch_command_line()))
}

pub fn get_launch_query_param(key: &str) -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.apps().launch_query_param(key)))
}
