use super::{OptionWarnExt, ResultWarnExt, format_error, format_info, format_warn};

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

#[test]
fn warn_extensions_keep_values_unchanged() {
    assert_eq!(Some(7).warn_none("unused"), Some(7));
    assert_eq!(Ok::<_, &str>(9).warn_err("unused"), Ok(9));
}

#[test]
fn warn_extensions_keep_failure_flow() {
    assert_eq!(None::<u8>.warn_none_once("missing test value"), None);
    assert_eq!(
        Err::<u8, _>("test error").warn_err_once("test operation fail"),
        Err("test error")
    );
}
