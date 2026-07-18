#[cfg(feature = "steamworks-runtime")]
pub mod account;
#[cfg(feature = "steamworks-runtime")]
pub mod achievements;
#[cfg(feature = "steamworks-runtime")]
mod app;
#[cfg(feature = "steamworks-runtime")]
pub mod apps;
#[cfg(feature = "steamworks-runtime")]
pub mod auth;
#[cfg(feature = "steamworks-runtime")]
pub mod cloud;
#[cfg(not(feature = "steamworks-runtime"))]
mod disabled;
pub mod error;
mod event_queue;
#[cfg(feature = "steamworks-runtime")]
pub mod events;
#[cfg(feature = "steamworks-runtime")]
pub mod friends;
#[cfg(feature = "steamworks-runtime")]
pub mod game_server;
#[cfg(feature = "steamworks-runtime")]
pub mod input;
#[cfg(feature = "steamworks-runtime")]
pub mod leaderboards;
#[cfg(feature = "steamworks-runtime")]
pub mod lobbies;
#[cfg(feature = "steamworks-runtime")]
pub mod networking;
#[cfg(feature = "steamworks-runtime")]
pub mod networking_messages;
#[cfg(feature = "steamworks-runtime")]
pub mod networking_sockets;
#[cfg(feature = "steamworks-runtime")]
pub mod networking_utils;
#[cfg(feature = "steamworks-runtime")]
pub mod remote_play;
#[cfg(feature = "steamworks-runtime")]
pub mod screenshots;
#[cfg(feature = "steamworks-runtime")]
pub mod servers;
#[cfg(feature = "steamworks-runtime")]
pub mod stats;
#[cfg(feature = "steamworks-runtime")]
pub mod timeline;
pub mod types;
#[cfg(feature = "steamworks-runtime")]
pub mod utils;
#[cfg(feature = "steamworks-runtime")]
pub mod workshop;

#[cfg(not(feature = "steamworks-runtime"))]
use disabled::app;

#[cfg(test)]
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(not(feature = "steamworks-runtime"))]
pub use disabled::{
    account, achievements, apps, auth, cloud, events, friends, input, leaderboards, lobbies,
    networking, networking_messages, networking_sockets, networking_utils, remote_play,
    screenshots, servers, stats, timeline, utils, workshop,
};

#[doc(hidden)]
pub mod runtime {
    use crate::error::SteamError;

    pub fn init_from_config(enabled: bool, app_id: Option<u32>) -> Result<(), SteamError> {
        crate::app::init_from_config(enabled, app_id)
    }

    pub fn init_from_config_with_input(
        enabled: bool,
        app_id: Option<u32>,
        input_mode: crate::input::SteamInputMode,
    ) -> Result<(), SteamError> {
        crate::app::init_from_config_with_input(enabled, app_id, input_mode)
    }

    pub fn run_callbacks() -> Result<(), SteamError> {
        crate::app::run_callbacks()
    }

    #[cfg(feature = "steamworks-runtime")]
    pub fn init_game_server(
        config: crate::game_server::GameServerConfig,
    ) -> Result<(), SteamError> {
        crate::game_server::init(config)
    }
}

