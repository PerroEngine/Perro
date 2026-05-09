use crate::{app, error::SteamError};

pub fn unlock(id: &str) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .achievement(id)
            .set()
            .map_err(|_| SteamError::CallFailed("achievement.set"))
    })
}

pub fn clear(id: &str) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .achievement(id)
            .clear()
            .map_err(|_| SteamError::CallFailed("achievement.clear"))
    })
}

pub fn store() -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .store_stats()
            .map_err(|_| SteamError::CallFailed("user_stats.store_stats"))
    })
}
