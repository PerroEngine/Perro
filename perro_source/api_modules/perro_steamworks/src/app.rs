use crate::error::SteamError;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct SteamState {
    enabled: bool,
    app_id: Option<u32>,
    client: Option<steamworks::Client>,
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
    if !enabled {
        let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
        if state.client.is_none() {
            state.enabled = false;
            state.app_id = None;
        }
        return Ok(());
    }

    let app_id = app_id.ok_or(SteamError::MissingAppId)?;
    let mut state = state().lock().map_err(|_| SteamError::NotReady)?;
    if state.client.is_some() {
        if state.app_id == Some(app_id) {
            state.enabled = true;
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
    Ok(())
}

pub fn run_callbacks() {
    let client = state().lock().ok().and_then(|state| state.client.clone());
    if let Some(client) = client {
        client.run_callbacks();
    }
}

pub fn enabled() -> bool {
    state().lock().map(|state| state.enabled).unwrap_or(false)
}

pub fn ready() -> bool {
    state()
        .lock()
        .map(|state| state.client.is_some())
        .unwrap_or(false)
}

pub fn app_id() -> Option<u32> {
    state().lock().ok().and_then(|state| state.app_id)
}

pub(crate) fn with_client<T>(
    f: impl FnOnce(&steamworks::Client) -> Result<T, SteamError>,
) -> Result<T, SteamError> {
    let client = {
        let state = state().lock().map_err(|_| SteamError::NotReady)?;
        if !state.enabled {
            return Err(SteamError::Disabled);
        }
        state.client.clone().ok_or(SteamError::NotReady)?
    };
    f(&client)
}
