use crate::{app, error::SteamError};

pub type InputType = steamworks::InputType;

pub fn is_init(explicitly_call_run_frame: bool) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.input().init(explicitly_call_run_frame)))
}

pub fn run_frame() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.input().run_frame();
        Ok(())
    })
}

pub fn get_connected_controllers() -> Result<Vec<u64>, SteamError> {
    app::with_client(|client| Ok(client.input().get_connected_controllers()))
}

pub fn is_action_manifest_set(path: &str) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.input().set_input_action_manifest_file_path(path)))
}

pub fn is_binding_panel_shown(input_handle: u64) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.input().show_binding_panel(input_handle)))
}

pub fn shutdown() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.input().shutdown();
        Ok(())
    })
}
