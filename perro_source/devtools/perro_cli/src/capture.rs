use crate::{parse_flag_value, resolve_local_path};
use perro_capture::FrameRate;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_FPS: f64 = 60.0;
const DEFAULT_SUPERSAMPLE: u32 = 2;

pub(crate) fn capture_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|path| resolve_local_path(&path, cwd))
        .unwrap_or_else(|| cwd.to_path_buf())
        .canonicalize()
        .unwrap_or_else(|_| cwd.to_path_buf());
    if !project_dir.join("project.toml").is_file() {
        return Err(format!(
            "invalid capture project `{}`; pass --path to a project.toml dir",
            project_dir.display()
        ));
    }

    let mode = parse_choice(args, "--mode", &["offline", "realtime"], "offline")?;
    let source = parse_nonempty(args, "--source", "main")?;
    let fps = parse_rate(args, "--fps", FrameRate::integer(DEFAULT_FPS as u32))?;
    let duration = parse_positive(args, "--duration", 1.0)?;
    let frames = exact_frame_count(duration, fps)?;
    let supersample = parse_supersample(args)?;
    let format = parse_choice(
        args,
        "--format",
        &["png", "gif", "webm", "mp4", "webp"],
        "png",
    )?;
    let aspect = parse_aspect(args)?;
    let framing = parse_choice(
        args,
        "--framing",
        &["fit", "crop", "expand", "stretch"],
        "fit",
    )?;
    let width = parse_optional_u32(args, "--width")?;
    let height = parse_optional_u32(args, "--height")?;
    let output = parse_required_path(args, "--output", cwd)?;
    let output = if output.is_absolute() {
        output
    } else {
        cwd.join(output)
    };
    if args.iter().any(|arg| arg == "--headless") {
        return Err("capture needs a graphics runner; remove --headless".to_string());
    }

    let executable =
        std::env::current_exe().map_err(|err| format!("resolve perro_cli executable: {err}"))?;
    let project_arg = project_dir.to_string_lossy();
    let project_arg = project_arg
        .strip_prefix(r"\\?\")
        .unwrap_or(project_arg.as_ref())
        .to_owned();
    let mut dev_args = vec!["dev".to_string(), "--path".to_string(), project_arg];
    for flag in ["--scene", "--sim"] {
        if let Some(value) = parse_flag_value(args, flag) {
            dev_args.push(flag.to_string());
            dev_args.push(value);
        }
    }
    for flag in ["--release", "--demo", "--playtest"] {
        if args.iter().any(|arg| arg == flag) {
            dev_args.push(flag.to_string());
        }
    }

    let mut command = Command::new(executable);
    command
        .args(&dev_args)
        // Keep caller cwd: `dev --path` resolves project workspaces itself.
        // Switching cwd to the project makes generated workspace-relative
        // paths invalid on Windows when Perro itself lives elsewhere.
        .current_dir(cwd)
        .env("PERRO_CAPTURE_MODE", &mode)
        .env("PERRO_CAPTURE_SOURCE", source)
        .env("PERRO_CAPTURE_OUTPUT", &output)
        .env("PERRO_CAPTURE_FORMAT", format)
        .env("PERRO_CAPTURE_FPS", fps.decimal_string())
        .env("PERRO_CAPTURE_FPS_NUM", fps.numerator().to_string())
        .env("PERRO_CAPTURE_FPS_DEN", fps.denominator().to_string())
        .env("PERRO_CAPTURE_DURATION", format_number(duration))
        .env("PERRO_CAPTURE_FRAMES", frames.to_string())
        .env("PERRO_CAPTURE_SUPERSAMPLE", supersample.to_string())
        .env("PERRO_CAPTURE_ASPECT", aspect)
        .env("PERRO_CAPTURE_FRAMING", framing)
        .env(
            "PERRO_CAPTURE_TRANSPARENT",
            if args.iter().any(|arg| arg == "--transparent") {
                "1"
            } else {
                "0"
            },
        )
        .env(
            "PERRO_CAPTURE_WIDTH",
            width.map_or_else(String::new, |v| v.to_string()),
        )
        .env(
            "PERRO_CAPTURE_HEIGHT",
            height.map_or_else(String::new, |v| v.to_string()),
        );
    if mode == "offline" {
        command.env("PERRO_OFFLINE_FPS", fps.decimal_string());
        command.env("PERRO_OFFLINE_FPS_NUM", fps.numerator().to_string());
        command.env("PERRO_OFFLINE_FPS_DEN", fps.denominator().to_string());
        command.env("PERRO_OFFLINE_FRAMES", frames.to_string());
    }
    let status = command
        .status()
        .map_err(|err| format!("launch capture runner: {err}"))?;
    if !status.success() {
        return Err(format!(
            "capture runner failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn parse_required_path(args: &[String], flag: &str, cwd: &Path) -> Result<PathBuf, String> {
    let raw =
        parse_flag_value(args, flag).ok_or_else(|| format!("missing required flag `{flag}`"))?;
    let path = resolve_local_path(&raw, cwd);
    if path.as_os_str().is_empty() {
        return Err(format!("`{flag}` must not be empty"));
    }
    Ok(path)
}

fn parse_nonempty(args: &[String], flag: &str, default: &str) -> Result<String, String> {
    let value = parse_flag_value(args, flag).unwrap_or_else(|| default.to_string());
    if value.trim().is_empty() {
        return Err(format!("`{flag}` must not be empty"));
    }
    Ok(value)
}

fn parse_positive(args: &[String], flag: &str, default: f64) -> Result<f64, String> {
    let value = parse_flag_value(args, flag)
        .map(|raw| {
            raw.parse::<f64>()
                .map_err(|_| format!("invalid `{flag}` value `{raw}`"))
        })
        .transpose()?
        .unwrap_or(default);
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("`{flag}` must be finite and > 0"));
    }
    Ok(value)
}

fn parse_rate(args: &[String], flag: &str, default: FrameRate) -> Result<FrameRate, String> {
    let Some(raw) = parse_flag_value(args, flag) else {
        return Ok(default);
    };
    FrameRate::parse(&raw).map_err(|_| format!("invalid `{flag}` value `{raw}`"))
}

fn parse_supersample(args: &[String]) -> Result<u32, String> {
    let Some(raw) = parse_flag_value(args, "--supersample") else {
        return Ok(DEFAULT_SUPERSAMPLE);
    };
    let value = raw.parse::<u32>().map_err(|_| {
        format!("invalid `--supersample` value `{raw}`; use an integer from 1 through 8")
    })?;
    if !(1..=8).contains(&value) {
        return Err(format!(
            "invalid `--supersample` value `{raw}`; use an integer from 1 through 8"
        ));
    }
    Ok(value)
}

fn parse_optional_u32(args: &[String], flag: &str) -> Result<Option<u32>, String> {
    let Some(raw) = parse_flag_value(args, flag) else {
        return Ok(None);
    };
    let value = raw
        .parse::<u32>()
        .map_err(|_| format!("invalid `{flag}` value `{raw}`"))?;
    if value == 0 {
        return Err(format!("`{flag}` must be > 0"));
    }
    Ok(Some(value))
}

fn parse_choice(
    args: &[String],
    flag: &str,
    choices: &[&str],
    default: &str,
) -> Result<String, String> {
    let value = parse_flag_value(args, flag).unwrap_or_else(|| default.to_string());
    if choices
        .iter()
        .any(|choice| value.eq_ignore_ascii_case(choice))
    {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(format!(
            "invalid `{flag}` value `{value}`; use {}",
            choices.join(" | ")
        ))
    }
}

fn parse_aspect(args: &[String]) -> Result<String, String> {
    let value = parse_flag_value(args, "--aspect").unwrap_or_else(|| "preserve".to_string());
    if value.eq_ignore_ascii_case("preserve") {
        return Ok("preserve".to_string());
    }
    let mut parts = value.split(':');
    let width = parts.next().and_then(|part| part.parse::<u32>().ok());
    let height = parts.next().and_then(|part| part.parse::<u32>().ok());
    if parts.next().is_some() || width.is_none_or(|v| v == 0) || height.is_none_or(|v| v == 0) {
        return Err(format!(
            "invalid `--aspect` value `{value}`; use preserve or W:H"
        ));
    }
    Ok(value)
}

fn exact_frame_count(duration: f64, rate: FrameRate) -> Result<u64, String> {
    let frames = duration * f64::from(rate.numerator()) / f64::from(rate.denominator());
    if !frames.is_finite() || frames <= 0.0 || frames > u64::MAX as f64 {
        return Err("duration * fps exceeds frame count limits".to_string());
    }
    let rounded = frames.round();
    if (frames - rounded).abs() > 1.0e-6 {
        return Err("duration * fps must resolve to a whole frame count".to_string());
    }
    Ok(rounded as u64)
}

fn format_number(value: f64) -> String {
    format!("{value:.9}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{exact_frame_count, parse_rate, parse_supersample};
    use perro_capture::FrameRate;

    #[test]
    fn frame_count_matches_requested_duration() {
        assert_eq!(exact_frame_count(6.0, FrameRate::integer(60)), Ok(360));
        assert_eq!(exact_frame_count(10.0, FrameRate::integer(300)), Ok(3000));
    }

    #[test]
    fn frame_count_rejects_fractional_output() {
        assert!(exact_frame_count(1.0, FrameRate::parse("29.97").expect("rate")).is_err());
    }

    #[test]
    fn fractional_rate_accepts_decimal_and_exact_frame_count() {
        let args = vec!["--fps".to_owned(), "30000/1001".to_owned()];
        let rate = parse_rate(&args, "--fps", FrameRate::integer(60)).expect("rate");
        assert_eq!(rate, FrameRate::new(30000, 1001).expect("rate"));
        assert_eq!(exact_frame_count(1.001, rate), Ok(30));
    }

    #[test]
    fn supersample_defaults_to_two_and_rejects_fractional_values() {
        assert_eq!(parse_supersample(&[]), Ok(2));
        let args = vec!["--supersample".to_owned(), "2.5".to_owned()];
        let error = parse_supersample(&args).expect_err("fractional supersample");
        assert!(error.contains("integer from 1 through 8"));
    }

    #[test]
    fn supersample_accepts_bounds_and_rejects_zero() {
        let low = vec!["--supersample".to_owned(), "1".to_owned()];
        let high = vec!["--supersample".to_owned(), "8".to_owned()];
        let zero = vec!["--supersample".to_owned(), "0".to_owned()];
        assert_eq!(parse_supersample(&low), Ok(1));
        assert_eq!(parse_supersample(&high), Ok(8));
        assert!(parse_supersample(&zero).is_err());
    }
}
