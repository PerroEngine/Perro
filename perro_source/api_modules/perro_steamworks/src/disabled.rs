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
        init_from_config_with_input(enabled, app_id, super::input::SteamInputMode::Off)
    }

    pub fn init_from_config_with_input(
        enabled: bool,
        app_id: Option<u32>,
        input_mode: super::input::SteamInputMode,
    ) -> Result<(), SteamError> {
        if enabled {
            if app_id.is_none() {
                return Err(SteamError::MissingAppId);
            }
            return Err(SteamError::Disabled);
        }
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        state.app_id = None;
        let _ = app_id;
        let _ = super::input::set_mode(input_mode);
        Ok(())
    }

    pub fn run_callbacks() -> Result<(), SteamError> {
        Ok(())
    }

    #[cfg(test)]
    pub fn is_enabled() -> Result<bool, SteamError> {
        Ok(false)
    }

    #[cfg(test)]
    pub fn is_ready() -> Result<bool, SteamError> {
        Ok(false)
    }

    #[cfg(test)]
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
    use super::{SteamError, SteamEvent, SteamEventQueueStats};
    use crate::event_queue::SteamEventQueue;
    use std::sync::{Mutex, OnceLock};

    fn queue() -> &'static Mutex<SteamEventQueue> {
        static QUEUE: OnceLock<Mutex<SteamEventQueue>> = OnceLock::new();
        QUEUE.get_or_init(|| Mutex::new(SteamEventQueue::new()))
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
            .map(|mut queue| queue.drain())
            .map_err(|_| SteamError::NotReady)
    }

    pub fn clear() -> Result<(), SteamError> {
        queue()
            .lock()
            .map(|mut queue| queue.clear())
            .map_err(|_| SteamError::NotReady)
    }

    pub fn queue_stats() -> Result<SteamEventQueueStats, SteamError> {
        queue()
            .lock()
            .map(|queue| queue.stats())
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
    use super::{SteamError, disabled};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FileInfo {
        pub name: String,
        pub size: u64,
    }

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
    pub fn get_avatar(
        _id: SteamID,
        _size: SteamAvatarSize,
    ) -> Result<Option<SteamAvatar>, SteamError> {
        disabled()
    }
    pub fn get_small_avatar(_id: SteamID) -> Result<Option<SteamAvatar>, SteamError> {
        disabled()
    }
    pub fn get_medium_avatar(_id: SteamID) -> Result<Option<SteamAvatar>, SteamError> {
        disabled()
    }
    pub fn get_large_avatar(_id: SteamID) -> Result<Option<SteamAvatar>, SteamError> {
        disabled()
    }
    pub fn request_user_information(_id: SteamID, _name_only: bool) -> Result<bool, SteamError> {
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

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum SteamInputMode {
        #[default]
        Off,
        Metadata,
        Actions,
    }

    impl SteamInputMode {
        pub const fn allows_action_reads(self) -> bool {
            matches!(self, Self::Actions)
        }
    }

    pub type InputType = DisabledEnum;
    pub type InputSourceMode = DisabledEnum;
    pub type InputActionOrigin = DisabledEnum;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct InputHandle(pub u64);

    impl InputHandle {
        pub const fn from_raw(raw: u64) -> Self {
            Self(raw)
        }

        pub const fn raw(self) -> u64 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ActionSetHandle(pub u64);

    impl ActionSetHandle {
        pub const fn raw(self) -> u64 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct DigitalActionHandle(pub u64);

    impl DigitalActionHandle {
        pub const fn raw(self) -> u64 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct AnalogActionHandle(pub u64);

    impl AnalogActionHandle {
        pub const fn raw(self) -> u64 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct InputController {
        pub handle: InputHandle,
        pub input_type: InputType,
        pub is_joycon: bool,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct DigitalActionData {
        pub state: bool,
        pub active: bool,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct AnalogActionData {
        pub mode: InputSourceMode,
        pub x: f32,
        pub y: f32,
        pub active: bool,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct MotionData {
        pub rot_quat: [f32; 4],
        pub pos_accel: [f32; 3],
        pub rot_vel: [f32; 3],
    }

    pub(crate) fn set_mode(_mode: SteamInputMode) -> Result<(), SteamError> {
        Ok(())
    }

    pub fn mode() -> Result<SteamInputMode, SteamError> {
        Ok(SteamInputMode::Off)
    }

    pub fn is_init(_explicitly_call_run_frame: bool) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn run_frame() -> Result<(), SteamError> {
        disabled()
    }
    pub fn get_connected_controllers() -> Result<Vec<InputHandle>, SteamError> {
        disabled()
    }
    pub fn get_controller_info() -> Result<Vec<InputController>, SteamError> {
        disabled()
    }
    pub const fn input_type_is_joycon(_input_type: InputType) -> bool {
        false
    }
    pub fn input_type(_handle: InputHandle) -> Result<InputType, SteamError> {
        disabled()
    }
    pub fn is_action_manifest_set(_path: &str) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn is_binding_panel_shown(_input_handle: InputHandle) -> Result<bool, SteamError> {
        disabled()
    }
    pub fn action_set_handle(_name: &str) -> Result<ActionSetHandle, SteamError> {
        disabled()
    }
    pub fn activate_action_set(
        _input_handle: InputHandle,
        _action_set: ActionSetHandle,
    ) -> Result<(), SteamError> {
        disabled()
    }
    pub fn digital_action_handle(_name: &str) -> Result<DigitalActionHandle, SteamError> {
        disabled()
    }
    pub fn analog_action_handle(_name: &str) -> Result<AnalogActionHandle, SteamError> {
        disabled()
    }
    pub fn digital_action_data(
        _input_handle: InputHandle,
        _action: DigitalActionHandle,
    ) -> Result<DigitalActionData, SteamError> {
        disabled()
    }
    pub fn analog_action_data(
        _input_handle: InputHandle,
        _action: AnalogActionHandle,
    ) -> Result<AnalogActionData, SteamError> {
        disabled()
    }
    pub fn digital_action_origins(
        _input_handle: InputHandle,
        _action_set: ActionSetHandle,
        _action: DigitalActionHandle,
    ) -> Result<Vec<InputActionOrigin>, SteamError> {
        disabled()
    }
    pub fn analog_action_origins(
        _input_handle: InputHandle,
        _action_set: ActionSetHandle,
        _action: AnalogActionHandle,
    ) -> Result<Vec<InputActionOrigin>, SteamError> {
        disabled()
    }
    pub fn glyph_for_action_origin(_origin: InputActionOrigin) -> Result<String, SteamError> {
        disabled()
    }
    pub fn string_for_action_origin(_origin: InputActionOrigin) -> Result<String, SteamError> {
        disabled()
    }
    pub fn motion_data(_input_handle: InputHandle) -> Result<MotionData, SteamError> {
        disabled()
    }
    pub fn shutdown() -> Result<(), SteamError> {
        disabled()
    }
}

pub mod auth {
    use super::{AppID, SteamError, SteamID, disabled};

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct AuthTicket(pub u64);

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum AuthSessionError {
        InvalidTicket,
        DuplicateRequest,
        InvalidVersion,
        GameMismatch,
        ExpiredTicket,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum UserHasLicense {
        HasLicense,
        DoesNotHaveLicense,
        NoAuth,
    }

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
    ) -> Result<Result<(), AuthSessionError>, SteamError> {
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
    use super::{SteamError, SteamID, disabled};
    use std::net::Ipv4Addr;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SendType {
        Unreliable,
        UnreliableNoDelay,
        Reliable,
        ReliableWithBuffering,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum P2PSessionError {
        None,
        NoRightsToApp,
        Timeout,
        Unknown(u8),
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct P2PSessionState {
        pub connection_active: bool,
        pub connecting: bool,
        pub error: P2PSessionError,
        pub using_relay: bool,
        pub bytes_queued_for_send: i32,
        pub packets_queued_for_send: i32,
        pub remote_ip: Option<Ipv4Addr>,
        pub remote_port: Option<u16>,
    }

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
    use super::{SteamError, SteamID, disabled};

    #[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct LeaderboardID(pub u64);

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct LeaderboardEntry {
        pub user: SteamID,
        pub global_rank: i32,
        pub score: i32,
        pub details: Vec<i32>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct LeaderboardScoreUpload {
        pub score: i32,
        pub changed: bool,
        pub global_rank_new: i32,
        pub global_rank_previous: i32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum LeaderboardSort {
        Ascending,
        Descending,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum LeaderboardDisplay {
        Numeric,
        TimeSeconds,
        TimeMilliseconds,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum LeaderboardUploadMode {
        KeepBest,
        Force,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum LeaderboardEntryScope {
        Global,
        AroundUser,
        Friends,
    }

    pub fn find(
        _name: &str,
        cb: impl FnOnce(Result<Option<LeaderboardID>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn find_or_create(
        _name: &str,
        _sort: LeaderboardSort,
        _display: LeaderboardDisplay,
        cb: impl FnOnce(Result<Option<LeaderboardID>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn upload_score(
        _leaderboard: &LeaderboardID,
        _score: i32,
        cb: impl FnOnce(Result<Option<LeaderboardScoreUpload>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn force_upload_score(
        _leaderboard: &LeaderboardID,
        _score: i32,
        cb: impl FnOnce(Result<Option<LeaderboardScoreUpload>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn upload_score_with_details(
        _leaderboard: &LeaderboardID,
        _mode: LeaderboardUploadMode,
        _score: i32,
        _details: &[i32],
        cb: impl FnOnce(Result<Option<LeaderboardScoreUpload>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn entries_global(
        _leaderboard: &LeaderboardID,
        _start: usize,
        _end: usize,
        cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn entries_around_user(
        _leaderboard: &LeaderboardID,
        _start: usize,
        _end: usize,
        cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn entries_friends(
        _leaderboard: &LeaderboardID,
        cb: impl FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn entries(
        _leaderboard: &LeaderboardID,
        _scope: LeaderboardEntryScope,
        _start: usize,
        _end: usize,
        _max_details_len: usize,
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FileType {
        Community,
        Microtransaction,
        Collection,
        Art,
        Video,
        Screenshot,
        Game,
        Software,
        Concept,
        WebGuide,
        IntegratedGuide,
        Merch,
        ControllerBinding,
        SteamworksAccessInvite,
        SteamVideo,
        GameManagedItem,
        Clip,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct ItemState {
        pub subscribed: bool,
        pub legacy_item: bool,
        pub installed: bool,
        pub needs_update: bool,
        pub downloading: bool,
        pub download_pending: bool,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct InstallInfo {
        pub folder: String,
        pub size_on_disk: u64,
        pub timestamp: u32,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CreateItemResult {
        pub file: WorkshopFileID,
        pub accepted_legal_agreement: bool,
    }

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
        cb: impl FnOnce(Result<CreateItemResult, SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
}
