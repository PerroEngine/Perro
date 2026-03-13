#[macro_export]
macro_rules! smid {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! sid {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! var {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! func {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! method {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! signal {
    ($name:expr) => {
        $crate::SignalID::from_string($name)
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
