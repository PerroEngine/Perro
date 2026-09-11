use perro_api::prelude::*;

is_steam! { const STEAM: bool = true; }
is_not_steam! { const STEAM: bool = false; }

#[test]
fn selects_items_and_statements() {
    assert_eq!(STEAM, cfg!(feature = "steamworks"));
    let mut hits = Vec::new();
    is_steam! { hits.push(true); }
    is_not_steam! { hits.push(false); }
    is_steam!({
        hits.push(true);
    });
    is_not_steam!({
        hits.push(false);
    });
    assert_eq!(hits, [STEAM, STEAM]);
}

#[cfg(not(feature = "steamworks"))]
is_steam! { this need not resolve or parse as Rust }
#[cfg(feature = "steamworks")]
is_not_steam! { this need not resolve or parse as Rust }

#[cfg(not(feature = "steamworks"))]
#[test]
fn steam_calls_return_disabled() {
    assert_eq!(
        steam_ach_unlock!("ACH_TEST"),
        Err(steam::SteamError::Disabled)
    );
    assert_eq!(
        steam_stat_get_i32!("wins"),
        Err(steam::SteamError::Disabled)
    );
}
