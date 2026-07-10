use std::process::Command;

fn perro(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_perro_cli"))
        .args(args)
        .output()
        .expect("run perro_cli")
}

#[test]
fn missing_value_before_switch_fails_before_command_work() {
    let output = perro(&["dev", "--path", "--release"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("missing value for flag `--path` in `dev`"));
}

#[test]
fn unknown_flag_has_clear_command_error() {
    let output = perro(&["clean", "--pth", "."]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("unknown flag `--pth` for `clean`"));
    assert!(stderr.contains("valid flags: --path"));
}

#[test]
fn command_help_keeps_success_exit() {
    let output = perro(&["dev", "--help"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success());
    assert!(stderr.contains("Usage:"));
}
