#[macro_export]
macro_rules! params {
    ($($value:expr),* $(,)?) => {
        &[$(::perro_variant::Variant::from($value)),*]
    };
}

#[macro_export]
macro_rules! variant {
    ($value:expr) => {
        ::perro_variant::Variant::from($value)
    };
}
