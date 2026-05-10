use crate::error::SteamError;
use crate::types::*;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DisabledHandle(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisabledEnum {
    #[default]
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisabledSendType {
    Unreliable,
    UnreliableNoDelay,
    #[default]
    Reliable,
    ReliableWithBuffering,
}

fn disabled<T>() -> Result<T, SteamError> {
    Err(SteamError::Disabled)
}

pub mod app {
    use super::SteamError;
    use std::sync::{Mutex, OnceLock};

    #[derive(Default)]
    struct State {
        app_id: Option<u32>,
    }

    fn state() -> &'static Mutex<State> {
        static STATE: OnceLock<Mutex<State>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(State::default()))
    }

    #[cfg(test)]
    pub(crate) fn reset_for_tests() {
        if let Ok(mut state) = state().lock() {
            *state = State::default();
        }
    }

    pub fn init_from_config(enabled: bool, app_id: Option<u32>) -> Result<(), SteamError> {
        if enabled {
            if app_id.is_none() {
                return Err(SteamError::MissingAppId);
            }
            return Err(SteamError::Disabled);
        }
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        state.app_id = None;
        let _ = app_id;
        Ok(())
    }

    pub fn run_callbacks() -> Result<(), SteamError> {
        Ok(())
    }

    pub fn is_enabled() -> Result<bool, SteamError> {
        Ok(false)
    }

    pub fn is_ready() -> Result<bool, SteamError> {
        Ok(false)
    }

    pub fn get_app_id() -> Result<Option<u32>, SteamError> {
        Ok(None)
    }
}

pub mod achievements {
    use super::{SteamError, disabled};

    pub fn unlock(_id: &str) -> Result<(), SteamError> {
        disabled()
    }

    pub fn unlock_many<I, S>(_ids: I) -> Result<(), SteamError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        disabled()
    }

    pub fn clear(_id: &str) -> Result<(), SteamError> {
        disabled()
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
}

pub mod events {
    use super::{SteamError, SteamEvent};
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    fn queue() -> &'static Mutex<VecDeque<SteamEvent>> {
        static QUEUE: OnceLock<Mutex<VecDeque<SteamEvent>>> = OnceLock::new();
        QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    #[allow(dead_code)]
    pub(crate) fn push(event: SteamEvent) {
        if let Ok(mut queue) = queue().lock() {
            queue.push_back(event);
        }
    }

    pub fn poll_one() -> Result<Option<SteamEvent>, SteamError> {
        queue()
            .lock()
            .map(|mut queue| queue.pop_front())
            .map_err(|_| SteamError::NotReady)
    }

    pub fn drain() -> Result<Vec<SteamEvent>, SteamError> {
        queue()
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .map_err(|_| SteamError::NotReady)
    }

    pub fn clear() -> Result<(), SteamError> {
        queue()
            .lock()
            .map(|mut queue| queue.clear())
            .map_err(|_| SteamError::NotReady)
    }
}

pub mod account {
    use super::{SteamError, SteamID, disabled};

    pub fn get_self_id() -> Result<SteamID, SteamError> {
        disabled()
    }
    pub fn get_name(_id: SteamID) -> Result<String, SteamError> {
        disabled()
    }
    pub fn get_self_name() -> Result<String, SteamError> {
        disabled()
    }
    pub fn get_level() -> Result<u32, SteamError> {
        disabled()
    }
    pub fn is_logged_on() -> Result<bool, SteamError> {
        disabled()
    }
}

pub mod apps {
    use super::{AppID, DLCID, SteamError, SteamID, disabled};

    pub fn is_installed(_app_id: AppID) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_dlc_installed(_app_id: AppID) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_dlc_id_installed(_dlc_id: DLCID) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_subscribed() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_subscribed_app(_app_id: AppID) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_subscribed_from_free_weekend() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_vac_banned() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_cybercafe() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_low_violence() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn get_build_id() -> Result<i32, SteamError> {
        disabled()
    }
    pub fn get_install_dir(_app_id: AppID) -> Result<String, SteamError> {
        disabled()
    }
    pub fn get_owner() -> Result<SteamID, SteamError> {
        disabled()
    }
    pub fn get_available_languages() -> Result<Vec<String>, SteamError> {
        disabled()
    }
    pub fn get_current_language() -> Result<String, SteamError> {
        disabled()
    }
    pub fn get_current_beta_name() -> Result<Option<String>, SteamError> {
        disabled()
    }
    pub fn get_launch_command_line() -> Result<String, SteamError> {
        disabled()
    }
    pub fn get_launch_query_param(_key: &str) -> Result<String, SteamError> {
        disabled()
    }
}

