#[cfg(not(perro_no_console))]
use std::collections::HashSet;
use std::fmt::Display;
#[cfg(not(perro_no_console))]
use std::io::{self, Write};
#[cfg(not(perro_no_console))]
use std::sync::{Mutex, OnceLock};

#[cfg(not(perro_no_console))]
type WarnSite = (&'static str, u32, u32);
#[cfg(not(perro_no_console))]
type WarnedSites = Mutex<HashSet<WarnSite>>;
#[cfg(not(perro_no_console))]
static WARNED_SITES: OnceLock<WarnedSites> = OnceLock::new();

pub trait OptionWarnExt<T> {
    /// Logs `message` when this option is `None`, then returns it unchanged.
    fn warn_none<M: Display>(self, message: M) -> Option<T>;

    /// Logs `message` once per source call site when this option is `None`.
    #[track_caller]
    fn warn_none_once<M: Display>(self, message: M) -> Option<T>;
}

impl<T> OptionWarnExt<T> for Option<T> {
    fn warn_none<M: Display>(self, message: M) -> Option<T> {
        if self.is_none() {
            warn(message);
        }
        self
    }

    #[track_caller]
    fn warn_none_once<M: Display>(self, message: M) -> Option<T> {
        if self.is_none() {
            warn_once(std::panic::Location::caller(), message);
        }
        self
    }
}

pub trait ResultWarnExt<T, E> {
    /// Logs `context` and the error when this result is `Err`, then returns it unchanged.
    fn warn_err<M: Display>(self, context: M) -> Result<T, E>
    where
        E: Display;

    /// Logs `context` and the error once per source call site when this result is `Err`.
    #[track_caller]
    fn warn_err_once<M: Display>(self, context: M) -> Result<T, E>
    where
        E: Display;
}

impl<T, E> ResultWarnExt<T, E> for Result<T, E> {
    fn warn_err<M: Display>(self, context: M) -> Result<T, E>
    where
        E: Display,
    {
        if let Err(error) = &self {
            warn(format_args!("{context}: {error}"));
        }
        self
    }

    #[track_caller]
    fn warn_err_once<M: Display>(self, context: M) -> Result<T, E>
    where
        E: Display,
    {
        if let Err(error) = &self {
            warn_once(
                std::panic::Location::caller(),
                format_args!("{context}: {error}"),
            );
        }
        self
    }
}

#[cfg(not(perro_no_console))]
fn warn_once(location: &'static std::panic::Location<'static>, message: impl Display) {
    let sites = WARNED_SITES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut sites = match sites.lock() {
        Ok(sites) => sites,
        Err(poisoned) => poisoned.into_inner(),
    };
    if sites.insert((location.file(), location.line(), location.column())) {
        drop(sites);
        warn(message);
    }
}

#[cfg(perro_no_console)]
fn warn_once(_location: &'static std::panic::Location<'static>, _message: impl Display) {}

#[cfg(not(perro_no_console))]
fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

#[cfg(not(perro_no_console))]
const AQUA: &str = "96";
#[cfg(not(perro_no_console))]
const YELLOW: &str = "93";
#[cfg(not(perro_no_console))]
const RED: &str = "91";

#[cfg(not(perro_no_console))]
pub fn print(message: impl Display) {
    let _ = writeln!(io::stdout(), "{message}");
}

#[cfg(perro_no_console)]
pub fn print(_message: impl Display) {}

#[cfg(not(perro_no_console))]
pub fn info(message: impl Display) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let _ = writeln!(handle, "{}", format_info(message, colors_enabled()));
}

#[cfg(perro_no_console)]
pub fn info(_message: impl Display) {}

#[cfg(not(perro_no_console))]
pub fn warn(message: impl Display) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();

    let _ = writeln!(handle, "{}", format_warn(message, colors_enabled()));
}

#[cfg(perro_no_console)]
pub fn warn(_message: impl Display) {}

#[cfg(not(perro_no_console))]
pub fn error(message: impl Display) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();

    let _ = writeln!(handle, "{}", format_error(message, colors_enabled()));
}

#[cfg(perro_no_console)]
pub fn error(_message: impl Display) {}

#[cfg(not(perro_no_console))]
fn format_info(message: impl Display, with_color: bool) -> String {
    format_prefixed("INFO", AQUA, message, with_color)
}

#[cfg(not(perro_no_console))]
fn format_warn(message: impl Display, with_color: bool) -> String {
    format_prefixed("WARN", YELLOW, message, with_color)
}

#[cfg(not(perro_no_console))]
fn format_error(message: impl Display, with_color: bool) -> String {
    format_prefixed("ERROR", RED, message, with_color)
}

#[cfg(not(perro_no_console))]
fn format_prefixed(
    level: &str,
    color_code: &str,
    message: impl Display,
    with_color: bool,
) -> String {
    if with_color {
        format!("\x1b[{color_code}m[{level}]\x1b[0m {message}")
    } else {
        format!("[{level}] {message}")
    }
}

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

#[cfg(test)]
#[path = "../tests/unit/log_tests.rs"]
mod tests;
