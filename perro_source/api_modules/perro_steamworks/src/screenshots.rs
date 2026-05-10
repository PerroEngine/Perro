use crate::{app, error::SteamError};
use std::path::Path;

pub type ScreenshotHandle = steamworks::screenshots::ScreenshotHandle;

pub fn trigger() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.screenshots().trigger_screenshot();
        Ok(())
    })
}

pub fn hook_screenshots(hook: bool) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.screenshots().hook_screenshots(hook);
        Ok(())
    })
}

pub fn add_to_library(
    path: &Path,
    thumbnail: Option<&Path>,
    width: i32,
    height: i32,
) -> Result<ScreenshotHandle, SteamError> {
    app::with_client(|client| {
        client
            .screenshots()
            .add_screenshot_to_library(path, thumbnail, width, height)
            .map_err(|_| SteamError::CallFailed("screenshots.add_to_library"))
    })
}
