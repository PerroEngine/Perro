pub mod account;
pub mod achievements;
pub mod app;
pub mod apps;
pub mod auth;
pub mod cloud;
pub mod error;
pub mod events;
pub mod friends;
pub mod input;
pub mod leaderboards;
pub mod lobbies;
pub mod networking;
pub mod networking_messages;
pub mod networking_sockets;
pub mod networking_utils;
pub mod remote_play;
pub mod screenshots;
pub mod servers;
pub mod stats;
pub mod timeline;
pub mod types;
pub mod utils;
pub mod workshop;

pub use error::SteamError;
pub use types::{
    AppID, ConnectionID, DLCID, FriendGame, FriendInfo, FriendListKind, FriendState, LeaderboardID,
    LobbyDataKey, LobbyDistance, LobbyID, LobbyInfo, LobbyJoinability, LobbyNearValueFilter,
    LobbyNumberComparison, LobbyNumberFilter, LobbySearch, LobbyStringFilter,
    LobbyStringFilterKind, LobbyType, OverlayDialog, RichPresenceKey, SocketID, SteamEvent,
    SteamID, StoreOverlayAction, UserOverlayDialog, WorkshopFileID,
};

#[macro_export]
macro_rules! steam_unlock {
    ($id:expr) => {
        $crate::achievements::unlock($id)
    };
}

#[macro_export]
macro_rules! steam_ach_unlock {
    ($first:expr, $($id:expr),+ $(,)?) => {
        $crate::achievements::unlock_many([$first, $($id),+])
    };
    ($id:expr) => {
        $crate::achievements::unlock_input($id)
    };
}

#[macro_export]
macro_rules! steam_clear {
    ($id:expr) => {
        $crate::achievements::clear($id)
    };
}

#[macro_export]
macro_rules! steam_ach_clear {
    ($id:expr) => {
        $crate::achievements::clear($id)
    };
}

#[macro_export]
macro_rules! steam_friend_list {
    () => {
        $crate::friends::get_list()
    };
}

#[macro_export]
macro_rules! steam_rich_presence_set {
    ($key:expr, $value:expr) => {
        $crate::friends::set_rich_presence($key, $value)
    };
}

#[macro_export]
macro_rules! steam_lobby_create {
    ($kind:expr, $max_members:expr) => {
        $crate::lobbies::create($kind, $max_members)
    };
}

#[macro_export]
macro_rules! steam_lobby_join {
    ($id:expr) => {
        $crate::lobbies::join($id)
    };
}

#[macro_export]
macro_rules! steam_lobby_leave {
    ($id:expr) => {
        $crate::lobbies::leave($id)
    };
}

#[macro_export]
macro_rules! steam_lobby_data_set {
    ($id:expr, $key:expr, $value:expr) => {
        $crate::lobbies::set_data($id, $key, $value)
    };
}

#[macro_export]
macro_rules! steam_lobby_chat {
    ($id:expr, $message:expr) => {
        $crate::lobbies::send_chat($id, $message)
    };
}

#[macro_export]
macro_rules! steam_events {
    () => {
        $crate::events::drain()
    };
}

#[macro_export]
macro_rules! steam_account_name {
    ($id:expr) => {
        $crate::account::get_name($id)
    };
}

#[macro_export]
macro_rules! steam_account_self_name {
    () => {
        $crate::account::get_self_name()
    };
}

#[macro_export]
macro_rules! steam_account_self_id {
    () => {
        $crate::account::get_self_id()
    };
}

#[macro_export]
macro_rules! steam_app_dlc_installed {
    ($id:expr) => {
        $crate::apps::is_dlc_id_installed($id)
    };
}

#[macro_export]
macro_rules! steam_app_subscribed {
    () => {
        $crate::apps::is_subscribed()
    };
    ($id:expr) => {
        $crate::apps::is_subscribed_app($id)
    };
}

#[macro_export]
macro_rules! steam_stat_get_i32 {
    ($name:expr) => {
        $crate::stats::get_i32($name)
    };
}

