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
        Fallback,
        Actions,
    }

    impl SteamInputMode {
        pub const fn allows_action_reads(self) -> bool {
            matches!(self, Self::Fallback | Self::Actions)
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

    pub const FALLBACK_ACTION_SET: &str = "perro_gamepad";
    pub const FALLBACK_DIGITAL_ACTIONS: [&str; 18] = [
        "perro_bottom",
        "perro_right",
        "perro_left",
        "perro_top",
        "perro_dpad_up",
        "perro_dpad_down",
        "perro_dpad_left",
        "perro_dpad_right",
        "perro_start",
        "perro_select",
        "perro_home",
        "perro_capture",
        "perro_l1",
        "perro_r1",
        "perro_l2",
        "perro_r2",
        "perro_l3",
        "perro_r3",
    ];
    pub const FALLBACK_ANALOG_ACTIONS: [&str; 4] = [
        "perro_left_stick",
        "perro_right_stick",
        "perro_left_trigger",
        "perro_right_trigger",
    ];

    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct FallbackGamepad {
        pub handle: InputHandle,
        pub input_type: InputType,
        pub buttons: [bool; 18],
        pub axes: [f32; 6],
        pub motion: MotionData,
    }

    pub const fn fallback_eligible(_input_type: InputType, _native_gamepad_present: bool) -> bool {
        false
    }

    pub fn fallback_gamepads(
        _native_gamepad_present: bool,
    ) -> Result<Vec<FallbackGamepad>, SteamError> {
        disabled()
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
    use std::path::Path;

    pub const RESULTS_PER_PAGE: u32 = 50;

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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Visibility {
        Public,
        FriendsOnly,
        Private,
        Unlisted,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum QueryAppIDs {
        Creator(AppID),
        Consumer(AppID),
        Both { creator: AppID, consumer: AppID },
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ItemType {
        Items,
        ItemsMtx,
        ItemsReadyToUse,
        Collections,
        Artwork,
        Videos,
        Screenshots,
        AllGuides,
        WebGuides,
        IntegratedGuides,
        UsableInGame,
        ControllerBindings,
        GameManagedItems,
        All,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum QueryType {
        RankedByVote,
        RankedByPublicationDate,
        AcceptedForGameRankedByAcceptanceDate,
        RankedByTrend,
        FavoritedByFriendsRankedByPublicationDate,
        CreatedByFriendsRankedByPublicationDate,
        RankedByNumTimesReported,
        CreatedByFollowedUsersRankedByPublicationDate,
        NotYetRated,
        RankedByTotalVotesAsc,
        RankedByVotesUp,
        RankedByTextSearch,
        RankedByTotalUniqueSubscriptions,
        RankedByPlaytimeTrend,
        RankedByTotalPlaytime,
        RankedByAveragePlaytimeTrend,
        RankedByLifetimeAveragePlaytime,
        RankedByPlaytimeSessionsTrend,
        RankedByLifetimePlaytimeSessions,
        RankedByLastUpdatedDate,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum UserListOrder {
        CreationOrderAsc,
        CreationOrderDesc,
        TitleAsc,
        LastUpdatedDesc,
        SubscriptionDateDesc,
        VoteScoreDesc,
        ForModeration,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum UserList {
        Published,
        VotedOn,
        VotedUp,
        VotedDown,
        WillVoteLater,
        Favorited,
        Subscribed,
        UsedOrPlayed,
        Followed,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum StatisticType {
        Subscriptions,
        Favorites,
        Followers,
        UniqueSubscriptions,
        UniqueFavorites,
        UniqueFollowers,
        UniqueWebsiteViews,
        Reports,
        SecondsPlayed,
        PlaytimeSessions,
        Comments,
        SecondsPlayedDuringTimePeriod,
        PlaytimeSessionsDuringTimePeriod,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ContentDescriptor {
        NudityOrSexualContent,
        FrequentViolenceOrGore,
        AdultOnlySexualContent,
        GratuitousSexualContent,
        AnyMatureContent,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum UpdateStatus {
        Invalid,
        PreparingConfig,
        PreparingContent,
        UploadingContent,
        UploadingPreviewFile,
        CommittingChanges,
    }

    pub struct QueryItem {
        pub file: WorkshopFileID,
        pub creator_app_id: Option<AppID>,
        pub consumer_app_id: Option<AppID>,
        pub title: String,
        pub description: String,
        pub owner: SteamID,
        pub time_created: u32,
        pub time_updated: u32,
        pub time_added_to_user_list: u32,
        pub visibility: Visibility,
        pub banned: bool,
        pub accepted_for_use: bool,
        pub tags: Vec<String>,
        pub tags_truncated: bool,
        pub file_name: String,
        pub file_type: FileType,
        pub file_size: u32,
        pub url: String,
        pub preview_url: Option<String>,
        pub num_upvotes: u32,
        pub num_downvotes: u32,
        pub score: f32,
        pub children: Option<Vec<WorkshopFileID>>,
        pub key_value_tags: Vec<(String, String)>,
        pub metadata: Option<Vec<u8>>,
        pub content_descriptors: Vec<ContentDescriptor>,
        pub statistics: Vec<(StatisticType, u64)>,
    }

    pub struct QueryPage {
        pub items: Vec<QueryItem>,
        pub total_results: u32,
        pub was_cached: bool,
    }

    pub struct Update;

    impl Update {
        pub fn title(self, _title: &str) -> Self {
            self
        }
        pub fn description(self, _description: &str) -> Self {
            self
        }
        pub fn language(self, _language: &str) -> Self {
            self
        }
        pub fn preview_path(self, _path: impl AsRef<Path>) -> Self {
            self
        }
        pub fn content_path(self, _path: impl AsRef<Path>) -> Self {
            self
        }
        pub fn metadata(self, _metadata: &str) -> Self {
            self
        }
        pub fn visibility(self, _visibility: Visibility) -> Self {
            self
        }
        pub fn tags<S: AsRef<str>>(self, _tags: Vec<S>, _allow_admin_tags: bool) -> Self {
            self
        }
        pub fn add_key_value_tag(self, _key: &str, _value: &str) -> Self {
            self
        }
        pub fn remove_key_value_tag(self, _key: &str) -> Self {
            self
        }
        pub fn remove_all_key_value_tags(self) -> Self {
            self
        }
        pub fn add_content_descriptor(self, _descriptor: ContentDescriptor) -> Self {
            self
        }
        pub fn remove_content_descriptor(self, _descriptor: ContentDescriptor) -> Self {
            self
        }
        pub fn submit(
            self,
            _change_note: Option<&str>,
            cb: impl FnOnce(Result<CreateItemResult, SteamError>) + Send + 'static,
        ) -> UpdateWatch {
            cb(Err(SteamError::Disabled));
            UpdateWatch
        }
    }

    pub struct UpdateWatch;

    impl UpdateWatch {
        pub fn progress(&self) -> (UpdateStatus, u64, u64) {
            (UpdateStatus::Invalid, 0, 0)
        }
    }

    pub struct Query;

    impl Query {
        pub fn exclude_tag(self, _tag: &str) -> Self {
            self
        }
        pub fn require_tag(self, _tag: &str) -> Self {
            self
        }
        pub fn match_any_tag(self, _any: bool) -> Self {
            self
        }
        pub fn language(self, _language: &str) -> Self {
            self
        }
        pub fn allow_cached_response(self, _max_age_seconds: u32) -> Self {
            self
        }
        pub fn include_long_description(self, _include: bool) -> Self {
            self
        }
        pub fn include_children(self, _include: bool) -> Self {
            self
        }
        pub fn include_metadata(self, _include: bool) -> Self {
            self
        }
        pub fn include_additional_previews(self, _include: bool) -> Self {
            self
        }
        pub fn include_key_value_tags(self, _include: bool) -> Self {
            self
        }
        pub fn return_only_ids(self, _only_ids: bool) -> Self {
            self
        }
        pub fn return_total_only(self, _total_only: bool) -> Self {
            self
        }
        pub fn cloud_file_name_filter(self, _file_name: &str) -> Self {
            self
        }
        pub fn search_text(self, _text: &str) -> Self {
            self
        }
        pub fn ranked_by_trend_days(self, _days: u32) -> Self {
            self
        }
        pub fn require_key_value_tag(self, _key: &str, _value: &str) -> Self {
            self
        }
        pub fn fetch(self, cb: impl FnOnce(Result<QueryPage, SteamError>) + Send + 'static) {
            cb(Err(SteamError::Disabled));
        }
        pub fn fetch_total(self, cb: impl Fn(Result<u32, SteamError>) + Send + 'static) {
            cb(Err(SteamError::Disabled));
        }
        pub fn fetch_ids(
            self,
            cb: impl Fn(Result<Vec<WorkshopFileID>, SteamError>) + Send + 'static,
        ) {
            cb(Err(SteamError::Disabled));
        }
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
    pub fn download(_file: WorkshopFileID, _high_priority: bool) -> Result<bool, SteamError> {
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
    pub fn start_update(_app_id: AppID, _file: WorkshopFileID) -> Result<Update, SteamError> {
        disabled()
    }
    pub fn query_all(
        _query_type: QueryType,
        _item_type: ItemType,
        _app_ids: QueryAppIDs,
        _page: u32,
    ) -> Result<Query, SteamError> {
        disabled()
    }
    pub fn query_user(
        _account_id: u32,
        _list: UserList,
        _item_type: ItemType,
        _order: UserListOrder,
        _app_ids: QueryAppIDs,
        _page: u32,
    ) -> Result<Query, SteamError> {
        disabled()
    }
    pub fn query_items(_files: &[WorkshopFileID]) -> Result<Query, SteamError> {
        disabled()
    }
    pub fn query_item(_file: WorkshopFileID) -> Result<Query, SteamError> {
        disabled()
    }
    pub fn delete(
        _file: WorkshopFileID,
        cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn start_playtime_tracking(
        _files: &[WorkshopFileID],
        cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn stop_playtime_tracking(
        _files: &[WorkshopFileID],
        cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn stop_all_playtime_tracking(
        cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
    ) -> Result<(), SteamError> {
        cb(Err(SteamError::Disabled));
        disabled()
    }
    pub fn init_for_game_server(_workshop_depot: u32, _folder: &str) -> Result<bool, SteamError> {
        disabled()
    }
}
