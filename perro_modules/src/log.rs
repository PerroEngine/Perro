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

#[cfg(test)]
mod tests {
    use super::{format_error, format_info, format_warn};

    #[test]
    fn format_info_without_color() {
        assert_eq!(format_info("hello", false), "[INFO] hello");
    }

    #[test]
    fn format_warn_without_color() {
        assert_eq!(format_warn("careful", false), "[WARN] careful");
    }

    #[test]
    fn format_error_without_color() {
        assert_eq!(format_error("boom", false), "[ERROR] boom");
    }

    #[test]
    fn format_info_with_color() {
        assert_eq!(
            format_info("hello", true),
            "\u{1b}[96m[INFO]\u{1b}[0m hello"
        );
    }

    #[test]
    fn format_warn_with_color() {
        assert_eq!(
            format_warn("careful", true),
            "\u{1b}[93m[WARN]\u{1b}[0m careful"
        );
    }

    #[test]
    fn format_error_with_color() {
        assert_eq!(
            format_error("boom", true),
            "\u{1b}[91m[ERROR]\u{1b}[0m boom"
        );
    }
}
