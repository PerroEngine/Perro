#[macro_export]
macro_rules! params {
    ($($value:expr),* $(,)?) => {
        &[$($crate::Variant::from($value)),*]
    };
}

#[macro_export]
macro_rules! variant {
    ($value:expr) => {
        $crate::Variant::from($value)
    };
}
