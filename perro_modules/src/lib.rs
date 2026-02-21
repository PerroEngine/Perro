pub mod file;
pub mod json;
pub mod log;

#[macro_export]
macro_rules! log_print {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log::print(format_args!($fmt $(, $arg)*))
    };
    ($message:expr) => {
        $crate::log::print($message)
    };
}

#[macro_export]
macro_rules! log_info {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log::info(format_args!($fmt $(, $arg)*))
    };
    ($message:expr) => {
        $crate::log::info($message)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log::warn(format_args!($fmt $(, $arg)*))
    };
    ($message:expr) => {
        $crate::log::warn($message)
    };
}

#[macro_export]
macro_rules! log_error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log::error(format_args!($fmt $(, $arg)*))
    };
    ($message:expr) => {
        $crate::log::error($message)
    };
}

pub mod prelude {
    pub use crate::file as FileMod;
    pub use crate::json as JSONMod;
    pub use crate::log as LogMod;
    pub use crate::{log_error, log_info, log_print, log_warn};
}

#[cfg(test)]
mod tests {
    #[test]
    fn log_macros_typecheck_and_forward() {
        let v = 42;
        crate::log_print!("print {v}");
        crate::log_info!("info {v}");
        crate::log_warn!("warn {v}");
        crate::log_error!("error {v}");
    }
}
