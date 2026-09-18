use crate::{error::SteamError, input::SteamInputMode};
use std::sync::{Mutex, OnceLock};

const STEAM_APP_ID_ENV: &str = "SteamAppId";

#[derive(Default)]
struct SteamState {
    enabled: bool,
    app_id: Option<u32>,
    client: Option<steamworks::Client>,
    stats_store_requested: bool,
    /// Set once a `SteamAppId` env init was tried. Env init is never
    /// retried per frame: each failed `SteamAPI_Init` leaks inside steam_api.
    env_init_attempted: bool,
}

fn state() -> &'static Mutex<SteamState> {
    static STATE: OnceLock<Mutex<SteamState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(SteamState::default()))
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    if let Ok(mut state) = state().lock() {
        *state = SteamState::default();
    }
}

pub fn init_from_config(enabled: bool, app_id: Option<u32>) -> Result<(), SteamError> {
    init_from_config_with_input(enabled, app_id, SteamInputMode::Fallback)
}

pub fn init_from_config_with_input(
    enabled: bool,
    app_id: Option<u32>,
    input_mode: SteamInputMode,
) -> Result<(), SteamError> {
    if !enabled {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        if state.client.is_none() {
            state.enabled = false;
            state.app_id = None;
        }
        crate::input::set_mode(SteamInputMode::Off)?;
        return Ok(());
    }

    let app_id = app_id.ok_or(SteamError::MissingAppId)?;
    {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        if state.client.is_some() {
            if state.app_id == Some(app_id) {
                state.enabled = true;
                drop(state);
                crate::input::init_for_mode(input_mode)?;
                return Ok(());
            }
            return Err(SteamError::AlreadyInitialized {
                current: state.app_id.unwrap_or_default(),
                requested: app_id,
            });
        }

        let client = match init_client(app_id) {
            Ok(client) => client,
            Err(err) => {
                // One attempt only: each failed init leaks inside steam_api.
                // Open the Steam client so the next launch connects.
                state.env_init_attempted = true;
                open_steam_client();
                return Err(err);
            }
        };
        state.enabled = true;
        state.app_id = Some(app_id);
        state.client = Some(client);
    }

    crate::input::init_for_mode(input_mode)?;
    Ok(())
}

/// Ask the OS to start the Steam client (no-op if already running). Fire and
/// forget: the game does not wait for it or reconnect this session.
fn open_steam_client() {
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("explorer")
        .arg("steam://open/main")
        .spawn();
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open")
        .arg("steam://open/main")
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let spawned = std::process::Command::new("xdg-open")
        .arg("steam://open/main")
        .spawn();
    #[cfg(not(any(unix, target_os = "windows")))]
    let spawned: std::io::Result<()> = Ok(());
    if let Err(err) = spawned {
        eprintln!("[steam] could not open Steam client: {err}");
    }
}

/// The one place a Steam client is created. `Client::init_app` sets the
/// `SteamAppId`/`SteamGameId` env vars; a failed attempt puts them back so the
/// env-init path does not see a stale id and retry.
///
/// Each failed `SteamAPI_InitFlat` leaks inside steam_api (~0.7 MB). Calling
/// `SteamAPI_Shutdown` afterwards does not free it (measured), so the only
/// guard is to never call it twice: boot tries once and never retries.
fn init_client(app_id: u32) -> Result<steamworks::Client, SteamError> {
    const ENV_KEYS: [&str; 2] = [STEAM_APP_ID_ENV, "SteamGameId"];
    let prior = ENV_KEYS.map(std::env::var_os);
    match steamworks::Client::init_app(app_id) {
        Ok(client) => Ok(client),
        Err(err) => {
            for (key, value) in ENV_KEYS.iter().zip(prior) {
                // SAFETY: steam init runs on the main thread during boot or
                // `run_callbacks`; engine code does not read these vars on
                // other threads.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
            Err(SteamError::InitFailed(err.to_string()))
        }
    }
}

pub fn run_callbacks() -> Result<(), SteamError> {
    if crate::game_server::is_ready_internal() {
        return crate::game_server::run_callbacks();
    }
    let client = {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        ensure_client_from_process_env(&mut state)?;
        state.client.clone()
    };
    if let Some(client) = client {
        client.process_callbacks(crate::events::enqueue_callback);
        flush_stats_store(&client)?;
    }
    Ok(())
}

pub fn shutdown() -> Result<(), SteamError> {
    if matches!(
        crate::input::mode(),
        Ok(mode) if mode != SteamInputMode::Off
    ) {
        let _ = crate::input::shutdown();
        let _ = crate::input::set_mode(SteamInputMode::Off);
    }
    let client = {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        state.enabled = false;
        state.app_id = None;
        state.stats_store_requested = false;
        state.client.take()
    };
    let _ = crate::events::clear();
    drop(client);
    Ok(())
}

pub(crate) fn request_stats_store() -> Result<(), SteamError> {
    state()
        .lock()
        .map(|mut state| {
            state.stats_store_requested = true;
        })
        .map_err(|_| SteamError::NotReady)
}

fn flush_stats_store(client: &steamworks::Client) -> Result<(), SteamError> {
    let should_store = {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        if !state.stats_store_requested {
            false
        } else {
            state.stats_store_requested = false;
            true
        }
    };
    if should_store {
        client
            .user_stats()
            .store_stats()
            .map_err(|_| SteamError::CallFailed("user_stats.store_stats"))?;
    }
    Ok(())
}

#[cfg(test)]
pub fn is_enabled() -> Result<bool, SteamError> {
    state()
        .lock()
        .map(|state| state.enabled)
        .map_err(|_| SteamError::NotReady)
}

/// True once a Steam client is live. Boot init is the only attempt.
pub fn is_ready() -> Result<bool, SteamError> {
    state()
        .lock()
        .map(|state| state.client.is_some())
        .map_err(|_| SteamError::NotReady)
}

#[cfg(test)]
pub fn get_app_id() -> Result<Option<u32>, SteamError> {
    state()
        .lock()
        .map(|state| state.app_id)
        .map_err(|_| SteamError::NotReady)
}

pub(crate) fn with_client<T>(
    f: impl FnOnce(&steamworks::Client) -> Result<T, SteamError>,
) -> Result<T, SteamError> {
    let client = {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        ensure_client_from_process_env(&mut state)?;
        if !state.enabled {
            return Err(SteamError::Disabled);
        }
        state.client.clone().ok_or(SteamError::NotReady)?
    };
    f(&client)
}

fn ensure_client_from_process_env(state: &mut SteamState) -> Result<(), SteamError> {
    if state.client.is_some() || state.enabled || state.env_init_attempted {
        return Ok(());
    }
    let Some(app_id) = std::env::var(STEAM_APP_ID_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
    else {
        return Ok(());
    };
    state.env_init_attempted = true;
    let client = init_client(app_id)?;
    state.enabled = true;
    state.app_id = Some(app_id);
    state.client = Some(client);
    Ok(())
}
