use perro_api::prelude::*;

#[test]
fn steam_achievement_macros_accept_single_multi_slice_and_vec() {
    perro_api::steam::app::init_from_config(false, None).expect("disabled Steam init");

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
}
