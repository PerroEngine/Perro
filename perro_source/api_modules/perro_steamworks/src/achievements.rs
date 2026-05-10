use crate::{app, error::SteamError};

pub fn unlock(id: &str) -> Result<(), SteamError> {
    unlock_many([id])
}

pub fn unlock_many<I, S>(ids: I) -> Result<(), SteamError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    app::with_client(|client| {
        let user_stats = client.user_stats();
        for id in ids {
            user_stats
                .achievement(id.as_ref())
                .set()
                .map_err(|_| SteamError::CallFailed("achievement.set"))?;
        }
        app::request_stats_store()
    })
}

pub fn clear(id: &str) -> Result<(), SteamError> {
    app::with_client(|client| {
        let user_stats = client.user_stats();
        user_stats
            .achievement(id)
            .clear()
            .map_err(|_| SteamError::CallFailed("achievement.clear"))?;
        app::request_stats_store()
    })
}

pub trait AchievementUnlockInput {
    fn unlock(self) -> Result<(), SteamError>;
}

impl AchievementUnlockInput for &str {
    fn unlock(self) -> Result<(), SteamError> {
        unlock(self)
    }
}

impl AchievementUnlockInput for &String {
    fn unlock(self) -> Result<(), SteamError> {
        unlock(self.as_str())
    }
}

impl<S> AchievementUnlockInput for &[S]
where
    S: AsRef<str>,
{
    fn unlock(self) -> Result<(), SteamError> {
        unlock_many(self.iter().map(AsRef::as_ref))
    }
}

impl<S, const N: usize> AchievementUnlockInput for &[S; N]
where
    S: AsRef<str>,
{
    fn unlock(self) -> Result<(), SteamError> {
        unlock_many(self.iter().map(AsRef::as_ref))
    }
}

impl<S> AchievementUnlockInput for &Vec<S>
where
    S: AsRef<str>,
{
    fn unlock(self) -> Result<(), SteamError> {
        unlock_many(self.iter().map(AsRef::as_ref))
    }
}

pub fn unlock_input(input: impl AchievementUnlockInput) -> Result<(), SteamError> {
    input.unlock()
}
