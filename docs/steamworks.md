# Steamworks

Use Steamworks from game scripts through the normal Perro API prelude.

```rust
use perro_api::prelude::*;
```

## Setup

Add Steam config to `project.toml`:

```toml
[steam]
enabled = true
app_id = 480
```

Perro reads this at startup.

When `enabled = true`, Perro initializes Steam with `app_id`.

Perro also runs Steam callbacks during each runtime update.

Game scripts do not need Steam init, shutdown, or callback code.

Use your real Steam app id for shipped games.

Use `480` only for local Steamworks testing.

When Steam is disabled, achievement calls return `steam::SteamError::Disabled`.

## Achievements

Achievement ids must match ids in Steamworks portal.

```rust
steam_ach_unlock!("ACH_FIRST_WIN")?;
steam_ach_clear!("ACH_FIRST_WIN")?;
```

```rust
steam::achievements::unlock("ACH_FIRST_WIN")?;
steam::achievements::clear("ACH_FIRST_WIN")?;
```

## Unlock Many

Use this when one game event earns multiple achievements.

```rust
steam_ach_unlock!("ACH_FIRST_WIN", "ACH_NO_DAMAGE")?;

let ids = ["ACH_FIRST_WIN", "ACH_NO_DAMAGE"];
steam_ach_unlock!(&ids)?;

steam::achievements::unlock_many(ids)?;
```

## API

| Use         | Function                                | Macro                                                              |
| ----------- | --------------------------------------- | ------------------------------------------------------------------ |
| Unlock one  | `steam::achievements::unlock(id)`       | `steam_ach_unlock!(id)`                                            |
| Clear one   | `steam::achievements::clear(id)`        | `steam_ach_clear!(id)`                                             |
| Unlock many | `steam::achievements::unlock_many(ids)` | `steam_ach_unlock!("ACH_A", "ACH_B")` or `steam_ach_unlock!(&ids)` |

## Errors

All Steam calls return `Result<(), steam::SteamError>`.

For gameplay code, treat Steam failure as non-fatal:

```rust
if let Err(err) = steam_ach_unlock!("ACH_FIRST_WIN") {
    log_warn!("Steam achievement failed: {err}");
}
```

Common errors:

| Error                       | Meaning                                                            |
| --------------------------- | ------------------------------------------------------------------ |
| `SteamError::Disabled`      | Steam is off in `project.toml`.                                    |
| `SteamError::NotReady`      | Steam did not initialize.                                          |
| `SteamError::CallFailed(_)` | Steam rejected the call. Check the achievement id and Steam setup. |
