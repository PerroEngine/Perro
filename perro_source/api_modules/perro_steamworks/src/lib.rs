pub mod achievements;
pub mod app;
pub mod error;
pub mod friends;
pub mod lobbies;

pub use error::SteamError;

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

pub mod prelude {
    pub use crate::SteamError;
    pub use crate::achievements;
    pub use crate::app;
    pub use crate::{steam_ach_clear, steam_ach_unlock, steam_clear, steam_unlock};
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
        app::run_callbacks();
        assert!(!app::enabled());
        assert!(!app::ready());
        assert_eq!(app::app_id(), None);
    }

    #[test]
    fn callbacks_before_init_noop() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::run_callbacks();
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
        assert!(!app::enabled());
        assert!(!app::ready());
    }

    #[test]
    fn disabled_init_with_app_id_stays_disabled() {
        let _guard = test_lock();
        app::reset_for_tests();
        app::init_from_config(false, Some(480)).expect("disabled init");
        assert!(!app::enabled());
        assert!(!app::ready());
        assert_eq!(app::app_id(), None);
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
        assert!(app::enabled());
        assert!(app::ready());
        assert_eq!(app::app_id(), Some(480));
        app::init_from_config(true, Some(480)).expect("same AppId re-init");
        app::run_callbacks();
    }
}
