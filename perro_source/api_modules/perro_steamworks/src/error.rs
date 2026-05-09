use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamError {
    Disabled,
    NotReady,
    MissingAppId,
    AlreadyInitialized { current: u32, requested: u32 },
    InitFailed(String),
    CallFailed(&'static str),
}

impl Display for SteamError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => write!(f, "Steam disabled"),
            Self::NotReady => write!(f, "Steam not ready"),
            Self::MissingAppId => write!(f, "Steam app_id missing"),
            Self::AlreadyInitialized { current, requested } => write!(
                f,
                "Steam already initialized with app_id {current}, requested {requested}"
            ),
            Self::InitFailed(err) => write!(f, "Steam init failed: {err}"),
            Self::CallFailed(call) => write!(f, "Steam call failed: {call}"),
        }
    }
}

impl std::error::Error for SteamError {}
