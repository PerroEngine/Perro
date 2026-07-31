#[macro_export]
macro_rules! hash_str {
    ($value:expr) => {
        const { $crate::string_to_u64($value) }
    };
}

#[macro_export]
macro_rules! smid {
    ($name:literal) => {
        const { $crate::ScriptMemberID::from_string($name) }
    };
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! sid {
    ($name:literal) => {
        const { $crate::ScriptMemberID::from_string($name) }
    };
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
/// Creates a script member id for a variable/property name.
///
/// Signature:
/// - `var!(&str) -> ScriptMemberID`
///
/// Usage:
/// - `var!("health") -> ScriptMemberID`
///
/// Literal names hash at compile time (`const` block); expression names hash
/// at runtime.
macro_rules! var {
    ($name:literal) => {
        const { $crate::ScriptMemberID::from_string($name) }
    };
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
/// Creates a script member id for a callable function name.
///
/// Signature:
/// - `func!(&str) -> ScriptMemberID`
///
/// Usage:
/// - `func!("take_damage") -> ScriptMemberID`
///
/// Literal names hash at compile time (`const` block); expression names hash
/// at runtime.
macro_rules! func {
    ($name:literal) => {
        const { $crate::ScriptMemberID::from_string($name) }
    };
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
/// Creates a script member id for a callable method name.
///
/// Signature:
/// - `method!(&str) -> ScriptMemberID`
///
/// Usage:
/// - `method!("take_damage") -> ScriptMemberID`
///
/// Literal names hash at compile time (`const` block); expression names hash
/// at runtime.
macro_rules! method {
    ($name:literal) => {
        const { $crate::ScriptMemberID::from_string($name) }
    };
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! signal {
    ($name:literal) => {
        const { $crate::SignalID::from_string($name) }
    };
    ($name:expr) => {
        $crate::SignalID::from_string($name)
    };
}

#[macro_export]
macro_rules! timer {
    ($name:literal) => {
        const { $crate::TimerID::from_string($name) }
    };
    ($name:expr) => {
        $crate::TimerID::from_string($name)
    };
}

#[macro_export]
macro_rules! tag {
    ($name:literal) => {
        const { $crate::TagID::from_string($name) }
    };
    ($name:expr) => {
        $crate::TagID::from_string($name)
    };
}

#[macro_export]
macro_rules! tags {
    ($($name:literal),* $(,)?) => {{
        const __TAGS: &[$crate::NodeTag] = &[$($crate::NodeTag::borrowed($name)),*];
        __TAGS
    }};
    ($($name:expr),* $(,)?) => {
        &[$($crate::NodeTag::new($name)),*]
    };
}
