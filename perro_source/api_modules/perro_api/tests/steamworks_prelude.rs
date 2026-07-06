#![cfg(feature = "steamworks")]

use perro_api::prelude::*;

#[test]
fn steam_achievement_macros_accept_single_multi_slice_and_vec() {
    let one = "ACH_ONE";
    assert_eq!(steam_ach_unlock!(one), Err(steam::SteamError::Disabled));

    assert_eq!(
        steam_ach_unlock!("ACH_ONE", "ACH_TWO"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_ach_unlock!("ACH_ONE", "ACH_TWO", "ACH_THREE"),
        Err(steam::SteamError::Disabled)
    );

    let slice = ["ACH_ONE", "ACH_TWO"];
    assert_eq!(steam_ach_unlock!(&slice), Err(steam::SteamError::Disabled));
    assert_eq!(
        steam_ach_unlock!(&slice[..]),
        Err(steam::SteamError::Disabled)
    );

    let vec = vec!["ACH_ONE".to_string(), "ACH_TWO".to_string()];
    assert_eq!(steam_ach_unlock!(&vec), Err(steam::SteamError::Disabled));
    assert_eq!(
        steam_ach_unlock!(&vec[..]),
        Err(steam::SteamError::Disabled)
    );

    assert_eq!(
        steam::achievements::unlock_many(["ACH_ONE", "ACH_TWO"]),
        Err(steam::SteamError::Disabled)
    );

    assert_eq!(steam_account_self_name!(), Err(steam::SteamError::Disabled));
    assert_eq!(steam_account_self_id!(), Err(steam::SteamError::Disabled));
    assert_eq!(
        steam_account_name!(steam::SteamID::from_id(1)),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(steam_friend_list!(), Err(steam::SteamError::Disabled));
    assert_eq!(
        steam_friend_avatar!(steam::SteamID::from_id(1), steam::SteamAvatarSize::Small),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_friend_avatar_small!(steam::SteamID::from_id(1)),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_friend_avatar_medium!(steam::SteamID::from_id(1)),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_friend_avatar_large!(steam::SteamID::from_id(1)),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(steam::SteamAvatarSize::Large.width(), 184);
    assert_eq!(steam::SteamAvatarSize::Large.height(), 184);
    assert_eq!(
        steam_rich_presence_set!(steam::RichPresenceKey::Status, "menu"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_lobby_create!(steam::LobbyType::FriendsOnly, 4),
        Err(steam::SteamError::Disabled)
    );

    let lobby = steam::LobbyID::from_id(1);
    assert_eq!(steam_lobby_join!(lobby), Err(steam::SteamError::Disabled));
    assert_eq!(steam_lobby_leave!(lobby), Err(steam::SteamError::Disabled));
    assert_eq!(
        steam_lobby_data_set!(lobby, "mode", "coop"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_lobby_chat!(lobby, "hi"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_app_dlc_installed!(steam::DLCID::from_id(1)),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(steam_app_subscribed!(), Err(steam::SteamError::Disabled));
    assert_eq!(
        steam_stat_get_i32!("wins"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_stat_set_i32!("wins", 1),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_cloud_read!("save.bin"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_cloud_write!("save.bin", &[1, 2, 3]),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_workshop_download!(steam::WorkshopFileID::from_id(1), true),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_p2p_send!(
            steam::SteamID::from_id(1),
            steam::networking::SendType::Reliable,
            &[1, 2, 3]
        ),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(steam_p2p_read!(1024), Err(steam::SteamError::Disabled));
    assert_eq!(steam_events!(), Ok(Vec::new()));
}
