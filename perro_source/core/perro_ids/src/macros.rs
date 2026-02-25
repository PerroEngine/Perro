#[macro_export]
macro_rules! smid {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! sid {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! var {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! func {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! method {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! signal {
    ($name:expr) => {
        ::perro_ids::SignalID::from_string($name)
    };
}

#[macro_export]
macro_rules! tag {
    ($name:expr) => {
        $crate::TagID::from_string($name)
    };
}

#[macro_export]
macro_rules! tags {
    ($($name:literal),* $(,)?) => {{
        const __TAGS: &[$crate::TagID] = &[$($crate::TagID::from_string($name)),*];
        __TAGS
    }};
    ($($name:expr),* $(,)?) => {
        &[$($crate::IntoTagID::into_tag_id($name)),*]
    };
}
