use crate::{error::SteamError, input::SteamInputMode};
use std::sync::{Mutex, OnceLock};

const STEAM_APP_ID_ENV: &str = "SteamAppId";

#[derive(Default)]
struct SteamState {
    enabled: bool,
    app_id: Option<u32>,
    client: Option<steamworks::Client>,
    stats_store_requested: bool,
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
    init_from_config_with_input(enabled, app_id, SteamInputMode::Off)
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

        let client = steamworks::Client::init_app(app_id)
            .map_err(|err| SteamError::InitFailed(err.to_string()))?;
        state.enabled = true;
        state.app_id = Some(app_id);
        state.client = Some(client);
    }

    crate::input::init_for_mode(input_mode)?;
    Ok(())
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

#[cfg(test)]
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
    if state.client.is_some() || state.enabled {
        return Ok(());
    }
    let Some(app_id) = std::env::var(STEAM_APP_ID_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
    else {
        return Ok(());
    };
    let client = steamworks::Client::init_app(app_id)
        .map_err(|err| SteamError::InitFailed(err.to_string()))?;
    state.enabled = true;
    state.app_id = Some(app_id);
    state.client = Some(client);
    Ok(())
}
