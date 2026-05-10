use crate::{app, error::SteamError, types::AppID};

pub type NotificationPosition = steamworks::NotificationPosition;
pub type GamepadTextInputMode = steamworks::GamepadTextInputMode;
pub type GamepadTextInputLineMode = steamworks::GamepadTextInputLineMode;

pub fn get_app_id() -> Result<AppID, SteamError> {
    app::with_client(|client| Ok(client.utils().app_id().into()))
}

pub fn get_ip_country() -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.utils().ip_country()))
}

pub fn is_overlay_enabled() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.utils().is_overlay_enabled()))
}

pub fn get_ui_language() -> Result<String, SteamError> {
    app::with_client(|client| Ok(client.utils().ui_language()))
}

pub fn get_server_real_time() -> Result<u32, SteamError> {
    app::with_client(|client| Ok(client.utils().get_server_real_time()))
}

pub fn set_overlay_notification_position(position: NotificationPosition) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.utils().set_overlay_notification_position(position);
        Ok(())
    })
}

pub fn is_steam_deck() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.utils().is_steam_running_on_steam_deck()))
}

pub fn is_big_picture() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.utils().is_steam_in_big_picture_mode()))
}
