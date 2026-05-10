use crate::{
    app,
    error::SteamError,
    types::{AppID, WorkshopFileID},
};

pub type FileType = steamworks::FileType;
pub type ItemState = steamworks::ItemState;
pub type InstallInfo = steamworks::InstallInfo;
pub type QueryHandle = steamworks::QueryHandle;
pub type QueryResult = steamworks::QueryResult;
pub type UGCQueryType = steamworks::UGCQueryType;
pub type UGCType = steamworks::UGCType;
pub type AppIDs = steamworks::AppIDs;

pub fn suspend_downloads(suspend: bool) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().suspend_downloads(suspend);
        Ok(())
    })
}

pub fn subscribe(
    file: WorkshopFileID,
    cb: impl FnOnce(Result<(), steamworks::SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().subscribe_item(file.into(), cb);
        Ok(())
    })
}

pub fn unsubscribe(
    file: WorkshopFileID,
    cb: impl FnOnce(Result<(), steamworks::SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().unsubscribe_item(file.into(), cb);
        Ok(())
    })
}

pub fn get_subscribed(include_locally_disabled: bool) -> Result<Vec<WorkshopFileID>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .ugc()
            .subscribed_items(include_locally_disabled)
            .into_iter()
            .map(Into::into)
            .collect())
    })
}

pub fn get_state(file: WorkshopFileID) -> Result<ItemState, SteamError> {
    app::with_client(|client| Ok(client.ugc().item_state(file.into())))
}

pub fn get_download_info(file: WorkshopFileID) -> Result<Option<(u64, u64)>, SteamError> {
    app::with_client(|client| Ok(client.ugc().item_download_info(file.into())))
}

pub fn get_install_info(file: WorkshopFileID) -> Result<Option<InstallInfo>, SteamError> {
    app::with_client(|client| Ok(client.ugc().item_install_info(file.into())))
}

pub fn is_download_started(file: WorkshopFileID, high_priority: bool) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.ugc().download_item(file.into(), high_priority)))
}

pub fn create(
    app_id: AppID,
    file_type: FileType,
    cb: impl FnOnce(Result<(steamworks::PublishedFileId, bool), steamworks::SteamError>)
    + Send
    + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().create_item(app_id.into(), file_type, cb);
        Ok(())
    })
}

pub fn get_query_item(file: WorkshopFileID) -> Result<QueryHandle, SteamError> {
    app::with_client(|client| {
        client
            .ugc()
            .query_item(file.into())
            .map_err(|_| SteamError::CallFailed("ugc.query_item"))
    })
}
