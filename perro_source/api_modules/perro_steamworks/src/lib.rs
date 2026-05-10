pub mod account;
pub mod achievements;
pub mod app;
pub mod error;
pub mod events;
pub mod friends;
pub mod lobbies;
pub mod types;

pub use error::SteamError;
pub use types::{
    FriendGame, FriendInfo, FriendListKind, FriendState, LobbyDataKey, LobbyDistance, LobbyId,
    LobbyInfo, LobbyJoinability, LobbyNearValueFilter, LobbyNumberComparison, LobbyNumberFilter,
    LobbySearch, LobbyStringFilter, LobbyStringFilterKind, LobbyType, OverlayDialog,
    RichPresenceKey, SteamEvent, SteamID, UserOverlayDialog,
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
        $crate::friends::list()
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

pub mod prelude {
    pub use crate::account;
    pub use crate::achievements;
    pub use crate::app;
    pub use crate::events;
    pub use crate::friends;
    pub use crate::lobbies;
    pub use crate::{
        FriendGame, FriendInfo, FriendListKind, FriendState, LobbyDataKey, LobbyDistance, LobbyId,
        LobbyInfo, LobbyJoinability, LobbyNearValueFilter, LobbyNumberComparison,
        LobbyNumberFilter, LobbySearch, LobbyStringFilter, LobbyStringFilterKind, LobbyType,
        OverlayDialog, RichPresenceKey, SteamError, SteamEvent, SteamID, UserOverlayDialog,
    };
    pub use crate::{
        steam_account_name, steam_account_self_id, steam_account_self_name, steam_ach_clear,
        steam_ach_unlock, steam_clear, steam_events, steam_friend_list, steam_lobby_chat,
        steam_lobby_create, steam_lobby_data_set, steam_lobby_join, steam_lobby_leave,
        steam_rich_presence_set, steam_unlock,
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
        assert_eq!(app::enabled(), Ok(false));
        assert_eq!(app::ready(), Ok(false));
        assert_eq!(app::app_id(), Ok(None));
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
        assert_eq!(app::enabled(), Ok(false));
        assert_eq!(app::ready(), Ok(false));
    }

    #[test]
    fn disabled_init_with_app_id_stays_disabled() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, Some(480)).expect("disabled init");
        assert_eq!(app::enabled(), Ok(false));
        assert_eq!(app::ready(), Ok(false));
        assert_eq!(app::app_id(), Ok(None));
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
            steam_lobby_join!(LobbyId::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_leave!(LobbyId::from_id(1)),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_data_set!(LobbyId::from_id(1), "mode", "coop"),
            Err(SteamError::Disabled)
        );
        assert_eq!(
            steam_lobby_chat!(LobbyId::from_id(1), "hi"),
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
        assert_eq!(app::enabled(), Ok(true));
        assert_eq!(app::ready(), Ok(true));
        assert_eq!(app::app_id(), Ok(Some(480)));
        app::init_from_config(true, Some(480)).expect("same AppId re-init");
        app::run_callbacks().expect("callbacks");
    }
}