#[macro_export]
macro_rules! steam_stat_set_i32 {
    ($name:expr, $value:expr) => {
        $crate::stats::set_i32($name, $value)
    };
}

#[macro_export]
macro_rules! steam_stat_store {
    () => {
        $crate::stats::store()
    };
}

#[macro_export]
macro_rules! steam_leaderboard_upload {
    ($leaderboard:expr, $method:expr, $score:expr, $details:expr, $cb:expr) => {
        $crate::leaderboards::upload($leaderboard, $method, $score, $details, $cb)
    };
}

#[macro_export]
macro_rules! steam_leaderboard_entries {
    ($leaderboard:expr, $request:expr, $start:expr, $end:expr, $details_len:expr, $cb:expr) => {
        $crate::leaderboards::entries($leaderboard, $request, $start, $end, $details_len, $cb)
    };
}

#[macro_export]
macro_rules! steam_cloud_read {
    ($name:expr) => {
        $crate::cloud::get_file_bytes($name)
    };
}

#[macro_export]
macro_rules! steam_cloud_write {
    ($name:expr, $bytes:expr) => {
        $crate::cloud::write($name, $bytes)
    };
}

#[macro_export]
macro_rules! steam_workshop_subscribe {
    ($file:expr, $cb:expr) => {
        $crate::workshop::subscribe($file, $cb)
    };
}

#[macro_export]
macro_rules! steam_workshop_download {
    ($file:expr, $high_priority:expr) => {
        $crate::workshop::is_download_started($file, $high_priority)
    };
}

#[macro_export]
macro_rules! steam_p2p_send {
    ($user:expr, $send_type:expr, $data:expr) => {
        $crate::networking::is_p2p_sent($user, $send_type, $data)
    };
    ($user:expr, $send_type:expr, $data:expr, $channel:expr) => {
        $crate::networking::is_p2p_sent_on_channel($user, $send_type, $data, $channel)
    };
}

#[macro_export]
macro_rules! steam_p2p_read {
    ($max_size:expr) => {
        $crate::networking::get_p2p_packet($max_size)
    };
    ($max_size:expr, $channel:expr) => {
        $crate::networking::get_p2p_packet_from_channel($max_size, $channel)
    };
}

