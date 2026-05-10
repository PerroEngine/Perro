use crate::{
    app,
    error::SteamError,
    types::{AppID, WorkshopFileID},
};

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

impl From<FileType> for steamworks::FileType {
    fn from(file_type: FileType) -> Self {
        match file_type {
            FileType::Community => Self::Community,
            FileType::Microtransaction => Self::Microtransaction,
            FileType::Collection => Self::Collection,
            FileType::Art => Self::Art,
            FileType::Video => Self::Video,
            FileType::Screenshot => Self::Screenshot,
            FileType::Game => Self::Game,
            FileType::Software => Self::Software,
            FileType::Concept => Self::Concept,
            FileType::WebGuide => Self::WebGuide,
            FileType::IntegratedGuide => Self::IntegratedGuide,
            FileType::Merch => Self::Merch,
            FileType::ControllerBinding => Self::ControllerBinding,
            FileType::SteamworksAccessInvite => Self::SteamworksAccessInvite,
            FileType::SteamVideo => Self::SteamVideo,
            FileType::GameManagedItem => Self::GameManagedItem,
            FileType::Clip => Self::Clip,
        }
    }
}

impl From<steamworks::FileType> for FileType {
    fn from(file_type: steamworks::FileType) -> Self {
        match file_type {
            steamworks::FileType::Community => Self::Community,
            steamworks::FileType::Microtransaction => Self::Microtransaction,
            steamworks::FileType::Collection => Self::Collection,
            steamworks::FileType::Art => Self::Art,
            steamworks::FileType::Video => Self::Video,
            steamworks::FileType::Screenshot => Self::Screenshot,
            steamworks::FileType::Game => Self::Game,
            steamworks::FileType::Software => Self::Software,
            steamworks::FileType::Concept => Self::Concept,
            steamworks::FileType::WebGuide => Self::WebGuide,
            steamworks::FileType::IntegratedGuide => Self::IntegratedGuide,
            steamworks::FileType::Merch => Self::Merch,
            steamworks::FileType::ControllerBinding => Self::ControllerBinding,
            steamworks::FileType::SteamworksAccessInvite => Self::SteamworksAccessInvite,
            steamworks::FileType::SteamVideo => Self::SteamVideo,
            steamworks::FileType::GameManagedItem => Self::GameManagedItem,
            steamworks::FileType::Clip => Self::Clip,
        }
    }
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

impl From<steamworks::ItemState> for ItemState {
    fn from(state: steamworks::ItemState) -> Self {
        Self {
            subscribed: state.contains(steamworks::ItemState::SUBSCRIBED),
            legacy_item: state.contains(steamworks::ItemState::LEGACY_ITEM),
            installed: state.contains(steamworks::ItemState::INSTALLED),
            needs_update: state.contains(steamworks::ItemState::NEEDS_UPDATE),
            downloading: state.contains(steamworks::ItemState::DOWNLOADING),
            download_pending: state.contains(steamworks::ItemState::DOWNLOAD_PENDING),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallInfo {
    pub folder: String,
    pub size_on_disk: u64,
    pub timestamp: u32,
}

impl From<steamworks::InstallInfo> for InstallInfo {
    fn from(info: steamworks::InstallInfo) -> Self {
        Self {
            folder: info.folder,
            size_on_disk: info.size_on_disk,
            timestamp: info.timestamp,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateItemResult {
    pub file: WorkshopFileID,
    pub accepted_legal_agreement: bool,
}

pub fn suspend_downloads(suspend: bool) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().suspend_downloads(suspend);
        Ok(())
    })
}

pub fn subscribe(
    file: WorkshopFileID,
    cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().subscribe_item(file.into(), move |result| {
            cb(result.map_err(|_| SteamError::CallFailed("ugc.subscribe_item")));
        });
        Ok(())
    })
}

pub fn unsubscribe(
    file: WorkshopFileID,
    cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().unsubscribe_item(file.into(), move |result| {
            cb(result.map_err(|_| SteamError::CallFailed("ugc.unsubscribe_item")));
        });
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
    app::with_client(|client| Ok(client.ugc().item_state(file.into()).into()))
}

pub fn get_download_info(file: WorkshopFileID) -> Result<Option<(u64, u64)>, SteamError> {
    app::with_client(|client| Ok(client.ugc().item_download_info(file.into())))
}

pub fn get_install_info(file: WorkshopFileID) -> Result<Option<InstallInfo>, SteamError> {
    app::with_client(|client| Ok(client.ugc().item_install_info(file.into()).map(Into::into)))
}

pub fn is_download_started(file: WorkshopFileID, high_priority: bool) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.ugc().download_item(file.into(), high_priority)))
}

pub fn create(
    app_id: AppID,
    file_type: FileType,
    cb: impl FnOnce(Result<CreateItemResult, SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .ugc()
            .create_item(app_id.into(), file_type.into(), move |result| {
                cb(match result {
                    Ok((file, accepted_legal_agreement)) => Ok(CreateItemResult {
                        file: file.into(),
                        accepted_legal_agreement,
                    }),
                    Err(_) => Err(SteamError::CallFailed("ugc.create_item")),
                });
            });
        Ok(())
    })
}