pub mod cloud {
    use super::{DisabledHandle, SteamError, disabled};

    pub type FileInfo = DisabledHandle;
    pub type Platforms = DisabledHandle;

    pub fn set_enabled_for_app(_enabled: bool) -> Result<(), SteamError> {
        disabled()
    }
    pub fn is_enabled_for_app() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_enabled_for_account() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn get_files() -> Result<Vec<FileInfo>, SteamError> {
        disabled()
    }
    pub fn is_file_present(_name: &str) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn delete(_name: &str) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn get_file_bytes(_name: &str) -> Result<Vec<u8>, SteamError> {
        disabled()
    }
    pub fn write(_name: &str, _bytes: &[u8]) -> Result<(), SteamError> {
        disabled()
    }
}

pub mod friends {
    use super::*;

    pub fn get_list() -> Result<Vec<FriendInfo>, SteamError> {
        disabled()
    }
    pub fn get_list_by(_kind: FriendListKind) -> Result<Vec<FriendInfo>, SteamError> {
        disabled()
    }
    pub fn get(_id: SteamID) -> Result<FriendInfo, SteamError> {
        disabled()
    }
    pub fn get_rich_presence<'a>(
        _id: SteamID,
        _key: impl Into<RichPresenceKey<'a>>,
    ) -> Result<Option<String>, SteamError> {
        disabled()
    }
    pub fn set_rich_presence<'a>(
        _key: impl Into<RichPresenceKey<'a>>,
        _value: &str,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn clear_rich_presence() -> Result<(), SteamError> {
        disabled()
    }
    pub fn open_overlay(_dialog: OverlayDialog) -> Result<(), SteamError> {
        disabled()
    }
    pub fn open_user_overlay(_dialog: UserOverlayDialog, _user: SteamID) -> Result<(), SteamError> {
        disabled()
    }
    pub fn open_store(_app_id: AppID, _action: StoreOverlayAction) -> Result<(), SteamError> {
        disabled()
    }
    pub fn open_web_page(_url: &str) -> Result<(), SteamError> {
        disabled()
    }
    pub fn open_invite_dialog(_lobby: LobbyID) -> Result<(), SteamError> {
        disabled()
    }
    pub fn invite_user_to_game(_user: SteamID, _connect: &str) -> Result<(), SteamError> {
        disabled()
    }
}

pub mod input {
    use super::{DisabledEnum, SteamError, disabled};

    pub type InputType = DisabledEnum;

    pub fn is_init(_explicitly_call_run_frame: bool) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn run_frame() -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_connected_controllers() -> Result<Vec<u64>, SteamError> {
        disabled()
    }
    pub fn is_action_manifest_set(_path: &str) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_binding_panel_shown(_input_handle: u64) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn shutdown() -> Result<(), SteamError> {
        disabled()
    }
}

pub mod auth {
    use super::{AppID, DisabledEnum, DisabledHandle, SteamError, SteamID, disabled};

    pub type AuthTicket = DisabledHandle;
    pub type AuthSessionError = DisabledEnum;
    pub type UserHasLicense = DisabledEnum;

    pub fn authentication_session_ticket() -> Result<(AuthTicket, Vec<u8>), SteamError> {
        disabled()
    }
    pub fn authentication_session_ticket_with_steam_id(
        _remote: SteamID,
    ) -> Result<(AuthTicket, Vec<u8>), SteamError> {
        disabled()
    }
    pub fn cancel_authentication_ticket(_ticket: AuthTicket) -> Result<(), SteamError> {
        disabled()
    }
    pub fn begin_authentication_session(
        _user: SteamID,
        _ticket: &[u8],
    ) -> Result<AuthSessionError, SteamError> {
        disabled()
    }
    pub fn end_authentication_session(_user: SteamID) -> Result<(), SteamError> {
        disabled()
    }
    pub fn authentication_session_ticket_for_webapi(
        _identity: &str,
    ) -> Result<AuthTicket, SteamError> {
        disabled()
    }
    pub fn user_has_license_for_app(
        _user: SteamID,
        _app_id: AppID,
    ) -> Result<UserHasLicense, SteamError> {
        disabled()
    }
}

pub mod lobbies {
    use super::*;

