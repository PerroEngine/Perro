use crate::types::{
    AppID, FriendInfo, FriendListKind, LobbyID, OverlayDialog, RichPresenceKey, SteamAvatar,
    SteamAvatarSize, SteamID, StoreOverlayAction, UserOverlayDialog,
};
use crate::{app, error::SteamError};

pub fn get_list() -> Result<Vec<FriendInfo>, SteamError> {
    get_list_by(FriendListKind::Friends)
}

pub fn get_list_by(kind: FriendListKind) -> Result<Vec<FriendInfo>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .friends()
            .get_friends(kind.into())
            .into_iter()
            .map(friend_info)
            .collect())
    })
}

pub fn get(id: SteamID) -> Result<FriendInfo, SteamError> {
    app::with_client(|client| Ok(friend_info(client.friends().get_friend(id.into()))))
}

pub fn get_avatar(id: SteamID, size: SteamAvatarSize) -> Result<Option<SteamAvatar>, SteamError> {
    app::with_client(|client| {
        let friend = client.friends().get_friend(id.into());
        Ok(friend_avatar(friend, size))
    })
}

pub fn get_small_avatar(id: SteamID) -> Result<Option<SteamAvatar>, SteamError> {
    get_avatar(id, SteamAvatarSize::Small)
}

pub fn get_medium_avatar(id: SteamID) -> Result<Option<SteamAvatar>, SteamError> {
    get_avatar(id, SteamAvatarSize::Medium)
}

pub fn get_large_avatar(id: SteamID) -> Result<Option<SteamAvatar>, SteamError> {
    get_avatar(id, SteamAvatarSize::Large)
}

pub fn request_user_information(id: SteamID, name_only: bool) -> Result<bool, SteamError> {
    app::with_client(|client| {
        Ok(client
            .friends()
            .request_user_information(id.into(), name_only))
    })
}

pub fn get_rich_presence<'a>(
    id: SteamID,
    key: impl Into<RichPresenceKey<'a>>,
) -> Result<Option<String>, SteamError> {
    app::with_client(|client| {
        Ok(client
            .friends()
            .get_friend(id.into())
            .rich_presence(key.into().as_str()))
    })
}

pub fn set_rich_presence<'a>(
    key: impl Into<RichPresenceKey<'a>>,
    value: &str,
) -> Result<(), SteamError> {
    app::with_client(|client| {
        if client
            .friends()
            .set_rich_presence(key.into().as_str(), Some(value))
        {
            Ok(())
        } else {
            Err(SteamError::CallFailed("friends.set_rich_presence"))
        }
    })
}

pub fn clear_rich_presence() -> Result<(), SteamError> {
    app::with_client(|client| {
        client.friends().clear_rich_presence();
        Ok(())
    })
}

pub fn open_overlay(dialog: OverlayDialog) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.friends().activate_game_overlay(dialog.as_str());
        Ok(())
    })
}

pub fn open_user_overlay(dialog: UserOverlayDialog, user: SteamID) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .friends()
            .activate_game_overlay_to_user(dialog.as_str(), user.into());
        Ok(())
    })
}

pub fn open_store(app_id: AppID, action: StoreOverlayAction) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .friends()
            .activate_game_overlay_to_store(app_id.into(), action.into());
        Ok(())
    })
}

pub fn open_web_page(url: &str) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.friends().activate_game_overlay_to_web_page(url);
        Ok(())
    })
}

pub fn open_invite_dialog(lobby: LobbyID) -> Result<(), SteamError> {
    app::with_client(|client| {
        client.friends().activate_invite_dialog(lobby.into());
        Ok(())
    })
}

pub fn invite_user_to_game(user: SteamID, connect: &str) -> Result<(), SteamError> {
    app::with_client(|client| {
        client
            .friends()
            .get_friend(user.into())
            .invite_user_to_game(connect);
        Ok(())
    })
}

fn friend_info(friend: steamworks::Friend) -> FriendInfo {
    FriendInfo {
        id: friend.id().into(),
        name: friend.name(),
        nickname: friend.nick_name(),
        state: friend.state().into(),
        game: friend.game_played().map(Into::into),
    }
}

fn friend_avatar(friend: steamworks::Friend, size: SteamAvatarSize) -> Option<SteamAvatar> {
    let rgba = match size {
        SteamAvatarSize::Small => friend.small_avatar(),
        SteamAvatarSize::Medium => friend.medium_avatar(),
        SteamAvatarSize::Large => friend.large_avatar(),
    }?;
    Some(SteamAvatar::new(size, rgba))
}
