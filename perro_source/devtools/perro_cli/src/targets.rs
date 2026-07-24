use crate::parse_flag_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DevOs {
    Windows,
    Linux,
    Macos,
}

impl DevOs {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }

    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "windows" | "win" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            "macos" | "mac" | "darwin" => Ok(Self::Macos),
            other => Err(format!(
                "invalid `--host {other}`. use `windows`, `linux`, or `macos`."
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::Linux => "Linux",
            Self::Macos => "macOS",
        }
    }
}

pub(crate) fn targets_command(args: &[String]) -> Result<(), String> {
    let host = parse_flag_value(args, "--host")
        .as_deref()
        .map(DevOs::parse)
        .transpose()?
        .unwrap_or_else(DevOs::current);
    print!("{}", target_support_report(host));
    Ok(())
}

fn target_support_report(host: DevOs) -> String {
    let mut out = format!(
        "Dev OS: {}\n\nStatus: READY = direct  SETUP = extra tools  NO = use other OS\n\n",
        host.label()
    );
    out.push_str("Target              Status  Need\n");
    out.push_str("------------------  ------  --------------------------------\n");
    match host {
        DevOs::Windows => {
            row(&mut out, "Windows x64", "READY", "-");
            row(&mut out, "Windows x86", "SETUP", "VS x86 C++ tools");
            row(&mut out, "Windows ARM64", "SETUP", "VS ARM64 C++ tools");
            row(&mut out, "Linux x64", "SETUP", "cross linker + sysroot");
            row(&mut out, "Linux x86", "SETUP", "cross linker + sysroot");
            row(&mut out, "Linux ARM64", "SETUP", "cross linker + sysroot");
            row(&mut out, "macOS Intel", "NO", "macOS + Xcode");
            row(&mut out, "macOS ARM64", "NO", "macOS + Xcode");
            row(&mut out, "macOS universal", "NO", "macOS + Xcode + lipo");
        }
        DevOs::Linux => {
            row(&mut out, "Windows x64", "SETUP", "MinGW GNU linker");
            row(&mut out, "Windows x86", "SETUP", "MinGW GNU linker");
            row(&mut out, "Windows ARM64", "SETUP", "LLVM/MinGW toolchain");
            row(&mut out, "Linux x64", "READY", "-");
            row(&mut out, "Linux x86", "SETUP", "x86 linker + sysroot");
            row(&mut out, "Linux ARM64", "SETUP", "ARM64 linker + sysroot");
            row(&mut out, "macOS Intel", "NO", "macOS + Xcode");
            row(&mut out, "macOS ARM64", "NO", "macOS + Xcode");
            row(&mut out, "macOS universal", "NO", "macOS + Xcode + lipo");
        }
        DevOs::Macos => {
            row(&mut out, "Windows x64", "SETUP", "MinGW GNU linker");
            row(&mut out, "Windows x86", "SETUP", "MinGW GNU linker");
            row(&mut out, "Windows ARM64", "SETUP", "LLVM/MinGW toolchain");
            row(&mut out, "Linux x64", "SETUP", "cross linker + sysroot");
            row(&mut out, "Linux x86", "SETUP", "cross linker + sysroot");
            row(&mut out, "Linux ARM64", "SETUP", "cross linker + sysroot");
            row(&mut out, "macOS Intel", "READY", "Xcode");
            row(&mut out, "macOS ARM64", "READY", "Xcode");
            row(&mut out, "macOS universal", "READY", "Xcode + lipo");
        }
    }
    row(&mut out, "Web", "READY", "wasm-bindgen");
    row(&mut out, "Android ARM64", "SETUP", "Android SDK + NDK");
    out
}

fn row(out: &mut String, target: &str, status: &str, need: &str) {
    out.push_str(&format!("{target:<18}  {status:<6}  {need}\n"));
}

#[cfg(test)]
mod tests {
    use super::{DevOs, target_support_report};

    #[test]
    fn windows_report_marks_mac_targets_unavailable() {
        let report = target_support_report(DevOs::Windows);
        assert!(report.contains("Windows x64         READY"));
        assert!(report.contains("Linux ARM64         SETUP"));
        assert!(report.contains("macOS universal     NO"));
    }

    #[test]
    fn mac_report_marks_universal_ready() {
        let report = target_support_report(DevOs::Macos);
        assert!(report.contains("macOS universal     READY"));
    }
}
