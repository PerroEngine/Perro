use crate::{app, error::SteamError};
use std::io::{Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
}

impl From<steamworks::SteamFileInfo> for FileInfo {
    fn from(info: steamworks::SteamFileInfo) -> Self {
        Self {
            name: info.name,
            size: info.size,
        }
    }
}

pub fn set_enabled_for_app(enabled: bool) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.remote_storage().set_cloud_enabled_for_app(enabled);
        Ok(())
    })
}

pub fn is_enabled_for_app() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.remote_storage().is_cloud_enabled_for_app()))
}

pub fn is_enabled_for_account() -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.remote_storage().is_cloud_enabled_for_account()))
}

pub fn get_files() -> Result<Vec<FileInfo>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .remote_storage()
            .files()
            .into_iter()
            .map(Into::into)
            .collect())
    })
}

pub fn is_file_present(name: &str) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.remote_storage().file(name).exists()))
}

pub fn delete(name: &str) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.remote_storage().file(name).delete()))
}

pub fn get_file_bytes(name: &str) -> Result<Vec<u8>, SteamError> {
    app::with_client(|client| {
        let mut out = Vec::new();
        client
            .remote_storage()
            .file(name)
            .read()
            .read_to_end(&mut out)
            .map_err(|_| SteamError::CallFailed("remote_storage.read"))?;
        Ok(out)
    })
}

pub fn write(name: &str, bytes: &[u8]) -> Result<(), SteamError> {
    app::with_client(|client| {
        let mut writer = client.remote_storage().file(name).write();
        writer
            .write_all(bytes)
            .map_err(|_| SteamError::CallFailed("remote_storage.write"))
    })
}
