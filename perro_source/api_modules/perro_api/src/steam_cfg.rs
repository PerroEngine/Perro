/// Include tokens only when `perro_api/steamworks` is enabled.
/// Supports items, statements, and blocks: `is_steam! { ... }` or `is_steam!({ ... })`.
#[cfg(feature = "steamworks")]
#[macro_export]
macro_rules! is_steam {
    ($($body:tt)*) => { $($body)* };
}

/// Discard tokens when `perro_api/steamworks` is disabled.
#[cfg(not(feature = "steamworks"))]
#[macro_export]
macro_rules! is_steam {
    ($($body:tt)*) => {};
}

/// Include tokens only when `perro_api/steamworks` is disabled.
#[cfg(not(feature = "steamworks"))]
#[macro_export]
macro_rules! is_not_steam {
    ($($body:tt)*) => { $($body)* };
}

/// Discard tokens when `perro_api/steamworks` is enabled.
#[cfg(feature = "steamworks")]
#[macro_export]
macro_rules! is_not_steam {
    ($($body:tt)*) => {};
}
