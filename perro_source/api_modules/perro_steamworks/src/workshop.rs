use crate::{
    app,
    error::SteamError,
    types::{AppID, SteamID, WorkshopFileID},
};
use std::path::Path;

pub use steamworks::{
    UGCContentDescriptorID as ContentDescriptor, UGCQueryType as QueryType,
    UGCStatisticType as StatisticType, UGCType as ItemType, UpdateStatus, UserList, UserListOrder,
};

pub const RESULTS_PER_PAGE: u32 = steamworks::RESULTS_PER_PAGE;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    FriendsOnly,
    Private,
    Unlisted,
}

impl From<Visibility> for steamworks::PublishedFileVisibility {
    fn from(value: Visibility) -> Self {
        match value {
            Visibility::Public => Self::Public,
            Visibility::FriendsOnly => Self::FriendsOnly,
            Visibility::Private => Self::Private,
            Visibility::Unlisted => Self::Unlisted,
        }
    }
}

impl From<steamworks::PublishedFileVisibility> for Visibility {
    fn from(value: steamworks::PublishedFileVisibility) -> Self {
        match value {
            steamworks::PublishedFileVisibility::Public => Self::Public,
            steamworks::PublishedFileVisibility::FriendsOnly => Self::FriendsOnly,
            steamworks::PublishedFileVisibility::Private => Self::Private,
            steamworks::PublishedFileVisibility::Unlisted => Self::Unlisted,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryAppIDs {
    Creator(AppID),
    Consumer(AppID),
    Both { creator: AppID, consumer: AppID },
}

impl From<QueryAppIDs> for steamworks::AppIDs {
    fn from(value: QueryAppIDs) -> Self {
        match value {
            QueryAppIDs::Creator(id) => Self::CreatorAppId(id.into()),
            QueryAppIDs::Consumer(id) => Self::ConsumerAppId(id.into()),
            QueryAppIDs::Both { creator, consumer } => Self::Both {
                creator: creator.into(),
                consumer: consumer.into(),
            },
        }
    }
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

pub struct Update {
    inner: steamworks::UpdateHandle,
}

impl Update {
    pub fn title(mut self, title: &str) -> Self {
        self.inner = self.inner.title(title);
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.inner = self.inner.description(description);
        self
    }

    pub fn language(mut self, language: &str) -> Self {
        self.inner = self.inner.language(language);
        self
    }

    pub fn preview_path(mut self, path: impl AsRef<Path>) -> Self {
        self.inner = self.inner.preview_path(path.as_ref());
        self
    }

    pub fn content_path(mut self, path: impl AsRef<Path>) -> Self {
        self.inner = self.inner.content_path(path.as_ref());
        self
    }

    pub fn metadata(mut self, metadata: &str) -> Self {
        self.inner = self.inner.metadata(metadata);
        self
    }

    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.inner = self.inner.visibility(visibility.into());
        self
    }

    pub fn tags<S: AsRef<str>>(mut self, tags: Vec<S>, allow_admin_tags: bool) -> Self {
        self.inner = self.inner.tags(tags, allow_admin_tags);
        self
    }

    pub fn add_key_value_tag(mut self, key: &str, value: &str) -> Self {
        self.inner = self.inner.add_key_value_tag(key, value);
        self
    }

    pub fn remove_key_value_tag(mut self, key: &str) -> Self {
        self.inner = self.inner.remove_key_value_tag(key);
        self
    }

    pub fn remove_all_key_value_tags(mut self) -> Self {
        self.inner = self.inner.remove_all_key_value_tags();
        self
    }

    pub fn add_content_descriptor(mut self, descriptor: ContentDescriptor) -> Self {
        self.inner = self.inner.add_content_descriptor(descriptor);
        self
    }

    pub fn remove_content_descriptor(mut self, descriptor: ContentDescriptor) -> Self {
        self.inner = self.inner.remove_content_descriptor(descriptor);
        self
    }

    pub fn submit(
        self,
        change_note: Option<&str>,
        cb: impl FnOnce(Result<CreateItemResult, SteamError>) + Send + 'static,
    ) -> UpdateWatch {
        let inner = self.inner.submit(change_note, move |result| {
            cb(result
                .map(|(file, accepted_legal_agreement)| CreateItemResult {
                    file: file.into(),
                    accepted_legal_agreement,
                })
                .map_err(|_| SteamError::CallFailed("ugc.submit_item_update")));
        });
        UpdateWatch { inner }
    }
}

pub struct UpdateWatch {
    inner: steamworks::UpdateWatchHandle,
}

impl UpdateWatch {
    pub fn progress(&self) -> (UpdateStatus, u64, u64) {
        self.inner.progress()
    }
}

pub struct Query {
    inner: steamworks::QueryHandle,
}

impl Query {
    pub fn exclude_tag(mut self, tag: &str) -> Self {
        self.inner = self.inner.exclude_tag(tag);
        self
    }

    pub fn require_tag(mut self, tag: &str) -> Self {
        self.inner = self.inner.require_tag(tag);
        self
    }

    pub fn match_any_tag(mut self, any: bool) -> Self {
        self.inner = self.inner.any_required(any);
        self
    }

    pub fn language(mut self, language: &str) -> Self {
        self.inner = self.inner.language(language);
        self
    }

    pub fn allow_cached_response(mut self, max_age_seconds: u32) -> Self {
        self.inner = self.inner.allow_cached_response(max_age_seconds);
        self
    }

    pub fn include_long_description(mut self, include: bool) -> Self {
        self.inner = self.inner.include_long_desc(include);
        self
    }

    pub fn include_children(mut self, include: bool) -> Self {
        self.inner = self.inner.include_children(include);
        self
    }

    pub fn include_metadata(mut self, include: bool) -> Self {
        self.inner = self.inner.include_metadata(include);
        self
    }

    pub fn include_additional_previews(mut self, include: bool) -> Self {
        self.inner = self.inner.include_additional_previews(include);
        self
    }

    pub fn include_key_value_tags(mut self, include: bool) -> Self {
        self.inner = self.inner.include_key_value_tags(include);
        self
    }

    pub fn return_only_ids(mut self, only_ids: bool) -> Self {
        self.inner = self.inner.set_return_only_ids(only_ids);
        self
    }

    pub fn return_total_only(mut self, total_only: bool) -> Self {
        self.inner = self.inner.set_return_total_only(total_only);
        self
    }

    pub fn cloud_file_name_filter(mut self, file_name: &str) -> Self {
        self.inner = self.inner.set_cloud_file_name_filter(file_name);
        self
    }

    pub fn search_text(mut self, text: &str) -> Self {
        self.inner = self.inner.set_search_text(text);
        self
    }

    pub fn ranked_by_trend_days(mut self, days: u32) -> Self {
        self.inner = self.inner.set_ranked_by_trend_days(days);
        self
    }

    pub fn require_key_value_tag(mut self, key: &str, value: &str) -> Self {
        self.inner = self.inner.add_required_key_value_tag(key, value);
        self
    }

    pub fn fetch(self, cb: impl FnOnce(Result<QueryPage, SteamError>) + Send + 'static) {
        self.inner.fetch(move |result| {
            cb(result
                .map(query_page)
                .map_err(|_| SteamError::CallFailed("ugc.send_query")));
        });
    }

    pub fn fetch_total(self, cb: impl Fn(Result<u32, SteamError>) + Send + 'static) {
        self.inner.fetch_total(move |result| {
            cb(result.map_err(|_| SteamError::CallFailed("ugc.send_query_total")));
        });
    }

    pub fn fetch_ids(self, cb: impl Fn(Result<Vec<WorkshopFileID>, SteamError>) + Send + 'static) {
        self.inner.fetch_ids(move |result| {
            cb(result
                .map(|files| files.into_iter().map(Into::into).collect())
                .map_err(|_| SteamError::CallFailed("ugc.send_query_ids")));
        });
    }
}

const STATISTICS: [StatisticType; 13] = [
    StatisticType::Subscriptions,
    StatisticType::Favorites,
    StatisticType::Followers,
    StatisticType::UniqueSubscriptions,
    StatisticType::UniqueFavorites,
    StatisticType::UniqueFollowers,
    StatisticType::UniqueWebsiteViews,
    StatisticType::Reports,
    StatisticType::SecondsPlayed,
    StatisticType::PlaytimeSessions,
    StatisticType::Comments,
    StatisticType::SecondsPlayedDuringTimePeriod,
    StatisticType::PlaytimeSessionsDuringTimePeriod,
];

fn query_page(results: steamworks::QueryResults<'_>) -> QueryPage {
    let total_results = results.total_results();
    let was_cached = results.was_cached();
    let items = (0..results.returned_results())
        .filter_map(|index| {
            let item = results.get(index)?;
            let key_value_tags = (0..results.key_value_tags(index))
                .filter_map(|tag| results.get_key_value_tag(index, tag))
                .collect();
            let statistics = STATISTICS
                .into_iter()
                .filter_map(|stat| results.statistic(index, stat).map(|value| (stat, value)))
                .collect();
            Some(QueryItem {
                file: item.published_file_id.into(),
                creator_app_id: item.creator_app_id.map(Into::into),
                consumer_app_id: item.consumer_app_id.map(Into::into),
                title: item.title,
                description: item.description,
                owner: item.owner.into(),
                time_created: item.time_created,
                time_updated: item.time_updated,
                time_added_to_user_list: item.time_added_to_user_list,
                visibility: item.visibility.into(),
                banned: item.banned,
                accepted_for_use: item.accepted_for_use,
                tags: item.tags,
                tags_truncated: item.tags_truncated,
                file_name: item.file_name,
                file_type: item.file_type.into(),
                file_size: item.file_size,
                url: item.url,
                preview_url: results.preview_url(index),
                num_upvotes: item.num_upvotes,
                num_downvotes: item.num_downvotes,
                score: item.score,
                children: results
                    .get_children(index)
                    .map(|files| files.into_iter().map(Into::into).collect()),
                key_value_tags,
                metadata: results.get_metadata(index),
                content_descriptors: results.content_descriptor(index),
                statistics,
            })
        })
        .collect();
    QueryPage {
        items,
        total_results,
        was_cached,
    }
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

pub fn download(file: WorkshopFileID, high_priority: bool) -> Result<bool, SteamError> {
    is_download_started(file, high_priority)
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

pub fn start_update(app_id: AppID, file: WorkshopFileID) -> Result<Update, SteamError> {
    app::with_client(|client| {
        Ok(Update {
            inner: client.ugc().start_item_update(app_id.into(), file.into()),
        })
    })
}

pub fn query_all(
    query_type: QueryType,
    item_type: ItemType,
    app_ids: QueryAppIDs,
    page: u32,
) -> Result<Query, SteamError> {
    app::with_client(|client| {
        client
            .ugc()
            .query_all(query_type, item_type, app_ids.into(), page)
            .map(|inner| Query { inner })
            .map_err(|_| SteamError::CallFailed("ugc.query_all"))
    })
}

pub fn query_user(
    account_id: u32,
    list: UserList,
    item_type: ItemType,
    order: UserListOrder,
    app_ids: QueryAppIDs,
    page: u32,
) -> Result<Query, SteamError> {
    app::with_client(|client| {
        client
            .ugc()
            .query_user(
                steamworks::AccountId::from_raw(account_id),
                list,
                item_type,
                order,
                app_ids.into(),
                page,
            )
            .map(|inner| Query { inner })
            .map_err(|_| SteamError::CallFailed("ugc.query_user"))
    })
}

pub fn query_items(files: &[WorkshopFileID]) -> Result<Query, SteamError> {
    if files.is_empty() {
        return Err(SteamError::CallFailed("ugc.query_items.empty"));
    }
    app::with_client(|client| {
        client
            .ugc()
            .query_items(files.iter().copied().map(Into::into).collect())
            .map(|inner| Query { inner })
            .map_err(|_| SteamError::CallFailed("ugc.query_items"))
    })
}

pub fn query_item(file: WorkshopFileID) -> Result<Query, SteamError> {
    query_items(&[file])
}

pub fn delete(
    file: WorkshopFileID,
    cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.ugc().delete_item(file.into(), move |result| {
            cb(result.map_err(|_| SteamError::CallFailed("ugc.delete_item")));
        });
        Ok(())
    })
}

pub fn start_playtime_tracking(
    files: &[WorkshopFileID],
    cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        let files: Vec<_> = files.iter().copied().map(Into::into).collect();
        client.ugc().start_playtime_tracking(&files, move |result| {
            cb(result.map_err(|_| SteamError::CallFailed("ugc.start_playtime_tracking")));
        });
        Ok(())
    })
}

pub fn stop_playtime_tracking(
    files: &[WorkshopFileID],
    cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        let files: Vec<_> = files.iter().copied().map(Into::into).collect();
        client.ugc().stop_playtime_tracking(&files, move |result| {
            cb(result.map_err(|_| SteamError::CallFailed("ugc.stop_playtime_tracking")));
        });
        Ok(())
    })
}

pub fn stop_all_playtime_tracking(
    cb: impl FnOnce(Result<(), SteamError>) + Send + 'static,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .ugc()
            .stop_playtime_tracking_for_all_items(move |result| {
                cb(result.map_err(|_| {
                    SteamError::CallFailed("ugc.stop_playtime_tracking_for_all_items")
                }));
            });
        Ok(())
    })
}

pub fn init_for_game_server(workshop_depot: u32, folder: &str) -> Result<bool, SteamError> {
    app::with_client(|client| Ok(client.ugc().init_for_game_server(workshop_depot, folder)))
}
