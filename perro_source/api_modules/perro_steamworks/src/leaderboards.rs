use crate::{app, error::SteamError, types::SteamID};

#[derive(Clone, Debug)]
pub struct LeaderboardID {
    raw: steamworks::Leaderboard,
}

impl LeaderboardID {
    fn new(raw: steamworks::Leaderboard) -> Self {
        Self { raw }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardEntry {
    pub user: SteamID,
    pub global_rank: i32,
    pub score: i32,
    pub details: Vec<i32>,
}

impl From<steamworks::LeaderboardEntry> for LeaderboardEntry {
    fn from(entry: steamworks::LeaderboardEntry) -> Self {
        Self {
            user: entry.user.into(),
            global_rank: entry.global_rank,
            score: entry.score,
            details: entry.details,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardScoreUpload {
    pub score: i32,
    pub changed: bool,
    pub global_rank_new: i32,
    pub global_rank_previous: i32,
}

impl From<steamworks::LeaderboardScoreUploaded> for LeaderboardScoreUpload {
    fn from(upload: steamworks::LeaderboardScoreUploaded) -> Self {
        Self {
            score: upload.score,
            changed: upload.was_changed,
            global_rank_new: upload.global_rank_new,
            global_rank_previous: upload.global_rank_previous,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderboardSort {
    Ascending,
    Descending,
}

impl From<LeaderboardSort> for steamworks::LeaderboardSortMethod {
    fn from(sort: LeaderboardSort) -> Self {
        match sort {
            LeaderboardSort::Ascending => Self::Ascending,
            LeaderboardSort::Descending => Self::Descending,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderboardDisplay {
    Numeric,
    TimeSeconds,
    TimeMilliseconds,
}

impl From<LeaderboardDisplay> for steamworks::LeaderboardDisplayType {
    fn from(display: LeaderboardDisplay) -> Self {
        match display {
            LeaderboardDisplay::Numeric => Self::Numeric,
            LeaderboardDisplay::TimeSeconds => Self::TimeSeconds,
            LeaderboardDisplay::TimeMilliseconds => Self::TimeMilliSeconds,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderboardUploadMode {
    KeepBest,
    Force,
}

impl From<LeaderboardUploadMode> for steamworks::UploadScoreMethod {
    fn from(mode: LeaderboardUploadMode) -> Self {
        match mode {
            LeaderboardUploadMode::KeepBest => Self::KeepBest,
            LeaderboardUploadMode::Force => Self::ForceUpdate,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderboardEntryScope {
    Global,
    AroundUser,
    Friends,
}

impl From<LeaderboardEntryScope> for steamworks::LeaderboardDataRequest {
    fn from(scope: LeaderboardEntryScope) -> Self {
        match scope {
            LeaderboardEntryScope::Global => Self::Global,
            LeaderboardEntryScope::AroundUser => Self::GlobalAroundUser,
            LeaderboardEntryScope::Friends => Self::Friends,
        }
    }
}

pub fn find(
    name: &str,
    cb: impl FnOnce(Result<Option<LeaderboardID>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user_stats().find_leaderboard(name, move |result| {
            cb(map_find_result(result, "user_stats.find_leaderboard"));
        });
        Ok(())
    })
}

pub fn find_or_create(
    name: &str,
    sort: LeaderboardSort,
    display: LeaderboardDisplay,
    cb: impl FnOnce(Result<Option<LeaderboardID>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user_stats().find_or_create_leaderboard(
            name,
            sort.into(),
            display.into(),
            move |result| {
                cb(map_find_result(
                    result,
                    "user_stats.find_or_create_leaderboard",
                ));
            },
        );
        Ok(())
    })
}

pub fn upload_score(
    leaderboard: &LeaderboardID,
    score: i32,
    cb: impl FnOnce(Result<Option<LeaderboardScoreUpload>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    upload_score_with_details(leaderboard, LeaderboardUploadMode::KeepBest, score, &[], cb)
}

pub fn force_upload_score(
    leaderboard: &LeaderboardID,
    score: i32,
    cb: impl FnOnce(Result<Option<LeaderboardScoreUpload>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    upload_score_with_details(leaderboard, LeaderboardUploadMode::Force, score, &[], cb)
}

pub fn upload_score_with_details(
    leaderboard: &LeaderboardID,
    mode: LeaderboardUploadMode,
    score: i32,
    details: &[i32],
    cb: impl FnOnce(Result<Option<LeaderboardScoreUpload>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user_stats().upload_leaderboard_score(
            &leaderboard.raw,
            mode.into(),
            score,
            details,
            move |result| {
                cb(match result {
                    Ok(upload) => Ok(upload.map(Into::into)),
                    Err(_) => Err(SteamError::CallFailed(
                        "user_stats.upload_leaderboard_score",
                    )),
                });
            },
        );
        Ok(())
    })
}

pub fn entries_global(
    leaderboard: &LeaderboardID,
    start: usize,
    end: usize,
    cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    entries(
        leaderboard,
        LeaderboardEntryScope::Global,
        start,
        end,
        0,
        cb,
    )
}

pub fn entries_around_user(
    leaderboard: &LeaderboardID,
    start: usize,
    end: usize,
    cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    entries(
        leaderboard,
        LeaderboardEntryScope::AroundUser,
        start,
        end,
        0,
        cb,
    )
}

pub fn entries_friends(
    leaderboard: &LeaderboardID,
    cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    entries(leaderboard, LeaderboardEntryScope::Friends, 0, 0, 0, cb)
}

pub fn entries(
    leaderboard: &LeaderboardID,
    scope: LeaderboardEntryScope,
    start: usize,
    end: usize,
    max_details_len: usize,
    cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.user_stats().download_leaderboard_entries(
            &leaderboard.raw,
            scope.into(),
            start,
            end,
            max_details_len,
            move |result| {
                cb(match result {
                    Ok(entries) => Ok(entries.into_iter().map(Into::into).collect()),
                    Err(_) => Err(SteamError::CallFailed(
                        "user_stats.download_leaderboard_entries",
                    )),
                });
            },
        );
        Ok(())
    })
}

fn map_find_result(
    result: Result<Option<steamworks::Leaderboard>, steamworks::SteamError>,
    err: &'static str,
) -> Result<Option<LeaderboardID>, SteamError> {
    match result {
        Ok(board) => Ok(board.map(LeaderboardID::new)),
        Err(_) => Err(SteamError::CallFailed(err)),
    }
}
