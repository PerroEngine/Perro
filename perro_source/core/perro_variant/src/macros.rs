#[macro_export]
/// Builds a borrowed `&[Variant]` from ordinary Rust values.
///
/// Usage:
/// - `params![expr1, expr2, ...] -> &[Variant]`
///
/// Example:
/// - `call_method!(ctx, enemy_id, method!("hit"), params![10_i32, "fire", true]);`
macro_rules! params {
    ($($value:expr),* $(,)?) => {
        &[$($crate::Variant::from($value)),*]
    };
}

#[macro_export]
/// Converts one value into a `Variant`.
///
/// Usage:
/// - `variant!(expr) -> Variant`
///
/// Example:
/// - `set_var!(ctx, enemy_id, var!("health"), variant!(100_i32));`
macro_rules! variant {
    ($value:expr) => {
        $crate::Variant::from($value)
    };
}