pub use auth::{AuthSessionError, AuthTicket, UserHasLicense};
pub use error::SteamError;
pub use input::{
    ActionSetHandle, AnalogActionData, AnalogActionHandle, DigitalActionData, DigitalActionHandle,
    InputActionOrigin, InputController, InputHandle, InputSourceMode, InputType, MotionData,
    SteamInputMode,
};
pub use leaderboards::{
    LeaderboardDisplay, LeaderboardEntry, LeaderboardEntryScope, LeaderboardID,
    LeaderboardScoreUpload, LeaderboardSort, LeaderboardUploadMode,
};
pub use remote_play::{RemotePlaySession, RemotePlaySessionID, SteamDeviceFormFactor};
pub use screenshots::ScreenshotHandle;
pub use timeline::{TimelineEventClipPriority, TimelineGameMode};
pub use types::{
    AppID, ConnectionID, DLCID, FriendGame, FriendInfo, FriendListKind, FriendState, LobbyDataKey,
    LobbyDistance, LobbyID, LobbyInfo, LobbyJoinability, LobbyNearValueFilter,
    LobbyNumberComparison, LobbyNumberFilter, LobbySearch, LobbyStringFilter,
    LobbyStringFilterKind, LobbyType, OverlayDialog, RichPresenceKey, SocketID, SteamAvatar,
    SteamAvatarSize, SteamEvent, SteamEventQueueStats, SteamID, StoreOverlayAction,
    UserOverlayDialog, WorkshopFileID,
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
macro_rules! steam_friend_avatar {
    ($id:expr, $size:expr) => {
        $crate::friends::get_avatar($id, $size)
    };
}

#[macro_export]
macro_rules! steam_friend_avatar_small {
    ($id:expr) => {
        $crate::friends::get_small_avatar($id)
    };
}

#[macro_export]
macro_rules! steam_friend_avatar_medium {
    ($id:expr) => {
        $crate::friends::get_medium_avatar($id)
    };
}

#[macro_export]
macro_rules! steam_friend_avatar_large {
    ($id:expr) => {
        $crate::friends::get_large_avatar($id)
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
macro_rules! steam_leaderboard_find {
    ($name:expr, $cb:expr) => {
        $crate::leaderboards::find($name, $cb)
    };
}

#[macro_export]
macro_rules! steam_leaderboard_create {
    ($name:expr, $sort:expr, $display:expr, $cb:expr) => {
        $crate::leaderboards::find_or_create($name, $sort, $display, $cb)
    };
}

#[macro_export]
macro_rules! steam_leaderboard_upload {
    ($leaderboard:expr, $score:expr, $cb:expr) => {
        $crate::leaderboards::upload_score($leaderboard, $score, $cb)
    };
    ($leaderboard:expr, $mode:expr, $score:expr, $details:expr, $cb:expr) => {
        $crate::leaderboards::upload_score_with_details($leaderboard, $mode, $score, $details, $cb)
    };
}

#[macro_export]
macro_rules! steam_leaderboard_entries {
    ($leaderboard:expr, $start:expr, $end:expr, $cb:expr) => {
        $crate::leaderboards::entries_global($leaderboard, $start, $end, $cb)
    };
    ($leaderboard:expr, $scope:expr, $start:expr, $end:expr, $details_len:expr, $cb:expr) => {
        $crate::leaderboards::entries($leaderboard, $scope, $start, $end, $details_len, $cb)
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
        ActionSetHandle, AnalogActionData, AnalogActionHandle, AppID, AuthSessionError, AuthTicket,
        ConnectionID, DLCID, DigitalActionData, DigitalActionHandle, FriendGame, FriendInfo,
        FriendListKind, FriendState, InputActionOrigin, InputController, InputHandle,
        InputSourceMode, InputType, LeaderboardDisplay, LeaderboardEntry, LeaderboardEntryScope,
        LeaderboardID, LeaderboardScoreUpload, LeaderboardSort, LeaderboardUploadMode,
        LobbyDataKey, LobbyDistance, LobbyID, LobbyInfo, LobbyJoinability, LobbyNearValueFilter,
        LobbyNumberComparison, LobbyNumberFilter, LobbySearch, LobbyStringFilter,
        LobbyStringFilterKind, LobbyType, MotionData, OverlayDialog, RemotePlaySession,
        RemotePlaySessionID, RichPresenceKey, ScreenshotHandle, SocketID, SteamAvatar,
        SteamAvatarSize, SteamDeviceFormFactor, SteamError, SteamEvent, SteamID, SteamInputMode,
        StoreOverlayAction, TimelineEventClipPriority, TimelineGameMode, UserHasLicense,
        UserOverlayDialog, WorkshopFileID,
    };
    pub use crate::{
        steam_account_name, steam_account_self_id, steam_account_self_name, steam_ach_clear,
        steam_ach_unlock, steam_app_dlc_installed, steam_app_subscribed, steam_clear,
        steam_cloud_read, steam_cloud_write, steam_events, steam_friend_avatar,
        steam_friend_avatar_large, steam_friend_avatar_medium, steam_friend_avatar_small,
        steam_friend_list, steam_leaderboard_create, steam_leaderboard_entries,
        steam_leaderboard_find, steam_leaderboard_upload, steam_lobby_chat, steam_lobby_create,
        steam_lobby_data_set, steam_lobby_join, steam_lobby_leave, steam_p2p_read, steam_p2p_send,
        steam_rich_presence_set, steam_stat_get_i32, steam_stat_set_i32, steam_unlock,
        steam_workshop_download, steam_workshop_subscribe,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

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
            steam_friend_avatar!(SteamID::from_id(1), SteamAvatarSize::Small),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_friend_avatar_small!(SteamID::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_friend_avatar_medium!(SteamID::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_friend_avatar_large!(SteamID::from_id(1)),
            Err(SteamError::Disabled)
        );
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
