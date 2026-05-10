use std::borrow::Cow;
use std::net::{Ipv4Addr, SocketAddrV4};

#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct SteamID(u64);

impl SteamID {
    pub const fn from_id(id: u64) -> Self {
        Self(id)
    }

    pub const fn get_id(self) -> u64 {
        self.0
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<steamworks::SteamId> for SteamID {
    fn from(id: steamworks::SteamId) -> Self {
        Self(id.raw())
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<SteamID> for steamworks::SteamId {
    fn from(id: SteamID) -> Self {
        steamworks::SteamId::from_raw(id.0)
    }
}

#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct AppID(u32);

impl AppID {
    pub const fn from_id(id: u32) -> Self {
        Self(id)
    }

    pub const fn get_id(self) -> u32 {
        self.0
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<steamworks::AppId> for AppID {
    fn from(id: steamworks::AppId) -> Self {
        Self(id.0)
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<AppID> for steamworks::AppId {
    fn from(id: AppID) -> Self {
        steamworks::AppId(id.0)
    }
}

#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct DLCID(u32);

impl DLCID {
    pub const fn from_id(id: u32) -> Self {
        Self(id)
    }

    pub const fn get_id(self) -> u32 {
        self.0
    }
}

impl From<DLCID> for AppID {
    fn from(id: DLCID) -> Self {
        Self(id.0)
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<DLCID> for steamworks::AppId {
    fn from(id: DLCID) -> Self {
        steamworks::AppId(id.0)
    }
}

#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct LobbyID(u64);

impl LobbyID {
    pub const fn from_id(id: u64) -> Self {
        Self(id)
    }

    pub const fn get_id(self) -> u64 {
        self.0
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<steamworks::LobbyId> for LobbyID {
    fn from(id: steamworks::LobbyId) -> Self {
        Self(id.raw())
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<LobbyID> for steamworks::LobbyId {
    fn from(id: LobbyID) -> Self {
        steamworks::LobbyId::from_raw(id.0)
    }
}

#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WorkshopFileID(u64);

impl WorkshopFileID {
    pub const fn from_id(id: u64) -> Self {
        Self(id)
    }

    pub const fn get_id(self) -> u64 {
        self.0
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<steamworks::PublishedFileId> for WorkshopFileID {
    fn from(id: steamworks::PublishedFileId) -> Self {
        Self(id.0)
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<WorkshopFileID> for steamworks::PublishedFileId {
    fn from(id: WorkshopFileID) -> Self {
        steamworks::PublishedFileId(id.0)
    }
}

#[cfg(feature = "steamworks-runtime")]
pub type LeaderboardID = steamworks::Leaderboard;
#[cfg(not(feature = "steamworks-runtime"))]
pub type LeaderboardID = u64;
#[cfg(feature = "steamworks-runtime")]
pub type SocketID = steamworks::networking_sockets::ListenSocket;
#[cfg(not(feature = "steamworks-runtime"))]
pub type SocketID = u64;
#[cfg(feature = "steamworks-runtime")]
pub type ConnectionID = steamworks::networking_sockets::NetConnection;
#[cfg(not(feature = "steamworks-runtime"))]
pub type ConnectionID = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FriendState {
    Offline,
    Online,
    Invisible,
    Busy,
    Away,
    Snooze,
    LookingToPlay,
    LookingToTrade,
}

#[cfg(feature = "steamworks-runtime")]
impl From<steamworks::FriendState> for FriendState {
    fn from(state: steamworks::FriendState) -> Self {
        match state {
            steamworks::FriendState::Offline => Self::Offline,
            steamworks::FriendState::Online => Self::Online,
            steamworks::FriendState::Invisible => Self::Invisible,
            steamworks::FriendState::Busy => Self::Busy,
            steamworks::FriendState::Away => Self::Away,
            steamworks::FriendState::Snooze => Self::Snooze,
            steamworks::FriendState::LookingToPlay => Self::LookingToPlay,
            steamworks::FriendState::LookingToTrade => Self::LookingToTrade,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendGame {
    pub game_id: u64,
    pub game_address: Ipv4Addr,
    pub game_port: u16,
    pub query_port: u16,
    pub lobby: LobbyID,
}

#[cfg(feature = "steamworks-runtime")]
impl From<steamworks::FriendGame> for FriendGame {
    fn from(game: steamworks::FriendGame) -> Self {
        Self {
            game_id: game.game.raw(),
            game_address: game.game_address,
            game_port: game.game_port,
            query_port: game.query_port,
            lobby: game.lobby.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendInfo {
    pub id: SteamID,
    pub name: String,
    pub nickname: Option<String>,
    pub state: FriendState,
    pub game: Option<FriendGame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FriendListKind {
    Friends,
    All,
    Blocked,
    FriendshipRequested,
    ClanMembers,
    InGame,
    RequestingFriendship,
    RequestingInfo,
    Ignored,
    IgnoredFriend,
    ChatMembers,
}

#[cfg(feature = "steamworks-runtime")]
impl From<FriendListKind> for steamworks::FriendFlags {
    fn from(kind: FriendListKind) -> Self {
        match kind {
            FriendListKind::Friends => steamworks::FriendFlags::IMMEDIATE,
            FriendListKind::All => steamworks::FriendFlags::ALL,
            FriendListKind::Blocked => steamworks::FriendFlags::BLOCKED,
            FriendListKind::FriendshipRequested => steamworks::FriendFlags::FRIENDSHIP_REQUESTED,
            FriendListKind::ClanMembers => steamworks::FriendFlags::CLAN_MEMBER,
            FriendListKind::InGame => steamworks::FriendFlags::ON_GAME_SERVER,
            FriendListKind::RequestingFriendship => steamworks::FriendFlags::REQUESTING_FRIENDSHIP,
            FriendListKind::RequestingInfo => steamworks::FriendFlags::REQUESTING_INFO,
            FriendListKind::Ignored => steamworks::FriendFlags::IGNORED,
            FriendListKind::IgnoredFriend => steamworks::FriendFlags::IGNORED_FRIEND,
            FriendListKind::ChatMembers => steamworks::FriendFlags::CHAT_MEMBER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverlayDialog {
    Friends,
    Community,
    Players,
    Settings,
    OfficialGameGroup,
    Stats,
    Achievements,
    Custom(&'static str),
}

impl OverlayDialog {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Friends => "Friends",
            Self::Community => "Community",
            Self::Players => "Players",
            Self::Settings => "Settings",
            Self::OfficialGameGroup => "OfficialGameGroup",
            Self::Stats => "Stats",
            Self::Achievements => "Achievements",
            Self::Custom(dialog) => dialog,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UserOverlayDialog {
    Profile,
    Chat,
    JoinTrade,
    Stats,
    Achievements,
    FriendAdd,
    FriendRemove,
    FriendRequestAccept,
    FriendRequestIgnore,
    Custom(&'static str),
}

impl UserOverlayDialog {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Profile => "SteamID",
            Self::Chat => "chat",
            Self::JoinTrade => "jointrade",
            Self::Stats => "stats",
            Self::Achievements => "achievements",
            Self::FriendAdd => "friendadd",
            Self::FriendRemove => "friendremove",
            Self::FriendRequestAccept => "friendrequestaccept",
            Self::FriendRequestIgnore => "friendrequestignore",
            Self::Custom(dialog) => dialog,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreOverlayAction {
    Open,
    AddToCart,
    AddToCartAndShow,
}

#[cfg(feature = "steamworks-runtime")]
impl From<StoreOverlayAction> for steamworks::OverlayToStoreFlag {
    fn from(action: StoreOverlayAction) -> Self {
        match action {
            StoreOverlayAction::Open => steamworks::OverlayToStoreFlag::None,
            StoreOverlayAction::AddToCart => steamworks::OverlayToStoreFlag::AddToCart,
            StoreOverlayAction::AddToCartAndShow => {
                steamworks::OverlayToStoreFlag::AddToCartAndShow
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RichPresenceKey<'a> {
    Status,
    Connect,
    SteamDisplay,
    SteamPlayerGroup,
    SteamPlayerGroupSize,
    Custom(Cow<'a, str>),
}

impl<'a> RichPresenceKey<'a> {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Status => "status",
            Self::Connect => "connect",
            Self::SteamDisplay => "steam_display",
            Self::SteamPlayerGroup => "steam_player_group",
            Self::SteamPlayerGroupSize => "steam_player_group_size",
            Self::Custom(key) => key.as_ref(),
        }
    }
}

impl<'a> From<&'a str> for RichPresenceKey<'a> {
    fn from(key: &'a str) -> Self {
        Self::Custom(Cow::Borrowed(key))
    }
}

impl From<String> for RichPresenceKey<'static> {
    fn from(key: String) -> Self {
        Self::Custom(Cow::Owned(key))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LobbyDataKey<'a> {
    Name,
    Mode,
    Map,
    Region,
    Build,
    Version,
    JoinCode,
    Custom(Cow<'a, str>),
}

impl<'a> LobbyDataKey<'a> {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Name => "name",
            Self::Mode => "mode",
            Self::Map => "map",
            Self::Region => "region",
            Self::Build => "build",
            Self::Version => "version",
            Self::JoinCode => "join_code",
            Self::Custom(key) => key.as_ref(),
        }
    }
}

impl<'a> From<&'a str> for LobbyDataKey<'a> {
    fn from(key: &'a str) -> Self {
        Self::Custom(Cow::Borrowed(key))
    }
}

impl From<String> for LobbyDataKey<'static> {
    fn from(key: String) -> Self {
        Self::Custom(Cow::Owned(key))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LobbyType {
    Private,
    FriendsOnly,
    Public,
    Invisible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LobbyJoinability {
    Open,
    Closed,
}

impl LobbyJoinability {
    pub const fn as_bool(self) -> bool {
        match self {
            Self::Open => true,
            Self::Closed => false,
        }
    }
}

#[cfg(feature = "steamworks-runtime")]
impl From<LobbyType> for steamworks::LobbyType {
    fn from(kind: LobbyType) -> Self {
        match kind {
            LobbyType::Private => steamworks::LobbyType::Private,
            LobbyType::FriendsOnly => steamworks::LobbyType::FriendsOnly,
            LobbyType::Public => steamworks::LobbyType::Public,
            LobbyType::Invisible => steamworks::LobbyType::Invisible,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LobbyDistance {
    Close,
    Default,
    Far,
    Worldwide,
}

#[cfg(feature = "steamworks-runtime")]
impl From<LobbyDistance> for steamworks::DistanceFilter {
    fn from(distance: LobbyDistance) -> Self {
        match distance {
            LobbyDistance::Close => steamworks::DistanceFilter::Close,
            LobbyDistance::Default => steamworks::DistanceFilter::Default,
            LobbyDistance::Far => steamworks::DistanceFilter::Far,
            LobbyDistance::Worldwide => steamworks::DistanceFilter::Worldwide,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LobbyStringFilterKind {
    EqualToOrLessThan,
    LessThan,
    Equal,
    GreaterThan,
    EqualToOrGreaterThan,
    NotEqual,
}

#[cfg(feature = "steamworks-runtime")]
impl From<LobbyStringFilterKind> for steamworks::StringFilterKind {
    fn from(kind: LobbyStringFilterKind) -> Self {
        match kind {
            LobbyStringFilterKind::EqualToOrLessThan => {
                steamworks::StringFilterKind::EqualToOrLessThan
            }
            LobbyStringFilterKind::LessThan => steamworks::StringFilterKind::LessThan,
            LobbyStringFilterKind::Equal => steamworks::StringFilterKind::Equal,
            LobbyStringFilterKind::GreaterThan => steamworks::StringFilterKind::GreaterThan,
            LobbyStringFilterKind::EqualToOrGreaterThan => {
                steamworks::StringFilterKind::EqualToOrGreaterThan
            }
            LobbyStringFilterKind::NotEqual => steamworks::StringFilterKind::NotEqual,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LobbyNumberComparison {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanEqualTo,
    LessThan,
    LessThanEqualTo,
}

#[cfg(feature = "steamworks-runtime")]
impl From<LobbyNumberComparison> for steamworks::ComparisonFilter {
    fn from(comparison: LobbyNumberComparison) -> Self {
        match comparison {
            LobbyNumberComparison::Equal => steamworks::ComparisonFilter::Equal,
            LobbyNumberComparison::NotEqual => steamworks::ComparisonFilter::NotEqual,
            LobbyNumberComparison::GreaterThan => steamworks::ComparisonFilter::GreaterThan,
            LobbyNumberComparison::GreaterThanEqualTo => {
                steamworks::ComparisonFilter::GreaterThanEqualTo
            }
            LobbyNumberComparison::LessThan => steamworks::ComparisonFilter::LessThan,
            LobbyNumberComparison::LessThanEqualTo => steamworks::ComparisonFilter::LessThanEqualTo,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyStringFilter<'a> {
    pub key: LobbyDataKey<'a>,
    pub value: Cow<'a, str>,
    pub kind: LobbyStringFilterKind,
}

impl<'a> LobbyStringFilter<'a> {
    pub fn new(
        key: impl Into<LobbyDataKey<'a>>,
        value: impl Into<Cow<'a, str>>,
        kind: LobbyStringFilterKind,
    ) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            kind,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyNumberFilter<'a> {
    pub key: LobbyDataKey<'a>,
    pub value: i32,
    pub comparison: LobbyNumberComparison,
}

impl<'a> LobbyNumberFilter<'a> {
    pub fn new(
        key: impl Into<LobbyDataKey<'a>>,
        value: i32,
        comparison: LobbyNumberComparison,
    ) -> Self {
        Self {
            key: key.into(),
            value,
            comparison,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyNearValueFilter<'a> {
    pub key: LobbyDataKey<'a>,
    pub value: i32,
}

impl<'a> LobbyNearValueFilter<'a> {
    pub fn new(key: impl Into<LobbyDataKey<'a>>, value: i32) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LobbySearch<'a> {
    pub max_results: Option<u64>,
    pub open_slots: Option<u8>,
    pub distance: Option<LobbyDistance>,
    pub string_filters: Vec<LobbyStringFilter<'a>>,
    pub number_filters: Vec<LobbyNumberFilter<'a>>,
    pub near_value_filters: Vec<LobbyNearValueFilter<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyInfo {
    pub id: LobbyID,
    pub owner: SteamID,
    pub members: Vec<SteamID>,
    pub member_limit: Option<usize>,
    pub data: Vec<(String, String)>,
    pub game_server: Option<(SocketAddrV4, Option<SteamID>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SteamEvent {
    LobbyList {
        lobbies: Vec<LobbyID>,
    },
    LobbyListFailed,
    LobbyCreated {
        lobby: LobbyID,
    },
    LobbyCreateFailed,
    LobbyJoined {
        lobby: LobbyID,
    },
    LobbyJoinFailed {
        lobby: LobbyID,
    },
    LobbyDataUpdated {
        lobby: LobbyID,
        member: SteamID,
    },
    LobbyChat {
        lobby: LobbyID,
        user: SteamID,
        chat_id: i32,
    },
    LobbyMemberChanged {
        lobby: LobbyID,
        user: SteamID,
    },
    LobbyJoinRequested {
        lobby: LobbyID,
        friend: SteamID,
    },
    RichPresenceJoinRequested {
        friend: SteamID,
        connect: String,
    },
    PersonaChanged {
        user: SteamID,
    },
    OverlayChanged {
        active: bool,
    },
    Callback {
        name: &'static str,
    },
}

#[cfg(all(test, feature = "steamworks-runtime"))]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip() {
        assert_eq!(SteamID::from_id(42).get_id(), 42);
        assert_eq!(LobbyID::from_id(99).get_id(), 99);
        assert_eq!(AppID::from_id(480).get_id(), 480);
        assert_eq!(DLCID::from_id(12345).get_id(), 12345);
        assert_eq!(WorkshopFileID::from_id(77).get_id(), 77);
    }

    #[test]
    fn enum_maps_lobby_type() {
        assert_eq!(
            steamworks::LobbyType::from(LobbyType::FriendsOnly),
            steamworks::LobbyType::FriendsOnly
        );
    }

    #[test]
    fn enum_maps_friend_list_kind() {
        assert_eq!(
            steamworks::FriendFlags::from(FriendListKind::Friends),
            steamworks::FriendFlags::IMMEDIATE
        );
        assert_eq!(
            steamworks::FriendFlags::from(FriendListKind::All),
            steamworks::FriendFlags::ALL
        );
        assert_eq!(
            steamworks::FriendFlags::from(FriendListKind::InGame),
            steamworks::FriendFlags::ON_GAME_SERVER
        );
    }

    #[test]
    fn enum_maps_lobby_filters() {
        assert_eq!(
            steamworks::DistanceFilter::from(LobbyDistance::Worldwide),
            steamworks::DistanceFilter::Worldwide
        );
        assert_eq!(
            steamworks::StringFilterKind::from(LobbyStringFilterKind::NotEqual),
            steamworks::StringFilterKind::NotEqual
        );
        assert_eq!(
            steamworks::ComparisonFilter::from(LobbyNumberComparison::GreaterThanEqualTo),
            steamworks::ComparisonFilter::GreaterThanEqualTo
        );
    }

    #[test]
    fn typed_keys_expose_expected_strings_and_custom_passthrough() {
        assert_eq!(RichPresenceKey::Status.as_str(), "status");
        assert_eq!(RichPresenceKey::Connect.as_str(), "connect");
        assert_eq!(
            RichPresenceKey::from("party_status").as_str(),
            "party_status"
        );
        assert_eq!(
            RichPresenceKey::from("queue_state".to_string()).as_str(),
            "queue_state"
        );

        assert_eq!(LobbyDataKey::Mode.as_str(), "mode");
        assert_eq!(LobbyDataKey::JoinCode.as_str(), "join_code");
        assert_eq!(LobbyDataKey::from("difficulty").as_str(), "difficulty");
        assert_eq!(LobbyDataKey::from("season".to_string()).as_str(), "season");
    }

    #[test]
    fn overlay_dialog_enums_and_custom_passthrough() {
        assert_eq!(OverlayDialog::Friends.as_str(), "Friends");
        assert_eq!(OverlayDialog::Custom("Workshop").as_str(), "Workshop");
        assert_eq!(UserOverlayDialog::Profile.as_str(), "SteamID");
        assert_eq!(UserOverlayDialog::Custom("inventory").as_str(), "inventory");
    }

    #[test]
    fn lobby_joinability_maps_to_bool() {
        assert!(LobbyJoinability::Open.as_bool());
        assert!(!LobbyJoinability::Closed.as_bool());
    }

    #[test]
    fn lobby_search_filters_accept_borrowed_owned_and_custom_keys() {
        let borrowed =
            LobbyStringFilter::new(LobbyDataKey::Mode, "coop", LobbyStringFilterKind::Equal);
        assert_eq!(borrowed.key.as_str(), "mode");
        assert!(matches!(borrowed.value, Cow::Borrowed("coop")));

        let owned = LobbyStringFilter::new(
            LobbyDataKey::from("difficulty".to_string()),
            "hard".to_string(),
            LobbyStringFilterKind::NotEqual,
        );
        assert_eq!(owned.key.as_str(), "difficulty");
        assert_eq!(owned.value.as_ref(), "hard");

        let number = LobbyNumberFilter::new("rank", 10, LobbyNumberComparison::GreaterThan);
        assert_eq!(number.key.as_str(), "rank");
        assert_eq!(number.value, 10);

        let near = LobbyNearValueFilter::new(LobbyDataKey::Region, 2);
        assert_eq!(near.key.as_str(), "region");
        assert_eq!(near.value, 2);

        let search = LobbySearch {
            string_filters: vec![borrowed, owned],
            number_filters: vec![number],
            near_value_filters: vec![near],
            ..Default::default()
        };
        assert_eq!(search.string_filters.len(), 2);
        assert_eq!(search.number_filters[0].key.as_str(), "rank");
        assert_eq!(search.near_value_filters[0].key.as_str(), "region");
    }
}
