use crate::{app, error::SteamError};

pub type LeaderboardID = steamworks::Leaderboard;
pub type LeaderboardEntry = steamworks::LeaderboardEntry;
pub type LeaderboardDataRequest = steamworks::LeaderboardDataRequest;
pub type LeaderboardDisplayType = steamworks::LeaderboardDisplayType;
pub type LeaderboardSortMethod = steamworks::LeaderboardSortMethod;
pub type LeaderboardScoreUploaded = steamworks::LeaderboardScoreUploaded;
pub type UploadScoreMethod = steamworks::UploadScoreMethod;

pub fn find(
    name: &str,
    cb: impl FnOnce(Result<Option<LeaderboardID>, steamworks::SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user_stats().find_leaderboard(name, cb);
        Ok(())
    })
}

pub fn find_or_create(
    name: &str,
    sort: LeaderboardSortMethod,
    display: LeaderboardDisplayType,
    cb: impl FnOnce(Result<Option<LeaderboardID>, steamworks::SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .find_or_create_leaderboard(name, sort, display, cb);
        Ok(())
    })
}

pub fn upload(
    leaderboard: &LeaderboardID,
    method: UploadScoreMethod,
    score: i32,
    details: &[i32],
    cb: impl FnOnce(Result<Option<LeaderboardScoreUploaded>, steamworks::SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .user_stats()
            .upload_leaderboard_score(leaderboard, method, score, details, cb);
        Ok(())
    })
}

pub fn entries(
    leaderboard: &LeaderboardID,
    request: LeaderboardDataRequest,
    start: usize,
    end: usize,
    max_details_len: usize,
    cb: impl FnOnce(Result<Vec<LeaderboardEntry>, steamworks::SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user_stats().download_leaderboard_entries(
            leaderboard,
            request,
            start,
            end,
            max_details_len,
            cb,
        );
        Ok(())
    })
}
