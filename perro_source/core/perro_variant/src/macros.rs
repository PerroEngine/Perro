#[macro_export]
/// Builds a borrowed `&[Variant]` from ordinary Rust values.
///
/// Signature:
/// - `params!(...) -> &[Variant]`
///
/// Usage:
/// - `params![expr1, expr2, ...] -> &[Variant]`
macro_rules! params {
    ($($value:expr),* $(,)?) => {
        &[$($crate::Variant::from($value)),*]
    };
}

#[macro_export]
/// Converts one value into a `Variant`.
///
/// Signature:
/// - `variant!(T) -> Variant` where `Variant: From<T>`
///
/// Usage:
/// - `variant!(expr) -> Variant`
macro_rules! variant {
    ($value:expr) => {
        $crate::Variant::from($value)
    };
}
