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
