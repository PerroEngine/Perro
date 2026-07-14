use crate::{SteamError, SteamID};
use std::{
    ffi::CString,
    net::Ipv4Addr,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug)]
pub struct GameServerConfig {
    pub app_id: u32,
    pub ip: Ipv4Addr,
    pub game_port: u16,
    pub query_port: u16,
    pub version: String,
    pub server_name: String,
    pub product: String,
    pub game_description: String,
    pub max_players: i32,
    pub listed: bool,
    pub secure: bool,
    pub login_token: Option<String>,
}

impl GameServerConfig {
    pub fn from_env(app_id: u32, project_name: &str, version: Option<&str>) -> Self {
        fn env_num<T: std::str::FromStr>(name: &str, fallback: T) -> T {
            std::env::var(name)
                .ok()
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(fallback)
        }
        Self {
            app_id,
            ip: std::env::var("PERRO_STEAM_SERVER_IP")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(Ipv4Addr::UNSPECIFIED),
            game_port: env_num("PERRO_STEAM_GAME_PORT", 27015),
            query_port: env_num("PERRO_STEAM_QUERY_PORT", 27016),
            version: version.unwrap_or("0.1.0").to_string(),
            server_name: std::env::var("PERRO_STEAM_SERVER_NAME")
                .unwrap_or_else(|_| project_name.to_string()),
            product: std::env::var("PERRO_STEAM_PRODUCT").unwrap_or_else(|_| app_id.to_string()),
            game_description: std::env::var("PERRO_STEAM_GAME_DESCRIPTION")
                .unwrap_or_else(|_| project_name.to_string()),
            max_players: env_num("PERRO_STEAM_MAX_PLAYERS", 64),
            listed: std::env::var("PERRO_STEAM_LISTED")
                .map_or(true, |raw| !matches!(raw.as_str(), "0" | "false" | "off")),
            secure: std::env::var("PERRO_STEAM_SECURE")
                .map_or(true, |raw| !matches!(raw.as_str(), "0" | "false" | "off")),
            login_token: std::env::var("PERRO_STEAM_GSLT")
                .ok()
                .filter(|token| !token.is_empty()),
        }
    }
}

struct State {
    app_id: u32,
    server: steamworks::Server,
}
unsafe impl Send for State {}

fn state() -> &'static Mutex<Option<State>> {
    static STATE: OnceLock<Mutex<Option<State>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub fn init(config: GameServerConfig) -> Result<(), SteamError> {
    let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
    if let Some(current) = state.as_ref() {
        return if current.app_id == config.app_id {
            Ok(())
        } else {
            Err(SteamError::AlreadyInitialized {
                current: current.app_id,
                requested: config.app_id,
            })
        };
    }
    // Steam reads this during game-server init; no Steam user/client login occurs.
    unsafe { std::env::set_var("SteamAppId", config.app_id.to_string()) };
    let mode = if config.secure {
        steamworks::ServerMode::AuthenticationAndSecure
    } else {
        steamworks::ServerMode::Authentication
    };
    let (server, _) = steamworks::Server::init(
        config.ip,
        config.game_port,
        config.query_port,
        mode,
        &config.version,
    )
    .map_err(|err| SteamError::InitFailed(err.to_string()))?;
    server.set_dedicated_server(true);
    server.set_product(&config.product);
    server.set_game_description(&config.game_description);
    server.set_server_name(&config.server_name);
    server.set_max_players(config.max_players);
    if let Some(token) = config.login_token.as_deref() {
        server.log_on(token);
    } else {
        server.log_on_anonymous();
    }
    server.set_advertise_server_active(config.listed);
    *state = Some(State {
        app_id: config.app_id,
        server,
    });
    Ok(())
}

pub fn is_ready() -> bool {
    state().lock().is_ok_and(|state| state.is_some())
}
pub(crate) fn is_ready_internal() -> bool {
    is_ready()
}

pub fn run_callbacks() -> Result<(), SteamError> {
    state()
        .lock()
        .map_err(|_| SteamError::NotReady)?
        .as_ref()
        .ok_or(SteamError::NotReady)?
        .server
        .process_callbacks(crate::events::enqueue_callback);
    Ok(())
}

pub fn server_id() -> Result<SteamID, SteamError> {
    with_server(|server| Ok(server.steam_id().into()))
}
pub fn begin_auth_session(user: SteamID, ticket: &[u8]) -> Result<(), SteamError> {
    with_server(|server| {
        server
            .begin_authentication_session(user.into(), ticket)
            .map_err(|err| SteamError::InitFailed(format!("auth session: {err:?}")))
    })
}
pub fn end_auth_session(user: SteamID) -> Result<(), SteamError> {
    with_server(|server| {
        server.end_authentication_session(user.into());
        Ok(())
    })
}

pub fn request_user_stats(user: SteamID) -> Result<(), SteamError> {
    let stats = stats()?;
    unsafe {
        steamworks_sys::SteamAPI_ISteamGameServerStats_RequestUserStats(stats, user.get_id())
    };
    Ok(())
}
pub fn set_user_achievement(user: SteamID, name: &str) -> Result<(), SteamError> {
    let name = cstr(name)?;
    let ok = unsafe {
        steamworks_sys::SteamAPI_ISteamGameServerStats_SetUserAchievement(
            stats()?,
            user.get_id(),
            name.as_ptr(),
        )
    };
    ok.then_some(()).ok_or(SteamError::CallFailed(
        "game_server_stats.set_user_achievement",
    ))
}
pub fn set_user_stat_i32(user: SteamID, name: &str, value: i32) -> Result<(), SteamError> {
    let name = cstr(name)?;
    let ok = unsafe {
        steamworks_sys::SteamAPI_ISteamGameServerStats_SetUserStatInt32(
            stats()?,
            user.get_id(),
            name.as_ptr(),
            value,
        )
    };
    ok.then_some(()).ok_or(SteamError::CallFailed(
        "game_server_stats.set_user_stat_i32",
    ))
}
pub fn set_user_stat_f32(user: SteamID, name: &str, value: f32) -> Result<(), SteamError> {
    let name = cstr(name)?;
    let ok = unsafe {
        steamworks_sys::SteamAPI_ISteamGameServerStats_SetUserStatFloat(
            stats()?,
            user.get_id(),
            name.as_ptr(),
            value,
        )
    };
    ok.then_some(()).ok_or(SteamError::CallFailed(
        "game_server_stats.set_user_stat_f32",
    ))
}
pub fn store_user_stats(user: SteamID) -> Result<(), SteamError> {
    unsafe {
        steamworks_sys::SteamAPI_ISteamGameServerStats_StoreUserStats(stats()?, user.get_id())
    };
    Ok(())
}

fn with_server<T>(
    f: impl FnOnce(&steamworks::Server) -> Result<T, SteamError>,
) -> Result<T, SteamError> {
    let state = state().lock().map_err(|_| SteamError::NotReady)?;
    f(&state.as_ref().ok_or(SteamError::NotReady)?.server)
}
fn stats() -> Result<*mut steamworks_sys::ISteamGameServerStats, SteamError> {
    if !is_ready() {
        return Err(SteamError::NotReady);
    }
    let ptr = unsafe { steamworks_sys::SteamAPI_SteamGameServerStats_v001() };
    (!ptr.is_null()).then_some(ptr).ok_or(SteamError::NotReady)
}
fn cstr(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::CallFailed("string contains NUL"))
}
