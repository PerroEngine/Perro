use std::fmt::Display;
use std::io::{self, Write};

fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

const AQUA: &str = "96";
const YELLOW: &str = "93";
const RED: &str = "91";

pub fn print(message: impl Display) {
    let _ = writeln!(io::stdout(), "{message}");
}

pub fn info(message: impl Display) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let _ = writeln!(handle, "{}", format_info(message, colors_enabled()));
}

pub fn warn(message: impl Display) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();

    let _ = writeln!(handle, "{}", format_warn(message, colors_enabled()));
}

pub fn error(message: impl Display) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();

    let _ = writeln!(handle, "{}", format_error(message, colors_enabled()));
}

fn format_info(message: impl Display, with_color: bool) -> String {
    format_prefixed("INFO", AQUA, message, with_color)
}

fn format_warn(message: impl Display, with_color: bool) -> String {
    format_prefixed("WARN", YELLOW, message, with_color)
}

fn format_error(message: impl Display, with_color: bool) -> String {
    format_prefixed("ERROR", RED, message, with_color)
}

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