    pub fn create(_kind: LobbyType, _max_members: u32) -> Result<(), SteamError> {
        disabled()
    }
    pub fn request_list(_search: LobbySearch<'_>) -> Result<(), SteamError> {
        disabled()
    }
    pub fn join(_lobby: LobbyID) -> Result<(), SteamError> {
        disabled()
    }
    pub fn leave(_lobby: LobbyID) -> Result<(), SteamError> {
        disabled()
    }
    pub fn set_data<'a>(
        _lobby: LobbyID,
        _key: impl Into<LobbyDataKey<'a>>,
        _value: &str,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_data<'a>(
        _lobby: LobbyID,
        _key: impl Into<LobbyDataKey<'a>>,
    ) -> Result<Option<String>, SteamError> {
        disabled()
    }
    pub fn get_all_data(_lobby: LobbyID) -> Result<Vec<(String, String)>, SteamError> {
        disabled()
    }
    pub fn get_members(_lobby: LobbyID) -> Result<Vec<SteamID>, SteamError> {
        disabled()
    }
    pub fn get_owner(_lobby: LobbyID) -> Result<SteamID, SteamError> {
        disabled()
    }
    pub fn get_info(_lobby: LobbyID) -> Result<LobbyInfo, SteamError> {
        disabled()
    }
    pub fn set_joinable(_lobby: LobbyID, _joinable: bool) -> Result<(), SteamError> {
        disabled()
    }
    pub fn set_joinability(
        _lobby: LobbyID,
        _joinability: LobbyJoinability,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn send_chat(_lobby: LobbyID, _message: impl AsRef<[u8]>) -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_chat(_lobby: LobbyID, _chat_id: i32) -> Result<Vec<u8>, SteamError> {
        disabled()
    }
}

pub mod networking {
    use super::{DisabledEnum, DisabledSendType, SteamError, SteamID, disabled};

    pub type SendType = DisabledSendType;
    pub type P2PSessionState = DisabledEnum;

    pub fn is_p2p_session_accepted(_user: SteamID) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_p2p_session_closed(_user: SteamID) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn get_session_state(_user: SteamID) -> Result<Option<P2PSessionState>, SteamError> {
        disabled()
    }
    pub fn is_p2p_sent(
        _user: SteamID,
        _send_type: SendType,
        _data: &[u8],
    ) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_p2p_sent_on_channel(
        _user: SteamID,
        _send_type: SendType,
        _data: &[u8],
        _channel: i32,
    ) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn get_p2p_available() -> Result<Option<usize>, SteamError> {
        disabled()
    }
    pub fn get_p2p_available_on_channel(_channel: i32) -> Result<Option<usize>, SteamError> {
        disabled()
    }
    pub fn get_p2p_packet(_max_size: usize) -> Result<Option<(SteamID, Vec<u8>)>, SteamError> {
        disabled()
    }
    pub fn get_p2p_packet_from_channel(
        _max_size: usize,
        _channel: i32,
    ) -> Result<Option<(SteamID, Vec<u8>)>, SteamError> {
        disabled()
    }
}

pub mod leaderboards {
    use super::{DisabledEnum, DisabledHandle, SteamError, disabled};

    pub type LeaderboardID = DisabledHandle;
    pub type LeaderboardEntry = DisabledHandle;
    pub type LeaderboardDataRequest = DisabledEnum;
    pub type LeaderboardDisplayType = DisabledEnum;
    pub type LeaderboardSortMethod = DisabledEnum;
    pub type LeaderboardScoreUploaded = DisabledHandle;
    pub type UploadScoreMethod = DisabledEnum;

