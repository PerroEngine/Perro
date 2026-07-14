pub const NET_MODE_OFFLINE: i64 = 0;
pub const NET_MODE_HOST: i64 = 1;
pub const NET_MODE_CLIENT: i64 = 2;

pub const DEFAULT_MAX_PLAYERS: i64 = 4;
pub const MIN_PLAYERS: i64 = 2;
pub const MAX_PLAYERS: i64 = 16;

pub const LOBBY_PRIVACY_PUBLIC: i64 = 0;
pub const LOBBY_PRIVACY_FRIENDS: i64 = 1;
pub const LOBBY_PRIVACY_PRIVATE: i64 = 2;

pub const LOBBY_DISTANCE_LOCAL: i64 = 0;
pub const LOBBY_DISTANCE_REGIONAL: i64 = 1;
pub const LOBBY_DISTANCE_WORLDWIDE: i64 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i64)]
pub enum NetMode {
    #[default]
    Offline = 0,
    Host = 1,
    Client = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i64)]
pub enum LobbyPrivacy {
    #[default]
    Public = 0,
    Friends = 1,
    Private = 2,
}

impl LobbyPrivacy {
    pub fn key(self) -> &'static str {
        match self {
            LobbyPrivacy::Public => "public",
            LobbyPrivacy::Friends => "friends",
            LobbyPrivacy::Private => "private",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i64)]
pub enum LobbyDistanceMode {
    Local = 0,
    Regional = 1,
    #[default]
    Worldwide = 2,
}

#[derive(Clone, Debug, Default)]
pub struct LobbyInfo {
    pub lobby_id: i64,
    pub owner_id: i64,
    pub name: String,
    pub members: i64,
    pub max_players: i64,
    pub started: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FriendLobbyInfo {
    pub steam_id: i64,
    pub lobby_id: i64,
    pub name: String,
    pub state: String,
}

pub fn clamp_max_players(value: i64) -> i64 {
    value.clamp(MIN_PLAYERS, MAX_PLAYERS)
}
