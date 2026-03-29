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
/// Creates a script member id for a variable/property name.
///
/// Usage:
/// - `var!("health") -> ScriptMemberID`
///
/// Example:
/// - `let hp = get_var!(ctx, enemy_id, var!("health"));`
macro_rules! var {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
/// Creates a script member id for a callable function name.
///
/// Usage:
/// - `func!("take_damage") -> ScriptMemberID`
///
/// Example:
/// - `let _ = call_method!(ctx, enemy_id, func!("take_damage"), params![10_i32]);`
macro_rules! func {
    ($name:expr) => {
        $crate::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
/// Creates a script member id for a callable method name.
///
/// Usage:
/// - `method!("take_damage") -> ScriptMemberID`
///
/// Example:
/// - `let _ = call_method!(ctx, enemy_id, method!("take_damage"), params![10_i32]);`
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