pub mod prelude {
    pub use crate::account;
    pub use crate::achievements;
    pub use crate::app;
    pub use crate::apps;
    pub use crate::auth;
    pub use crate::cloud;
    pub use crate::events;
    pub use crate::friends;
    pub use crate::input;
    pub use crate::leaderboards;
    pub use crate::lobbies;
    pub use crate::networking;
    pub use crate::networking_messages;
    pub use crate::networking_sockets;
    pub use crate::networking_utils;
    pub use crate::remote_play;
    pub use crate::screenshots;
    pub use crate::servers;
    pub use crate::stats;
    pub use crate::timeline;
    pub use crate::utils;
    pub use crate::workshop;
    pub use crate::{
        AppID, ConnectionID, DLCID, FriendGame, FriendInfo, FriendListKind, FriendState,
        LeaderboardID, LobbyDataKey, LobbyDistance, LobbyID, LobbyInfo, LobbyJoinability,
        LobbyNearValueFilter, LobbyNumberComparison, LobbyNumberFilter, LobbySearch,
        LobbyStringFilter, LobbyStringFilterKind, LobbyType, OverlayDialog, RichPresenceKey,
        SocketID, SteamError, SteamEvent, SteamID, StoreOverlayAction, UserOverlayDialog,
        WorkshopFileID,
    };
    pub use crate::{
        steam_account_name, steam_account_self_id, steam_account_self_name, steam_ach_clear,
        steam_ach_unlock, steam_app_dlc_installed, steam_app_subscribed, steam_clear,
        steam_cloud_read, steam_cloud_write, steam_events, steam_friend_list,
        steam_leaderboard_entries, steam_leaderboard_upload, steam_lobby_chat, steam_lobby_create,
        steam_lobby_data_set, steam_lobby_join, steam_lobby_leave, steam_p2p_read, steam_p2p_send,
        steam_rich_presence_set, steam_stat_get_i32, steam_stat_set_i32, steam_stat_store,
        steam_unlock, steam_workshop_download, steam_workshop_subscribe,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn disabled_init_noop() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, None).expect("disabled init");
        app::run_callbacks().expect("callbacks");
        assert_eq!(app::is_enabled(), Ok(false));
        assert_eq!(app::is_ready(), Ok(false));
        assert_eq!(app::get_app_id(), Ok(None));
    }

    #[test]
    fn callbacks_before_init_noop() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::run_callbacks().expect("callbacks");
    }

    #[test]
    fn achievements_return_disabled_without_steam() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, None).expect("disabled init");
        assert_eq!(achievements::unlock("ACH_TEST"), Err(SteamError::Disabled));
        assert_eq!(achievements::clear("ACH_TEST"), Err(SteamError::Disabled));
    }

    #[test]
    fn enabled_init_requires_app_id() {
        let _guard = test_lock();
        app::reset_for_tests();
        assert_eq!(
            app::init_from_config(true, None),
            Err(SteamError::MissingAppId)
        );
        assert_eq!(app::is_enabled(), Ok(false));
        assert_eq!(app::is_ready(), Ok(false));
    }

    #[test]
    fn disabled_init_with_app_id_stays_disabled() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, Some(480)).expect("disabled init");
        assert_eq!(app::is_enabled(), Ok(false));
        assert_eq!(app::is_ready(), Ok(false));
        assert_eq!(app::get_app_id(), Ok(None));
    }

    #[test]
    fn macros_route_to_disabled_errors() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, None).expect("disabled init");
        assert_eq!(steam_unlock!("ACH_TEST"), Err(SteamError::Disabled));
        assert_eq!(steam_clear!("ACH_TEST"), Err(SteamError::Disabled));
        assert_eq!(steam_ach_unlock!("ACH_TEST"), Err(SteamError::Disabled));
        assert_eq!(steam_ach_clear!("ACH_TEST"), Err(SteamError::Disabled));
        assert_eq!(steam_account_self_name!(), Err(SteamError::Disabled));
        assert_eq!(steam_account_self_id!(), Err(SteamError::Disabled));
        assert_eq!(
            steam_account_name!(SteamID::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(steam_friend_list!(), Err(SteamError::Disabled));
        assert_eq!(
            steam_rich_presence_set!(RichPresenceKey::Status, "menu"),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_create!(LobbyType::FriendsOnly, 4),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_join!(LobbyID::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_leave!(LobbyID::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_data_set!(LobbyID::from_id(1), "mode", "coop"),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_chat!(LobbyID::from_id(1), "hi"),
            Err(SteamError::Disabled)
        );
        assert_eq!(steam_events!(), Ok(Vec::new()));
    }

    #[test]
    fn achievement_macros_accept_expressions() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, None).expect("disabled init");
        let id = "ACH_TEST";
        assert_eq!(steam_ach_unlock!(id), Err(SteamError::Disabled));
        assert_eq!(
            steam_ach_clear!(format!("ACH_{suffix}", suffix = "TEST").as_str()),
            Err(SteamError::Disabled)
        );
        assert_eq!(steam_ach_unlock!(&[id]), Err(SteamError::Disabled));
        assert_eq!(
            steam_ach_unlock!("ACH_TEST", "ACH_OTHER"),
            Err(SteamError::Disabled)
        );
    }

    #[test]
    fn live_steam_480_init_is_idempotent_when_enabled() {
        if std::env::var_os("PERRO_STEAMWORKS_LIVE_TESTS").is_none() {
            return;
        }
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(true, Some(480)).expect("Steam AppId 480 init");
        assert_eq!(app::is_enabled(), Ok(true));
        assert_eq!(app::is_ready(), Ok(true));
        assert_eq!(app::get_app_id(), Ok(Some(480)));
        app::init_from_config(true, Some(480)).expect("same AppId re-init");
        app::run_callbacks().expect("callbacks");
    }
}