    pub fn find(
        _name: &str,
        cb: impl FnOnce(Result<Option<LeaderboardID>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn find_or_create(
        _name: &str,
        _sort_method: LeaderboardSortMethod,
        _display_type: LeaderboardDisplayType,
        cb: impl FnOnce(Result<Option<LeaderboardID>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn upload(
        _leaderboard: LeaderboardID,
        _method: UploadScoreMethod,
        _score: i32,
        _details: &[i32],
        cb: impl FnOnce(Result<Option<LeaderboardScoreUploaded>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn entries(
        _leaderboard: LeaderboardID,
        _request: LeaderboardDataRequest,
        _start: i32,
        _end: i32,
        _details_len: usize,
        cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
}

pub mod networking_messages {
    use super::{DisabledEnum, DisabledHandle, SteamError, SteamID, disabled};

    pub type SendFlags = DisabledEnum;
    pub type NetworkingMessage = DisabledHandle;
    pub type NetworkingIdentity = DisabledHandle;
    pub type NetworkingConnectionState = DisabledEnum;
    pub type NetConnectionInfo = DisabledHandle;
    pub type NetConnectionRealTimeInfo = DisabledHandle;

    pub fn identity_steam_id(_id: SteamID) -> NetworkingIdentity {
        DisabledHandle::default()
    }
    pub fn send_to_user(
        _user: NetworkingIdentity,
        _send_flags: SendFlags,
        _data: &[u8],
        _channel: i32,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_received(
        _channel: u32,
        _batch_size: usize,
    ) -> Result<Vec<NetworkingMessage>, SteamError> {
        disabled()
    }
    pub fn get_session_info(
        _user: NetworkingIdentity,
    ) -> Result<Option<NetConnectionInfo>, SteamError> {
        disabled()
    }
}

pub mod networking_sockets {
    use super::*;

    pub type ListenSocket = DisabledHandle;
    pub type NetConnection = DisabledHandle;
    pub type NetPollGroup = DisabledHandle;
    pub type NetworkingConfigEntry = DisabledHandle;
    pub type NetworkingIdentity = DisabledHandle;
    pub type NetworkingAvailability = DisabledEnum;
    pub type NetworkingAvailabilityError = DisabledEnum;

    pub fn listen_ip(
        _addr: SocketAddrV4,
        _options: &[NetworkingConfigEntry],
    ) -> Result<ListenSocket, SteamError> {
        disabled()
    }
    pub fn connect_ip(
        _addr: SocketAddrV4,
        _options: &[NetworkingConfigEntry],
    ) -> Result<NetConnection, SteamError> {
        disabled()
    }
    pub fn listen_p2p(
        _virtual_port: i32,
        _options: &[NetworkingConfigEntry],
    ) -> Result<ListenSocket, SteamError> {
        disabled()
    }
    pub fn connect_p2p(
        _identity: NetworkingIdentity,
        _virtual_port: i32,
        _options: &[NetworkingConfigEntry],
    ) -> Result<NetConnection, SteamError> {
        disabled()
    }
    pub fn init_authentication() -> Result<NetworkingAvailability, SteamError> {
        disabled()
    }
    pub fn auth_status() -> Result<NetworkingAvailability, SteamError> {
        disabled()
    }
}

pub mod networking_utils {
    use super::{DisabledEnum, DisabledHandle, SteamError, disabled};

    pub type NetworkingAvailability = DisabledEnum;
    pub type NetworkingAvailabilityError = DisabledEnum;
    pub type RelayNetworkStatus = DisabledHandle;

    pub fn init_relay_network_access() -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_relay_network_status() -> Result<NetworkingAvailability, SteamError> {
        disabled()
    }
    pub fn get_detailed_relay_network_status() -> Result<RelayNetworkStatus, SteamError> {
        disabled()
    }
}

pub mod remote_play {
    use super::{DisabledEnum, DisabledHandle, SteamError, disabled};

    pub type RemotePlaySessionID = DisabledHandle;
    pub type RemotePlaySession = DisabledHandle;
    pub type SteamDeviceFormFactor = DisabledEnum;

    pub fn get_sessions() -> Result<Vec<RemotePlaySession>, SteamError> {
        disabled()
    }
    pub fn get_session(_session: RemotePlaySessionID) -> Result<RemotePlaySession, SteamError> {
        disabled()
    }
}

pub mod screenshots {
    use super::{DisabledHandle, SteamError, disabled};

    pub type ScreenshotHandle = DisabledHandle;

    pub fn trigger() -> Result<(), SteamError> {
        disabled()
    }
    pub fn hook_screenshots(_hook: bool) -> Result<(), SteamError> {
        disabled()
    }
    pub fn add_to_library(
        _path: &str,
        _thumbnail: Option<&str>,
        _width: i32,
        _height: i32,
    ) -> Result<ScreenshotHandle, SteamError> {
        disabled()
    }
}

pub mod servers {
    use super::{DisabledHandle, Ipv4Addr, SteamError, disabled};

    pub type MatchmakingServers = DisabledHandle;
    pub type GameServerItem = DisabledHandle;
    pub type ServerListRequest = DisabledHandle;
    pub type PingCallbacks = DisabledHandle;
    pub type ServerRulesCallbacks = DisabledHandle;

    pub fn ping_server(
        _ip: Ipv4Addr,
        _port: u16,
        _callbacks: PingCallbacks,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn server_rules(
        _ip: Ipv4Addr,
        _port: u16,
        _callbacks: ServerRulesCallbacks,
    ) -> Result<(), SteamError> {
        disabled()
    }
}

pub mod stats {
    use super::{SteamError, disabled};

    pub fn achievement_unlocked(_id: &str) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn achievement_unlock_time(_id: &str) -> Result<(bool, u32), SteamError> {
        disabled()
    }
    pub fn achievement_percent(_id: &str) -> Result<f32, SteamError> {
        disabled()
    }
    pub fn achievement_names() -> Result<Option<Vec<String>>, SteamError> {
        disabled()
    }
    pub fn get_i32(_name: &str) -> Result<i32, SteamError> {
        disabled()
    }
    pub fn set_i32(_name: &str, _value: i32) -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_f32(_name: &str) -> Result<f32, SteamError> {
        disabled()
    }
    pub fn set_f32(_name: &str, _value: f32) -> Result<(), SteamError> {
        disabled()
    }
    pub fn global_i64(_name: &str) -> Result<i64, SteamError> {
        disabled()
    }
    pub fn global_f64(_name: &str) -> Result<f64, SteamError> {
        disabled()
    }
    pub fn store() -> Result<(), SteamError> {
        disabled()
    }
    pub fn reset_all(_achievements_too: bool) -> Result<(), SteamError> {
        disabled()
    }
}

pub mod timeline {
    use super::{DisabledEnum, Duration, SteamError, disabled};

    pub type TimelineGameMode = DisabledEnum;
    pub type TimelineEventClipPriority = DisabledEnum;

    pub fn set_game_mode(_mode: TimelineGameMode) -> Result<(), SteamError> {
        disabled()
    }
    pub fn set_state_description(
        _description: &str,
        _duration: Duration,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn clear_state_description(_duration: Duration) -> Result<(), SteamError> {
        disabled()
    }
    pub fn add_event(
        _icon: &str,
        _title: &str,
        _description: &str,
        _priority: TimelineEventClipPriority,
        _start_offset: Duration,
        _duration: Duration,
    ) -> Result<(), SteamError> {
        disabled()
    }
}

pub mod utils {
    use super::{AppID, DisabledEnum, SteamError, disabled};

    pub type NotificationPosition = DisabledEnum;
    pub type GamepadTextInputMode = DisabledEnum;
    pub type GamepadTextInputLineMode = DisabledEnum;

    pub fn get_app_id() -> Result<AppID, SteamError> {
        disabled()
    }
    pub fn get_ip_country() -> Result<String, SteamError> {
        disabled()
    }
    pub fn is_overlay_enabled() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn get_ui_language() -> Result<String, SteamError> {
        disabled()
    }
    pub fn get_server_real_time() -> Result<u32, SteamError> {
        disabled()
    }
    pub fn set_overlay_notification_position(
        _position: NotificationPosition,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn is_steam_deck() -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_big_picture() -> Result<bool, SteamError> {
        disabled()
    }
}

pub mod workshop {
    use super::*;

    pub type FileType = DisabledEnum;
    pub type ItemState = DisabledEnum;
    pub type InstallInfo = DisabledHandle;
    pub type QueryHandle = DisabledHandle;
    pub type QueryResult = DisabledHandle;
    pub type UGCQueryType = DisabledEnum;
    pub type UGCType = DisabledEnum;
    pub type AppIDs = DisabledHandle;

    pub fn suspend_downloads(_suspend: bool) -> Result<(), SteamError> {
        disabled()
    }
    pub fn subscribe(
        _file: WorkshopFileID,
        cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn unsubscribe(
        _file: WorkshopFileID,
        cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn get_subscribed(
        _include_locally_disabled: bool,
    ) -> Result<Vec<WorkshopFileID>, SteamError> {
        disabled()
    }
    pub fn get_state(_file: WorkshopFileID) -> Result<ItemState, SteamError> {
        disabled()
    }
    pub fn get_download_info(_file: WorkshopFileID) -> Result<Option<(u64, u64)>, SteamError> {
        disabled()
    }
    pub fn get_install_info(_file: WorkshopFileID) -> Result<Option<InstallInfo>, SteamError> {
        disabled()
    }
    pub fn is_download_started(
        _file: WorkshopFileID,
        _high_priority: bool,
    ) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn create(
        _app_id: AppID,
        _file_type: FileType,
        cb: impl FnOnce(Result<(WorkshopFileID, bool), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn get_query_item(_file: WorkshopFileID) -> Result<QueryHandle, SteamError> {
        disabled()
    }
}
