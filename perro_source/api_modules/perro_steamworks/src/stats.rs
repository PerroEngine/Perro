use crate::{app, error::SteamError};

pub fn achievement_unlocked(id: &str) -> Result<bool, SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .achievement(id)
            .get()
            .map_err(|_| SteamError::CallFailed("user_stats.achievement.get"))
    })
}

pub fn achievement_unlock_time(id: &str) -> Result<(bool, u32), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .achievement(id)
            .get_achievement_and_unlock_time()
            .map_err(|_| SteamError::CallFailed("user_stats.achievement.unlock_time"))
    })
}

pub fn achievement_percent(id: &str) -> Result<f32, SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .achievement(id)
            .get_achievement_achieved_percent()
            .map_err(|_| SteamError::CallFailed("user_stats.achievement.percent"))
    })
}

pub fn achievement_names() -> Result<Option<Vec<String>>, SteamError> {
    app::with_client(|client| Ok(client.user_stats().get_achievement_names()))
}

pub fn get_i32(name: &str) -> Result<i32, SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .get_stat_i32(name)
            .map_err(|_| SteamError::CallFailed("user_stats.get_i32"))
    })
}

pub fn set_i32(name: &str, value: i32) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .set_stat_i32(name, value)
            .map_err(|_| SteamError::CallFailed("user_stats.set_i32"))?;
        app::request_stats_store()
    })
}

pub fn get_f32(name: &str) -> Result<f32, SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .get_stat_f32(name)
            .map_err(|_| SteamError::CallFailed("user_stats.get_f32"))
    })
}

pub fn set_f32(name: &str, value: f32) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .set_stat_f32(name, value)
            .map_err(|_| SteamError::CallFailed("user_stats.set_f32"))?;
        app::request_stats_store()
    })
}

pub fn global_i64(name: &str) -> Result<i64, SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .get_global_stat_i64(name)
            .map_err(|_| SteamError::CallFailed("user_stats.global_i64"))
    })
}

pub fn global_f64(name: &str) -> Result<f64, SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .get_global_stat_f64(name)
            .map_err(|_| SteamError::CallFailed("user_stats.global_f64"))
    })
}

pub fn reset_all(achievements_too: bool) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .reset_all_stats(achievements_too)
            .map_err(|_| SteamError::CallFailed("user_stats.reset_all_stats"))
    })
}
